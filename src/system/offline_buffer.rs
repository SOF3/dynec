use std::cell::RefCell;
use std::marker::PhantomData;

use crate::entity::{self, ealloc};
use crate::world::offline;
use crate::{comp, Archetype};

/// Allows creating entities of an archetype.
pub struct EntityCreator<'t, A: Archetype> {
    buffer: &'t RefCell<&'t mut offline::BufferShard>,
    ealloc: ealloc::BorrowedShard<'t, A>,
}

impl<'t, A: Archetype> EntityCreator<'t, A> {
    /// Constructs an entity creator.
    pub fn new(
        buffer: &'t RefCell<&'t mut offline::BufferShard>,
        ealloc: ealloc::BorrowedShard<'t, A>,
    ) -> Self {
        Self { buffer, ealloc }
    }

    /// Queues to create an entity.
    pub fn create(&mut self, comps: comp::Map<A>) -> entity::Entity<A> {
        self.with_hint(comps, Default::default())
    }

    /// Queues to create an entity with hint.
    pub fn with_hint(
        &mut self,
        comps: comp::Map<A>,
        hint: <A::Ealloc as entity::Ealloc>::AllocHint,
    ) -> entity::Entity<A> {
        let mut buffer = self.buffer.borrow_mut();
        let ealloc = &mut *self.ealloc;
        buffer.create_entity_with_hint_and_shard(comps, &mut *ealloc, hint)
    }
}

/// Allows deleting entities of an archetype.
pub struct EntityDeleter<'t, A: Archetype> {
    buffer: &'t RefCell<&'t mut offline::BufferShard>,
    _ph:    PhantomData<A>,
}

impl<'t, A: Archetype> EntityDeleter<'t, A> {
    /// Constructs an entity deleter from a macro.
    pub fn new(buffer: &'t RefCell<&'t mut offline::BufferShard>) -> Self {
        Self { buffer, _ph: PhantomData }
    }

    /// Queues to mark an entity for deletion.
    pub fn queue<E: entity::Ref<Archetype = A>>(&mut self, entity: E) {
        let mut buffer = self.buffer.borrow_mut();
        buffer.delete_entity::<A, E>(entity);
    }
}

#[cfg(test)]
mod tests;
