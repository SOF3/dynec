//! Component reading and writing.

use std::any::type_name;
use std::collections::HashMap;
use std::marker::PhantomData;

use rayon::prelude::ParallelIterator;

use super::typed;
use crate::entity::ealloc;
use crate::storage::Partition as _;
use crate::system::MutPartition as _;
use crate::util::DbgTypeId;
use crate::{comp, entity, storage, system, Archetype};

pub(crate) mod isotope;
pub(crate) mod simple;

/// Stores the component states in a world.
pub struct Components {
    pub(crate) archetypes: HashMap<DbgTypeId, Box<dyn typed::AnyTyped>>,
}

impl Components {
    /// Creates a dummy, empty component store used for testing.
    pub fn empty() -> Self { Self { archetypes: HashMap::new() } }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype<A: Archetype>(&self) -> &typed::Typed<A> {
        match self.archetypes.get(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any().downcast_ref().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                type_name::<A>()
            ),
        }
    }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype_mut<A: Archetype>(&mut self) -> &mut typed::Typed<A> {
        match self.archetypes.get_mut(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any_mut().downcast_mut().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                type_name::<A>()
            ),
        }
    }
}

struct PartitionAccessor<'t, A: Archetype, C, S> {
    partition: S,
    _ph:       PhantomData<(A, &'t C)>,
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

    type IterMutMove = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>;
    fn iter_mut_move(self) -> Self::IterMutMove {
        self.partition.iter_mut().map(|(entity, data)| (entity::TempRef::new(entity), data))
    }
}

fn mut_owned_par_iter_mut<'t, A: Archetype, C: 'static>(
    accessor: &'t mut impl system::MutFull<A, C>,
    snapshot: &'t ealloc::Snapshot<A::RawEntity>,
) -> impl ParallelIterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
where
    C: comp::Must<A>,
{
    let partition = accessor.as_partition();
    rayon::iter::split((partition, snapshot.as_slice()), |(partition, slice)| {
        let Some(midpt) = slice.midpoint_for_split() else { return ((partition, slice), None) };
        let (slice_left, slice_right) = slice.split_at(midpt);
        let (partition_left, partition_right) = partition.split_at(entity::TempRef::new(midpt));
        ((partition_left, slice_left), Some((partition_right, slice_right)))
    })
    .flat_map_iter(|(partition, _slice)| partition.iter_mut_move())
}

#[cfg(test)]
#[allow(clippy::extra_unused_type_parameters)] // macro magic
mod _assert {
    static_assertions::assert_impl_all!(super::Components: Send, Sync);
}
