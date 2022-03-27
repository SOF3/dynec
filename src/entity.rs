//! An entity is a single object that owns components.
//!
//! Entities are reference-counted in debug mode,
//! so they cannot be copied directly.
//!
//! All components and states referencing entities
//! must override the `for_each_entity` method.
//!
//! All strong references to an entity must be dropped before it gets deleted.

use std::any::TypeId;
use std::marker::PhantomData;
use std::num;
use std::sync::Arc;

use crate::Archetype;

mod sealed {
    use crate::Archetype;

    pub trait Ref<A: Archetype> {
        fn id(&self) -> super::Raw;
    }
}

/// A raw, untyped entity ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Raw(u32);

impl Raw {
    pub(crate) fn usize(self) -> usize { self.0.try_into().expect("usize >= u32") }
}

/// A trait implemented by all types of entity references.
pub trait Ref<A: Archetype>: sealed::Ref<A> {}

impl<A: Archetype, T: sealed::Ref<A>> Ref<A> for T {}

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

impl<A: Archetype> sealed::Ref<A> for Entity<A> {
    fn id(&self) -> Raw { self.id }
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

impl<A: Archetype> Owner for Entity<A> {
    fn visit<'s, 'f, F: FnMut(&'s mut Raw)>(&'s mut self, ty: TypeId, visitor: &'f mut F) {
        if ty == TypeId::of::<A>() {
            visitor(&mut self.id);
        }
    }
}

/// A weak counted reference to an entity.
///
/// This reference can outlive the entity.
/// However, it must still be visited in [`Owner::visit`].
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

impl<A: Archetype> sealed::Ref<A> for Weak<A> {
    fn id(&self) -> Raw { self.id }
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

impl<A: Archetype> Owner for Weak<A> {
    fn visit<'s, 'f, F: FnMut(&'s mut Raw)>(&'s mut self, ty: TypeId, visitor: &'f mut F) {
        if ty == TypeId::of::<A>() {
            visitor(&mut self.id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Generation(num::Wrapping<u32>);

/// A type that may own entity references (no matter strong or weak).
pub trait Owner {
    /// Executes the given function for each entity reference.
    fn visit<'s, 'f, F: FnMut(&'s mut Raw)>(&'s mut self, ty: TypeId, visitor: &'f mut F);
}
