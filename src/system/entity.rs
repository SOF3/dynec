use std::cell::RefCell;
use std::marker::PhantomData;
use std::{mem, ops};

use rayon::prelude::ParallelIterator;

use super::{accessor, Accessor};
use crate::entity::ealloc::Shard;
use crate::entity::{self, ealloc, Ref as _};
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

/// Allows iterating all entities of an archetype.
pub trait EntityIterator<A: Archetype> {
    /// Return value of [`entities`](Self::entities).
    type Entities<'t>: Iterator<Item = entity::TempRef<'t, A>>
    where
        Self: 't;
    /// Iterates over all entity IDs in this archetype.
    fn entities(&self) -> Self::Entities<'_>;

    /// Return value of [`par_entities`](Self::par_entities).
    type ParEntities<'t>: ParallelIterator<Item = entity::TempRef<'t, A>>
    where
        Self: 't;
    /// Iterates over all entity IDs in this archetype in parallel.
    fn par_entities(&self) -> Self::ParEntities<'_>;

    /// Return value of [`chunks`](Self::chunks).
    type Chunks<'t>: Iterator<Item = entity::TempRefChunk<'t, A>>
    where
        Self: 't;
    /// Iterates over all contiguous chunks of entity IDs.
    fn chunks(&self) -> Self::Chunks<'_>;

    /// Return value of [`entities_with`](Self::entities_with).
    type EntitiesWith<'t, T: Accessor<A> + 't>: Iterator<
        Item = (entity::TempRef<'t, A>, T::Entity<'t>),
    >
    where
        Self: 't;
    /// Iterates over all entities, yielding the components requested.
    fn entities_with<T: Accessor<A>>(&self, accessors: T) -> Self::EntitiesWith<'_, T>;

    /// Return value of [`chunks_with`](Self::chunks_with).
    type ChunksWith<'t, T: accessor::Chunked<A> + 't>: Iterator<
        Item = (entity::TempRefChunk<'t, A>, T::Chunk<'t>),
    >
    where
        Self: 't;
    /// Iterates over all entities,
    /// yielding the components requested in contiguous chunks.
    fn chunks_with<T: accessor::Chunked<A>>(&self, accessors: T) -> Self::ChunksWith<'_, T>;
}

pub struct EntityIteratorImpl<R> {
    ealloc: R,
}

impl<A: Archetype, R> EntityIterator<A> for EntityIteratorImpl<R>
where
    R: ops::Deref,
    <R as ops::Deref>::Target: ealloc::Shard<Raw = A::RawEntity>,
{
    type Entities<'t> = impl Iterator<Item = entity::TempRef<'t, A>>
    where
        Self: 't;
    fn entities(&self) -> Self::Entities<'_> {
        self.ealloc
            .iter_allocated_chunks()
            .flat_map(<A::RawEntity as entity::Raw>::range)
            .map(entity::TempRef::new)
    }

    type ParEntities<'t> = impl ParallelIterator<Item = entity::TempRef<'t, A>>
    where
        Self: 't;
    fn par_entities(&self) -> Self::ParEntities<'_> {
        self.ealloc
            .par_iter_allocated_chunks()
            .flat_map(<A::RawEntity as entity::Raw>::par_range)
            .map(entity::TempRef::new)
    }

    type Chunks<'t> = impl Iterator<Item = entity::TempRefChunk<'t, A>> + 't
    where
        Self: 't;
    fn chunks(&self) -> Self::Chunks<'_> {
        self.ealloc.iter_allocated_chunks().map(|range| entity::TempRefChunk {
            start: range.start,
            end:   range.end,
            _ph:   PhantomData,
        })
    }

    type EntitiesWith<'t, T: Accessor<A> + 't> = impl Iterator<Item = (entity::TempRef<'t, A>, T::Entity<'t>)>
    where
        Self: 't;
    fn entities_with<T: Accessor<A>>(&self, mut accessor: T) -> Self::EntitiesWith<'_, T> {
        let mut previous = None;
        self.entities().map(move |entity| {
            if let Some(previous) = mem::replace(&mut previous, Some(entity)) {
                assert!(previous.id() < entity.id());
            }
            let projected = unsafe { T::entity(&mut accessor, entity) };
            (entity, projected)
        })
    }

    type ChunksWith<'t, T: accessor::Chunked<A> + 't> = impl Iterator<Item = (entity::TempRefChunk<'t, A>, T::Chunk<'t>)> + 't
    where
        Self: 't;
    fn chunks_with<T: accessor::Chunked<A>>(&self, mut accessor: T) -> Self::ChunksWith<'_, T> {
        let mut previous = None;
        self.chunks().map(move |chunk: entity::TempRefChunk<A>| {
            assert!(chunk.start <= chunk.end);
            if let Some(previous) = mem::replace(&mut previous, Some(chunk)) {
                assert!(previous.end < chunk.start);
            }
            let projected = unsafe { T::chunk(&mut accessor, chunk) };
            (chunk, projected)
        })
    }
}

/// Constructs an instance of [`EntityIterator`] that reads from the given allocator.
///
/// Although this function accepts an allocator shard,
/// it actually reads the global buffer shared between shards,
/// which is independent of the changes in the current shard.
/// Hence, the iterator describe the state after the previous tick completes,
/// which does not include newly initialized entities
/// and includes those queued for deletion.
/// This behavior is reasonable, because newly initialized entities should not be accessed at all,
/// and those queued for deletion may have a finalizer or
/// be given a finalizer when running later systems,
/// so those queued for deletion are still included.
///
/// This function is typically called from the code generated by
/// [`#[system]`](macro@crate::system).
pub fn entity_iterator<A: Archetype, R>(ealloc: R) -> impl EntityIterator<A>
where
    R: ops::Deref,
    <R as ops::Deref>::Target: ealloc::Shard<Raw = A::RawEntity>,
{
    EntityIteratorImpl { ealloc }
}
