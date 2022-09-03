//! Tracks entity references owned by components and globals.
//! See [`Referrer`] for more information.

use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops;

use self::search_single::SearchSingleStrong;
use super::Raw;
use crate::slice_any::AnySliceMut;
use crate::util::DbgTypeId;
use crate::Archetype;

pub(crate) mod search_single;
mod std_impl;

/// A type that may own entity references (no matter strong or weak).
///
/// The parameters passed in this trait are abstracted by opaque types/traits.
/// Implementors should only forward the arg reference to implementors in its member fields,
/// where the actual logic is eventually implemented by
/// owned [`Entity`](super::Entity) and [`Weak`](super::Weak) fields.
///
/// # Non-implementors
/// ## [`UnclonableRef`](super::UnclonableRef)
/// This trait is deliberately not implemented for `UnclonableRef`,
/// because this trait should only be used in global states and components,
/// while `UnclonableRef` should only be used in temporary variables in systems.
///
/// ## [`Rc`](std::rc::Rc)/[`Arc`](std::sync::Arc)
/// Each entity reference must be visited only exactly once.
/// Therefore, it is not possible to implement `Referrer` for ref-counted types,
/// because multiple references would be visited multiple times.
/// If sharing an entity reference is ever necessary,
/// consider refactoring to store the underlying type in a separate unique global state.
///
/// # Example implementation
/// ```
/// use dynec::entity::referrer;
///
/// struct MyCollection<T: referrer::Referrer> {
///     data: [T; 10],
/// };
///
/// impl<T: referrer::Referrer> referrer::Referrer for MyCollection<T> {
///     fn visit_type(arg: &mut referrer::VisitTypeArg) {
///         if arg.mark::<Self>().is_continue() {
///             <T as referrer::Referrer>::visit_type(arg);
///         }
///     }
///
///     fn visit_mut<V: referrer::VisitMutArg>(&mut self, arg: &mut V) {
///         for value in &mut self.data {
///             <T as referrer::Referrer>::visit_mut(value, arg);
///         }
///     }
/// }
/// ```
pub trait Referrer: 'static {
    /// Visit all types that may appear under this referrer.
    ///
    /// It is OK to visit the same type twice.
    /// `arg` contains an internal hash set that avoids recursion.
    fn visit_type(arg: &mut VisitTypeArg);

    /// Execute the given function on every strong and weak entity reference exactly once.
    ///
    /// Implementors are recommended to mark the implementation as `#[inline]`
    /// since this function is a no-op for most implementors.
    fn visit_mut<V: VisitMutArg>(&mut self, arg: &mut V);
}

/// The opaque argument passed to [`Referrer::visit_type`].
/// This type is used to hide the implementation detail from users
/// such that the actual arguments are only visible to the internals.
pub struct VisitTypeArg<'t> {
    recursion_guard:        HashSet<DbgTypeId>,
    pub(crate) found_archs: HashSet<DbgTypeId>,
    // for future compatibility
    _ph:                    PhantomData<&'t ()>,
}

impl<'t> VisitTypeArg<'t> {
    pub(crate) fn new() -> Self {
        Self {
            recursion_guard: HashSet::new(),
            found_archs:     HashSet::new(),
            _ph:             PhantomData,
        }
    }

    /// All types visited by this arg must call `mark` at least once to avoid recursion.
    /// Implementors should return immediately if [`ops::ControlFlow::Break`] is returned.
    pub fn mark<T: 'static>(&mut self) -> ops::ControlFlow<(), ()> {
        if self.recursion_guard.insert(DbgTypeId::of::<T>()) {
            ops::ControlFlow::Continue(())
        } else {
            ops::ControlFlow::Break(())
        }
    }

    fn add_archetype<A: Archetype>(&mut self) { self.found_archs.insert(DbgTypeId::of::<A>()); }
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// The trait bound for arguments passed to [`Referrer::visit_mut`].
///
/// This is a bound-only trait.
/// Downstream crates cannot implement this trait,
/// nor should they call any methods on this trait.
pub trait VisitMutArg: sealed::Sealed {
    #[doc(hidden)]
    fn _visit_strong(&mut self, args: VisitStrongArgs) -> VisitStrongResult;

    #[doc(hidden)]
    fn _visit_weak(&mut self, args: VisitWeakArgs) -> VisitWeakResult;
}

#[doc(hidden)]
pub struct VisitStrongArgs<'t> {
    archetype: DbgTypeId,
    raw:       usize,
    rc:        &'t mut super::MaybeArc,
}

#[doc(hidden)]
pub struct VisitStrongResult {
    new_raw: usize,
}

#[doc(hidden)]
pub struct VisitWeakArgs<'t> {
    archetype: DbgTypeId,
    raw:       usize,
    rc:        &'t mut super::MaybeWeak,
}

#[doc(hidden)]
pub struct VisitWeakResult {
    new_raw: usize,
}

impl<A: Archetype> Referrer for super::Entity<A> {
    #[inline]
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_break() {
            return;
        }
        arg.add_archetype::<A>();
    }

    #[inline]
    fn visit_mut<V: VisitMutArg>(&mut self, arg: &mut V) {
        let ret = arg._visit_strong(VisitStrongArgs {
            archetype: DbgTypeId::of::<A>(),
            raw:       self.id.to_primitive(),
            rc:        &mut self.rc,
        });
        self.id = A::RawEntity::from_primitive(ret.new_raw);
    }
}

impl<A: Archetype> Referrer for super::Weak<A> {
    #[inline]
    fn visit_type(arg: &mut VisitTypeArg) { arg.mark::<Self>(); }

    #[inline]
    fn visit_mut<V: VisitMutArg>(&mut self, arg: &mut V) {
        let ret = arg._visit_weak(VisitWeakArgs {
            archetype: DbgTypeId::of::<A>(),
            raw:       self.id.to_primitive(),
            rc:        &mut self.rc,
        });
        self.id = A::RawEntity::from_primitive(ret.new_raw);
    }
}

/// Virtual dispatch table that operates on slices of its target.
pub(crate) struct Vtable {
    search_single_strong: for<'t> fn(&'t mut AnySliceMut<'t>, &mut SearchSingleStrong),
}

impl Vtable {
    pub(crate) fn of<T: Referrer>() -> Self {
        Self {
            search_single_strong: |slice, state| {
                let slice = slice.reborrow().downcast::<T>();
                for item in slice {
                    item.visit_mut(state);
                }
            },
        }
    }

    pub(crate) fn search_single_strong<'t>(
        &self,
        slice: &'t mut AnySliceMut<'t>,
        state: &mut SearchSingleStrong,
    ) {
        (self.search_single_strong)(slice, state)
    }
}
