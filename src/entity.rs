//! An entity is a single object that owns components.
//!
//! Entities are reference-counted in debug mode,
//! so they cannot be copied directly.
//!
//! All components and states referencing entities
//! must override the `for_each_entity` method.
//!
//! All strong references to an entity must be dropped before it gets deleted.

use std::marker::PhantomData;
use std::num;
use std::sync::Arc;

use crate::Archetype;

mod permutation;
pub use permutation::Permutation;

mod referrer;
pub use referrer::{Referrer, Visitor};

/// Re-export of [`dynec::EntityRef`](crate::EntityRef).
pub use crate::macro_docs::EntityRef as Ref;

mod sealed {
    pub trait Ref {
        fn id(&self) -> Raw;
    }

    /// Sealed-public wrapper for `Raw`.
    pub struct Raw(pub(crate) super::Raw);

    /// Sealed-public wrapper for `&mut Raw`.
    pub struct RefMutRaw<'s>(pub(crate) &'s mut super::Raw);
}

#[allow(unused_imports)]
pub(crate) use sealed::Ref as RefId;

/// A trait implemented by all types of entity references.
pub trait Ref: sealed::Ref {
    /// The archetype that this entity belongs to.
    type Archetype: Archetype;
}

/// A raw, untyped entity ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Raw(u32);

impl Raw {
    pub(crate) fn usize(self) -> usize { self.0.try_into().expect("usize >= u32") }

    pub(crate) fn into_api<A: Archetype>(self) -> UnclonableRef<A> {
        UnclonableRef { value: self, _ph: PhantomData }
    }
}

/// An unclonable reference to an entity.
///
/// This type is not ref-counted.
/// It is only used as a short-lived pointer passed from the dynec API
/// for entity references that should not outlive a short scope (e.g. an API callback).
/// Thus, it is always passed to users as a reference and cannot be cloned.
#[repr(transparent)]
pub struct UnclonableRef<A: Archetype> {
    value: Raw,
    _ph:   PhantomData<A>,
}

impl<A: Archetype> sealed::Ref for UnclonableRef<A> {
    fn id(&self) -> sealed::Raw { sealed::Raw(self.value) }
}
impl<A: Archetype> Ref for UnclonableRef<A> {
    type Archetype = A;
}

/// A counted reference to an entity.
///
/// This reference must be dropped before an entity is deleted
/// (after all finalizers have been unset).
/// Use `Weak` if the reference is allowed to outlive the entity.
pub struct Entity<A: Archetype> {
    id:  Raw,
    _ph: PhantomData<A>,

    #[cfg(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ))]
    rc: Arc<()>,
}

impl<A: Archetype> Entity<A> {
    /// Allocates a new strong reference to an entity.
    ///
    /// This method should only be used when a completely new entity has been created.
    pub(crate) fn allocate_new(id: Raw) -> Self {
        Self {
            id,
            _ph: PhantomData,

            #[cfg(any(
                all(debug_assertions, feature = "debug-entity-rc"),
                all(not(debug_assertions), feature = "release-entity-rc"),
            ))]
            rc: Arc::new(()),
        }
    }
}

impl<A: Archetype> sealed::Ref for Entity<A> {
    fn id(&self) -> sealed::Raw { sealed::Raw(self.id) }
}
impl<A: Archetype> Ref for Entity<A> {
    type Archetype = A;
}

impl<A: Archetype> Clone for Entity<A> {
    fn clone(&self) -> Self {
        Self {
            id:  self.id,
            _ph: PhantomData,

            #[cfg(any(
                all(debug_assertions, feature = "debug-entity-rc"),
                all(not(debug_assertions), feature = "release-entity-rc"),
            ))]
            rc: Arc::clone(&self.rc),
        }
    }
}

/// A weak counted reference to an entity.
///
/// This reference can outlive the entity.
/// However, it must still be visited in [`Referrer::visit_each`].
///
/// This type additionally stores the generation of an entity
/// in order to disambiguate new entities that uses the recycled memory.
/// Therefore, the weak reference actually consumes more memory than the strong reference.
pub struct Weak<A: Archetype> {
    id:         Raw,
    generation: Generation,
    _ph:        PhantomData<A>,

    #[cfg(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ))]
    rc: Arc<()>,
}

impl<A: Archetype> sealed::Ref for Weak<A> {
    fn id(&self) -> sealed::Raw { sealed::Raw(self.id) }
}
impl<A: Archetype> Ref for Weak<A> {
    type Archetype = A;
}

impl<A: Archetype> Clone for Weak<A> {
    fn clone(&self) -> Self {
        Self {
            id:         self.id,
            generation: self.generation,
            _ph:        PhantomData,

            #[cfg(any(
                all(debug_assertions, feature = "debug-entity-rc"),
                all(not(debug_assertions), feature = "release-entity-rc"),
            ))]
            rc: Arc::clone(&self.rc),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Generation(num::Wrapping<u32>);

#[cfg(test)]
mod tests {
    use super::{Raw, Ref};
    use crate::test_util::TestArch;

    // ensure that Ref<Archetype = A> for a fixed `A` must be object-safe.
    fn test_object_safety() {
        let _: &dyn Ref<Archetype = TestArch> = &Raw(1).into_api::<TestArch>();
    }
}
