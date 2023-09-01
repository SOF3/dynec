//! An entity is a single object that owns components.
//!
//! Entities are reference-counted in debug mode,
//! so they cannot be copied directly.
//!
//! All components and states referencing entities
//! must override the `for_each_entity` method.
//!
//! All strong references to an entity must be dropped before it gets deleted.

use std::iter;
use std::marker::PhantomData;

use crate::Archetype;

mod raw;
pub use raw::Raw;

pub mod deletion;

pub mod ealloc;
pub use ealloc::Ealloc;

pub mod generation;
pub use generation::Generation;

pub(crate) mod rctrack;

pub mod referrer;
pub use referrer::Referrer;

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

/// A temporary, non-`'static` reference to an entity.
///
/// This type is not ref-counted.
/// It is only used as a short-lived pointer passed from the dynec API
/// for entity references that should not outlive a short scope (e.g. an API callback).
#[repr(transparent)]
pub struct TempRef<'t, A: Archetype> {
    value: A::RawEntity,
    _ph:   PhantomData<&'t ()>,
}

impl<'t, A: Archetype> TempRef<'t, A> {
    /// Creates a new TemporaryRef with a lifetime.
    pub(crate) fn new(value: A::RawEntity) -> Self { Self { value, _ph: PhantomData } }
}

impl<'t, A: Archetype> sealed::Sealed for TempRef<'t, A> {}
impl<'t, A: Archetype> Ref for TempRef<'t, A> {
    type Archetype = A;
    fn id(&self) -> A::RawEntity { self.value }
}

impl<'t, A: Archetype> Clone for TempRef<'t, A> {
    fn clone(&self) -> Self { *self }
}

impl<'t, A: Archetype> Copy for TempRef<'t, A> {}

/// A chunk of continuous [`TempRef`]s.
// Instantiations of this struct must guarantee that all entities in `start..end`
// satisfy the presence invariants for the duration of the lifetime `'t`.
pub struct TempRefChunk<'t, A: Archetype> {
    pub(crate) start: A::RawEntity,
    pub(crate) end:   A::RawEntity,
    pub(crate) _ph:   PhantomData<&'t ()>,
}

impl<'t, A: Archetype> TempRefChunk<'t, A> {
    /// Iterates over all entities in the chunk.
    pub fn iter(&self) -> impl Iterator<Item = TempRef<'t, A>> + '_ {
        iter::successors(Some(self.start), |prev| {
            Some(prev.add(1)).filter(|&value| value < self.end)
        })
        .map(|value| TempRef::new(value))
    }
}

impl<'t, A: Archetype> Clone for TempRefChunk<'t, A> {
    fn clone(&self) -> Self { *self }
}

impl<'t, A: Archetype> Copy for TempRefChunk<'t, A> {}

#[cfg(any(
    all(debug_assertions, feature = "debug-entity-rc"),
    all(not(debug_assertions), feature = "release-entity-rc"),
))]
pub(crate) mod maybe {
    use std::sync::{Arc, Weak};

    pub(crate) type MaybeArc = Arc<()>;
    pub(crate) type MaybeWeak = Weak<()>;

    pub(crate) fn downgrade(arc: &MaybeArc) -> MaybeWeak { Arc::downgrade(arc) }
}

#[cfg(not(any(
    all(debug_assertions, feature = "debug-entity-rc"),
    all(not(debug_assertions), feature = "release-entity-rc"),
)))]
mod maybe {
    #[derive(Clone, Default)]
    pub(crate) struct MaybeArc;
    #[derive(Clone)]
    pub(crate) struct MaybeWeak;

    #[allow(clippy::unused_unit)]
    pub(crate) fn downgrade(&MaybeArc: &MaybeArc) -> MaybeWeak { MaybeWeak }
}

pub(crate) use maybe::{MaybeArc, MaybeWeak};

impl<'t, T: Ref> sealed::Sealed for &'t T {}
impl<'t, T: Ref> Ref for &'t T {
    type Archetype = T::Archetype;
    fn id(&self) -> <T::Archetype as Archetype>::RawEntity { Ref::id(&**self) }
}

/// A strong reference to an entity.
///
/// This reference must be dropped before an entity is deleted
/// (after all finalizers have been unset).
/// Use `Weak` if the reference is allowed to outlive the entity.
pub struct Entity<A: Archetype> {
    id: A::RawEntity,

    pub(crate) rc: MaybeArc,
}

impl<A: Archetype> Entity<A> {
    /// Creates a new strong reference to an entity.
    ///
    /// This method should only be used when a completely new entity has been created.
    #[allow(clippy::default_constructed_unit_structs)]
    pub(crate) fn new_allocated(id: A::RawEntity) -> Self { Self { id, rc: MaybeArc::default() } }

    /// Converts the strong reference into a weak reference.
    pub fn weak(&self, store: &impl generation::WeakStore) -> Weak<A> {
        let store = store
            .resolve::<A>()
            .expect("entity was instantiated without generation store initialized");

        // since this strong reference is still valid,
        // the current state of the generation store is the actual generation.
        let generation = store.get(self.id.to_primitive());

        Weak { id: self.id, generation, rc: maybe::downgrade(&self.rc) }
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

            rc: Clone::clone(&self.rc),
        }
    }
}

/// A weak counted reference to an entity.
///
/// This reference can outlive the entity.
/// However, it must still be visited in [`referrer::Referrer::visit_mut`].
///
/// This type additionally stores the generation of an entity
/// in order to disambiguate new entities that uses the recycled memory.
/// Therefore, the weak reference actually consumes more memory than the strong reference.
pub struct Weak<A: Archetype> {
    id:         A::RawEntity,
    generation: Generation,

    rc: maybe::MaybeWeak,
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

            rc: Clone::clone(&self.rc),
        }
    }
}

#[cfg(test)]
mod tests;
