//! Tracks entity references owned by components and globals.
//! See [`Referrer`] for more information.

use std::any::TypeId;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops;

use crate::util::DbgTypeId;
use crate::Archetype;

mod std_impl;

/// The object-safe part of [`Referrer`].
pub trait Dyn: 'static {
    /// Performs the mapping and increments the counter for each entity.
    ///
    /// Each entity reference must be visited only exactly once for each visitor.
    /// As a result, `Referrer` is not implemented for [`Arc`](std::sync::Arc)
    /// because it may result in visiting the same entity reference multiple times,
    /// which will lead to incorrect behaviour.
    fn visit(&mut self, arg: &mut VisitArg);
}

/// The opaque argument passed to [`Dyn::visit`].
///
/// This type is used to hide the implementation detail from users
/// such that the actual arguments are only visible to the internals.
pub struct VisitArg<'t> {
    archetype: DbgTypeId,
    mapping:   &'t [usize],
    counter:   &'t mut usize,
}

/// A type that may own entity references (no matter strong or weak).
///
/// The parameters passed in this trait are abstracted by the opaque type [`VisitArg`].
/// Implementors should only forward the arg reference to the other implementors,
/// where the actual logic is eventually implemented by
/// owned [`Entity`](super::Entity) and [`Weak`](super::Weak) fields.
///
/// This trait is deliberately not implemented for [`UnclonableRef`](super::UnclonableRef),
/// because this trait should only be used in global states and components,
/// whilst `UnclonableRef` should only be used in temporary variables in systems.
pub trait Referrer: Dyn {
    /// Visit all types that may appear under this referrer.
    ///
    /// It is OK to visit the same type twice.
    /// `arg` contains an internal hash set that avoids recursion.
    fn visit_type(arg: &mut VisitTypeArg);
}

/// The opaque argument passed to [`Dyn::visit`].
///
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

impl<A: Archetype> Dyn for super::Weak<A> {
    fn visit(&mut self, &mut VisitArg { archetype, mapping, ref mut counter }: &mut VisitArg) {
        if archetype == TypeId::of::<A>() {
            let &new = mapping
                .get(super::Raw::to_primitive(self.id))
                .expect("Weak reference not in entity. ");
            self.id = <A::RawEntity as super::Raw>::from_primitive(new);
            **counter += 1;
        }
    }
}

impl<A: Archetype> Referrer for super::Weak<A> {
    fn visit_type(arg: &mut VisitTypeArg) { arg.mark::<Self>(); }
}

impl<A: Archetype> Dyn for super::Entity<A> {
    fn visit(&mut self, &mut VisitArg { archetype, mapping, ref mut counter }: &mut VisitArg) {
        if archetype == TypeId::of::<A>() {
            let &new = mapping
                .get(super::Raw::to_primitive(self.id))
                .expect("Weak reference not in entity. ");
            self.id = <A::RawEntity as super::Raw>::from_primitive(new);
            **counter += 1;
        }
    }
}

impl<A: Archetype> Referrer for super::Entity<A> {
    fn visit_type(arg: &mut VisitTypeArg) {
        if arg.mark::<Self>().is_break() {
            return;
        }
        arg.add_archetype::<A>();
    }
}
