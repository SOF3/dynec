use std::any;
use std::marker::PhantomData;

use rayon::prelude::ParallelIterator;

use crate::entity::{self, ealloc, Raw as _};
use crate::storage::{self, Partition as _};
use crate::{comp, system, Archetype};

pub(super) struct PartitionAccessor<'t, A: Archetype, C, S> {
    pub(super) partition: S,
    pub(super) _ph:       PhantomData<(A, &'t C)>,
}

impl<'t, A, C, StorageParT> system::Mut<A, C> for PartitionAccessor<'t, A, C, StorageParT>
where
    A: Archetype,
    C: Send + Sync + 'static,
    StorageParT: storage::Partition<'t, A::RawEntity, C>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
        self.partition.get_mut(entity.id())
    }

    type IterMut<'u> = impl Iterator<Item = (entity::TempRef<'u, A>, &'u mut C)> + 'u where Self: 'u;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.partition
            .by_ref()
            .iter_mut()
            .map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}

impl<'t, A, C, StorageParT> system::MutPartition<'t, A, C>
    for PartitionAccessor<'t, A, C, StorageParT>
where
    A: Archetype,
    C: Send + Sync + 'static,
    StorageParT: storage::Partition<'t, A::RawEntity, C>,
{
    fn split_at<E: entity::Ref<Archetype = A>>(self, entity: E) -> (Self, Self) {
        let (left, right) = self.partition.partition_at(entity.id());

        (Self { partition: left, _ph: PhantomData }, Self { partition: right, _ph: PhantomData })
    }

    type IntoIterMut = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>;
    fn into_iter_mut(self) -> Self::IntoIterMut {
        self.partition.iter_mut().map(|(entity, data)| (entity::TempRef::new(entity), data))
    }
}

impl<'t, A, C, StorageParT> system::MutChunk<A, C> for PartitionAccessor<'t, A, C, StorageParT>
where
    A: Archetype,
    C: Send + Sync + 'static,
    StorageParT: storage::PartitionChunked<'t, A::RawEntity, C>,
{
    fn get_chunk_mut(&mut self, chunk: entity::TempRefChunk<'_, A>) -> &'_ mut [C]
    where
        C: comp::Must<A>,
    {
        match self.partition.get_chunk_mut(chunk.start, chunk.end) {
            Some(comp) => comp,
            None => panic!(
                "Component {}/{} implements comp::Must but is not present",
                any::type_name::<A>(),
                any::type_name::<C>(),
            ),
        }
    }
}

impl<'t, A, C, StorageParT> system::MutPartitionChunk<'t, A, C>
    for PartitionAccessor<'t, A, C, StorageParT>
where
    A: Archetype,
    C: Send + Sync + 'static,
    StorageParT: storage::PartitionChunked<'t, A::RawEntity, C>,
{
    type IntoIterChunksMut = impl Iterator<Item = (entity::TempRefChunk<'t, A>, &'t mut [C])>;

    fn into_iter_chunks_mut(self) -> Self::IntoIterChunksMut {
        self.partition.into_iter_chunks_mut().map(|(initial, data)| {
            (entity::TempRefChunk::new(initial, initial.add(data.len())), data)
        })
    }
}

pub(super) fn mut_owned_par_iter_mut<'t, A: Archetype, C: 'static>(
    partition: impl system::MutPartition<'t, A, C>,
    snapshot: &'t ealloc::Snapshot<A::RawEntity>,
) -> impl ParallelIterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
where
    C: comp::Must<A>,
{
    rayon::iter::split((partition, snapshot.as_slice()), |(partition, slice)| {
        let Some(midpt) = slice.midpoint_for_split() else { return ((partition, slice), None) };
        let (slice_left, slice_right) = slice.split_at(midpt);
        let (partition_left, partition_right) = partition.split_at(entity::TempRef::new(midpt));
        ((partition_left, slice_left), Some((partition_right, slice_right)))
    })
    .flat_map_iter(|(partition, _slice)| partition.into_iter_mut())
}

pub(super) fn mut_owned_par_iter_chunks_mut<'t, A: Archetype, C: 'static>(
    partition: impl system::MutPartitionChunk<'t, A, C>,
    snapshot: &'t ealloc::Snapshot<A::RawEntity>,
) -> impl ParallelIterator<Item = (entity::TempRefChunk<'t, A>, &'t mut [C])>
where
    C: comp::Must<A>,
{
    rayon::iter::split((partition, snapshot.as_slice()), |(partition, slice)| {
        let Some(midpt) = slice.midpoint_for_split() else { return ((partition, slice), None) };
        let (slice_left, slice_right) = slice.split_at(midpt);
        let (partition_left, partition_right) = partition.split_at(entity::TempRef::new(midpt));
        ((partition_left, slice_left), Some((partition_right, slice_right)))
    })
    .flat_map_iter(|(partition, _slice)| partition.into_iter_chunks_mut())
}
