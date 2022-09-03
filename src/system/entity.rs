use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops;

use crate::entity::{self, ealloc};
use crate::world::offline;
use crate::{comp, Archetype};

/// Allows creating entities of an archetype.
pub trait EntityCreator<A: Archetype> {
    /// Queues to create an entity.
    fn create(&mut self, comps: comp::Map<A>) -> entity::Entity<A> {
        self.with_hint(comps, Default::default())
    }

    /// Queues to create an entity with hint.
    fn with_hint(
        &mut self,
        comps: comp::Map<A>,
        hint: <A::Ealloc as entity::Ealloc>::AllocHint,
    ) -> entity::Entity<A>;
}

/// An implementation of [`EntityCreator`], used in macros.
///
/// Semver-exempt.
#[doc(hidden)]
pub struct EntityCreatorImpl<'t, R: ops::DerefMut + 't>
where
    <R as ops::Deref>::Target: ealloc::Shard,
{
    pub buffer: &'t RefCell<&'t mut offline::BufferShard>,
    pub ealloc: R,
}

impl<'t, A: Archetype, R: ops::DerefMut> EntityCreator<A> for EntityCreatorImpl<'t, R>
where
    <R as ops::Deref>::Target:
        ealloc::Shard<Raw = A::RawEntity, Hint = <A::Ealloc as entity::Ealloc>::AllocHint>,
{
    fn with_hint(
        &mut self,
        comps: comp::Map<A>,
        hint: <<R as ops::Deref>::Target as ealloc::Shard>::Hint,
    ) -> entity::Entity<A> {
        let mut buffer = self.buffer.borrow_mut();
        let ealloc = &mut *self.ealloc;
        buffer.create_entity_with_hint_and_shard(comps, &mut *ealloc, hint)
    }
}

/// Allows deleting entities of an archetype.
pub trait EntityDeleter<A: Archetype> {
    /// Queues to mark an entity for deletion.
    fn queue<E: entity::Ref<Archetype = A>>(&mut self, entity: E);
}

/// An implementation of [`EntityDeleter`], used in macros.
///
/// Semver-exempt.
#[doc(hidden)]
pub struct EntityDeleterImpl<'t, A: Archetype> {
    pub buffer: &'t RefCell<&'t mut offline::BufferShard>,
    pub _ph:    PhantomData<A>,
}

impl<'t, A: Archetype> EntityDeleter<A> for EntityDeleterImpl<'t, A> {
    fn queue<E: entity::Ref<Archetype = A>>(&mut self, entity: E) {
        let mut buffer = self.buffer.borrow_mut();
        buffer.delete_entity::<A, E>(entity);
    }
}
