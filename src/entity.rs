//! An entity is a single object that owns components.
//!
//! Entities are reference-counted in debug mode,
//! so they cannot be copied directly.
//!
//! All components and states referencing entities
//! must override the `for_each_entity` method.
//!
//! All strong references to an entity must be dropped before it gets deleted.

#[cfg(any(
    all(debug_assertions, feature = "debug-entity-rc"),
    all(not(debug_assertions), feature = "release-entity-rc"),
))]
use std::sync;

use crate::Archetype;

mod raw;
pub use raw::Raw;

pub mod ealloc;
pub use ealloc::Ealloc;

pub mod generation;
pub use generation::Generation;

mod referrer;
pub use referrer::{Referrer, ReferrerArg};

/// Re-export of [`dynec::EntityRef`](crate::EntityRef).
pub use crate::macros::EntityRef as Ref;

mod sealed {
    pub trait Sealed {}
}

/// A trait implemented by all types of entity references.
pub trait Ref: sealed::Sealed {
    /// The archetype that this entity belongs to.
    type Archetype: Archetype;

    /// The underlying entity ID referenced.
    fn id(&self) -> <Self::Archetype as Archetype>::RawEntity;
}

/// An unclonable reference to an entity.
///
/// This type is not ref-counted.
/// It is only used as a short-lived pointer passed from the dynec API
/// for entity references that should not outlive a short scope (e.g. an API callback).
/// Thus, it is always passed to users as a reference and cannot be cloned.
///
/// This type deliberately does **not** implement [`Clone`] and [`Copy`] for the reasons above.
#[repr(transparent)]
pub struct UnclonableRef<A: Archetype> {
    value: A::RawEntity,
}

impl<A: Archetype> UnclonableRef<A> {
    /// Creates a new UnclonableRef.
    pub(crate) fn new(value: A::RawEntity) -> Self { Self { value } }
}

impl<A: Archetype> sealed::Sealed for UnclonableRef<A> {}
impl<A: Archetype> Ref for UnclonableRef<A> {
    type Archetype = A;
    fn id(&self) -> A::RawEntity { self.value }
}

/// A strong reference to an entity.
///
/// This reference must be dropped before an entity is deleted
/// (after all finalizers have been unset).
/// Use `Weak` if the reference is allowed to outlive the entity.
pub struct Entity<A: Archetype> {
    id: A::RawEntity,

    #[cfg(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ))]
    rc: sync::Arc<()>,
}

impl<A: Archetype> Entity<A> {
    /// Creates a new strong reference to an entity.
    ///
    /// This method should only be used when a completely new entity has been created.
    pub(crate) fn new_allocated(id: A::RawEntity) -> Self {
        Self {
            id,

            #[cfg(any(
                all(debug_assertions, feature = "debug-entity-rc"),
                all(not(debug_assertions), feature = "release-entity-rc"),
            ))]
            rc: sync::Arc::new(()),
        }
    }

    pub fn weak(&self, store: &generation::Store) -> Weak<A> {
        // since this strong reference is still valid,
        // the current state of the generation store is the actual generation.
        let generation = store.get(self.id.to_primitive());

        Weak {
            id: self.id,
            generation,
            #[cfg(any(
                all(debug_assertions, feature = "debug-entity-rc"),
                all(not(debug_assertions), feature = "release-entity-rc"),
            ))]
            rc: sync::Arc::downgrade(&self.rc),
        }
    }
}

impl<A: Archetype> sealed::Sealed for Entity<A> {}
impl<A: Archetype> Ref for Entity<A> {
    type Archetype = A;
    fn id(&self) -> A::RawEntity { self.id }
}

impl<A: Archetype> Clone for Entity<A> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,

            #[cfg(any(
                all(debug_assertions, feature = "debug-entity-rc"),
                all(not(debug_assertions), feature = "release-entity-rc"),
            ))]
            rc: sync::Arc::clone(&self.rc),
        }
    }
}

impl<'t, T: Ref> sealed::Sealed for &'t T {}
impl<'t, T: Ref> Ref for &'t T {
    type Archetype = T::Archetype;
    fn id(&self) -> <T::Archetype as Archetype>::RawEntity { Ref::id(&**self) }
}

/// A weak counted reference to an entity.
///
/// This reference can outlive the entity.
/// However, it must still be visited in [`Referrer::visit`].
///
/// This type additionally stores the generation of an entity
/// in order to disambiguate new entities that uses the recycled memory.
/// Therefore, the weak reference actually consumes more memory than the strong reference.
pub struct Weak<A: Archetype> {
    id:         A::RawEntity,
    generation: Generation,

    #[cfg(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ))]
    rc: sync::Weak<()>,
}

impl<A: Archetype> sealed::Sealed for Weak<A> {}
impl<A: Archetype> Ref for Weak<A> {
    type Archetype = A;
    fn id(&self) -> A::RawEntity { self.id }
}

impl<A: Archetype> Clone for Weak<A> {
    fn clone(&self) -> Self {
        Self {
            id:         self.id,
            generation: self.generation,

            #[cfg(any(
                all(debug_assertions, feature = "debug-entity-rc"),
                all(not(debug_assertions), feature = "release-entity-rc"),
            ))]
            rc: sync::Weak::clone(&self.rc),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::{Ref, UnclonableRef};
    use crate::TestArch;

    // ensure that Ref<Archetype = A> for a fixed `A` must be object-safe.
    fn test_object_safety() {
        let _: &dyn Ref<Archetype = TestArch> =
            &UnclonableRef::new(NonZeroU32::new(1).expect("1 != 0"));
    }
}
