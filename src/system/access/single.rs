use std::marker::PhantomData;
use std::{any, ops};

use rayon::prelude::ParallelIterator;

use crate::entity::{self, ealloc, Raw as _};
use crate::storage::{self, Access as _, Chunked as _};
use crate::{comp, util, Archetype, Storage};

/// Access a single component storage, i.e. a simple archetyped component
/// or an isotope archetyped component for a single discriminant.
pub struct AccessSingle<A, C, StorageRef> {
    storage: StorageRef,
    _ph:     PhantomData<(A, C)>,
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef> {
    pub(crate) fn new(storage: StorageRef) -> Self { Self { storage, _ph: PhantomData } }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    /// Returns an immutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    pub fn try_get(&self, entity: impl entity::Ref<Archetype = A>) -> Option<&C> {
        self.storage.get(entity.id())
    }

    /// Iterates over all initialized components in this storage.
    pub fn iter(&self) -> impl Iterator<Item = (entity::TempRef<A>, &C)> {
        self.storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::Must<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    /// Returns an immutable reference to the component for the specified entity.
    ///
    /// # Panics
    /// This method panics if the entity is not fully initialized yet.
    /// This happens when an entity is newly created and the cycle hasn't joined yet.
    pub fn get(&self, entity: impl entity::Ref<Archetype = A>) -> &C {
        match self.try_get(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }

    /// Iterates over chunks of entities in parallel.
    ///
    /// This returns a [rayon `ParallelIterator`](rayon::iter::ParallelIterator)
    /// that processes different chunks of entities
    ///
    /// Requires [`comp::Must<A>`] because this iterator assumes that
    /// existence in `snapshot` implies existence in storage.
    pub fn par_iter<'t>(
        &'t self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> impl ParallelIterator<Item = (entity::TempRef<'t, A>, &'t C)> {
        rayon::iter::split(snapshot.as_slice(), |slice| slice.split()).flat_map_iter(|slice| {
            slice.iter_chunks().flat_map(<A::RawEntity as entity::Raw>::range).map(|id| {
                let entity = entity::TempRef::new(id);
                let data = self.get(entity);
                (entity, data)
            })
        })
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: storage::Chunked<RawEntity = A::RawEntity, Comp = C>,
{
    /// Returns the chunk of components as a slice.
    ///
    /// # Panics
    /// This method panics if any component in the chunk is missing.
    /// In general, users should not get an [`entity::TempRefChunk`]
    /// that includes an uninitialized entity,
    /// so panic is basically impossible if [`comp::Must`] was implemented correctly.
    pub fn get_chunk(&self, chunk: entity::TempRefChunk<A>) -> &[C] {
        self.storage.get_chunk(chunk.start, chunk.end).expect("chunk is not completely filled")
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::Must<A>,
    StorageRef: ops::Deref + Sync,
    StorageRef::Target: storage::Chunked<RawEntity = A::RawEntity, Comp = C>,
{
    /// Iterates over chunks of entities in parallel.
    ///
    /// This returns a [rayon `ParallelIterator`](rayon::iter::ParallelIterator)
    /// that processes different chunks of entities
    pub fn par_iter_chunks<'t>(
        &'t self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> impl ParallelIterator<Item = (entity::TempRefChunk<'t, A>, &'t [C])> {
        rayon::iter::split(snapshot.as_slice(), |slice| slice.split()).flat_map_iter(|slice| {
            // we don't need to split over the holes in parallel,
            // because splitting the total space is more important than splitting the holes
            slice.iter_chunks().map(|chunk| {
                let chunk = entity::TempRefChunk::new(chunk.start, chunk.end);
                let data = self.get_chunk(chunk);
                (chunk, data)
            })
        })
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: storage::Access<RawEntity = A::RawEntity, Comp = C>,
{
    /// Returns a mutable reference to the component for the specified entity,
    /// or `None` if the component is not present in the entity.
    ///
    /// Note that this method returns `Option<&mut C>`, not `&mut Option<C>`.
    /// This means setting the Option itself to `Some`/`None` will not modify any stored value.
    /// Use [`Write::set`] to add/remove a component.
    pub fn try_get_mut(&mut self, entity: impl entity::Ref<Archetype = A>) -> Option<&mut C> {
        self.storage.get_mut(entity.id())
    }

    /// Iterates over mutable references to all initialized components in this storage.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (entity::TempRef<A>, &mut C)> {
        self.storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::Must<A>,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: storage::Access<RawEntity = A::RawEntity, Comp = C>,
{
    /// Returns a mutable reference to the component for the specified entity.
    ///
    /// This method is infallible, assuming [`comp::Must`] is only implemented
    /// for components with [`Required`](comp::Presence::Required) presence.
    pub fn get_mut(&mut self, entity: impl entity::Ref<Archetype = A>) -> &mut C {
        match self.try_get_mut(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    /// Overwrites the component for the specified entity.
    ///
    /// Passing `None` to this method removes the component from the entity.
    /// This leads to a panic for components with [`comp::Presence::Required`] presence.
    pub fn set(&mut self, entity: impl entity::Ref<Archetype = A>, value: Option<C>) -> Option<C> {
        self.storage.set(entity.id(), value)
    }

    /// Converts the accessor to a [`MutPartition`] that covers all entities.
    ///
    /// The actual splitting partitions can be obtained
    /// by calling [`split_at`](Access::split_at) on the returned value.
    pub fn as_partition(
        &mut self,
    ) -> AccessSingle<A, C, util::OwnedDeref<<StorageRef::Target as Storage>::Partition<'_>>> {
        AccessSingle {
            storage: util::OwnedDeref(self.storage.as_partition()),
            _ph:     PhantomData,
        }
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::Must<A>,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: Storage<RawEntity = A::RawEntity, Comp = C>,
{
    /// Iterates over all entities in parallel.
    ///
    /// This returns a rayon [`ParallelIterator`] that processes different entities.
    pub fn par_iter_mut<'t>(
        &'t mut self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> impl ParallelIterator<Item = (entity::TempRef<'t, A>, &'t mut C)> {
        rayon::iter::split((self.as_partition(), snapshot.as_slice()), |(partition, slice)| {
            let Some(midpt) = slice.midpoint_for_split() else { return ((partition, slice), None) };
            let (slice_left, slice_right) = slice.split_at(midpt);
            let (partition_left, partition_right) = partition.split_at(midpt);
            ((partition_left, slice_left), Some((partition_right, slice_right)))
        })
        .flat_map_iter(|(partition, _slice)| partition.into_iter_mut())
    }
}

impl<'t, A, C, StorageT> AccessSingle<A, C, util::OwnedDeref<StorageT>>
where
    A: Archetype,
    StorageT: storage::Partition<'t, RawEntity = A::RawEntity, Comp = C>,
{
    /// Splits the accessor into two partitions.
    ///
    /// The first partition accesses all entities less than `entity`;
    /// the second partition accesses all entities greater than or equal to `entity`.
    pub fn split_at(self, entity: A::RawEntity) -> (Self, Self) {
        let (left, right) = self.storage.0.split_at(entity);
        (
            Self { storage: util::OwnedDeref(left), _ph: PhantomData },
            Self { storage: util::OwnedDeref(right), _ph: PhantomData },
        )
    }

    pub fn split_out(&mut self, entity: A::RawEntity) -> Self {
        let right = self.storage.0.split_out(entity);
        Self { storage: util::OwnedDeref(right), _ph: PhantomData }
    }

    pub fn try_into_mut(self, entity: impl entity::Ref<Archetype = A>) -> Option<&'t mut C> {
        self.storage.0.into_mut(entity.id())
    }
}

impl<'t, A, C, StorageT> AccessSingle<A, C, util::OwnedDeref<StorageT>>
where
    A: Archetype,
    C: comp::Must<A>,
    StorageT: storage::Partition<'t, RawEntity = A::RawEntity, Comp = C>,
{
    pub fn into_mut(self, entity: impl entity::Ref<Archetype = A>) -> &'t mut C {
        match self.try_into_mut(entity) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }

    /// Iterates over mutable references to all initialized components in this partition.
    pub fn into_iter_mut(self) -> impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)> {
        self.storage.0.into_iter_mut().map(|(entity, data)| (entity::TempRef::new(entity), data))
    }
}

impl<A, C, StorageRef> AccessSingle<A, C, StorageRef>
where
    A: Archetype,
    C: comp::Must<A>,
    StorageRef: ops::DerefMut + Sync,
    StorageRef::Target: storage::Chunked<RawEntity = A::RawEntity, Comp = C>,
    for<'u> <StorageRef::Target as Storage>::Partition<'u>: storage::PartitionChunked<'u>,
{
    /// Returns the chunk of components as a mutable slice.
    /// Typically called from an accessor.
    ///
    /// # Panics
    /// This method panics if any component in the chunk is missing.
    /// In general, if [`comp::Must`] is implemented correctly,
    /// users should not obtain an [`entity::TempRefChunk`] that includes an uninitialized entity,
    /// so panic is practically impossible.
    pub fn get_chunk_mut(&mut self, chunk: entity::TempRefChunk<A>) -> &[C] {
        self.storage.get_chunk(chunk.start, chunk.end).expect("chunk is not completely filled")
    }

    /// Iterates over all entity chunks in parallel.
    ///
    /// This returns a rayon [`ParallelIterator`] that processes different chunks of entities.
    pub fn par_iter_chunks_mut<'t>(
        &'t mut self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> impl ParallelIterator<Item = (entity::TempRefChunk<'t, A>, &'t mut [C])> {
        rayon::iter::split((self.as_partition(), snapshot.as_slice()), |(partition, slice)| {
            let Some(midpt) = slice.midpoint_for_split() else { return ((partition, slice), None) };
            let (slice_left, slice_right) = slice.split_at(midpt);
            let (partition_left, partition_right) = partition.split_at(midpt);
            ((partition_left, slice_left), Some((partition_right, slice_right)))
        })
        .flat_map_iter(|(partition, _slice)| partition.into_iter_chunks_mut())
    }
}

impl<'t, A, C, StorageT> AccessSingle<A, C, util::OwnedDeref<StorageT>>
where
    A: Archetype,
    C: comp::Must<A>,
    StorageT: storage::PartitionChunked<'t, RawEntity = A::RawEntity, Comp = C>,
{
    pub fn into_chunk_mut(self, chunk: entity::TempRefChunk<A>) -> &'t mut [C] {
        match self.storage.0.into_chunk_mut(chunk.start, chunk.end) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        }
    }

    /// Iterates over mutable references to all initialized components in this storage.
    pub fn into_iter_chunks_mut(
        self,
    ) -> impl Iterator<Item = (entity::TempRefChunk<'t, A>, &'t mut [C])> {
        self.storage
            .0
            .into_iter_chunks_mut()
            .map(|(entity, data)| (entity::TempRefChunk::new(entity, entity.add(data.len())), data))
    }
}
