use std::any::type_name;
use std::marker::PhantomData;
use std::sync::Arc;

use parking_lot::lock_api::ArcRwLockWriteGuard;
use parking_lot::RwLock;
use rayon::prelude::ParallelIterator;

use crate::entity::ealloc;
use crate::storage::Chunked;
use crate::world::rw::{self, isotope};
use crate::{comp, entity, storage, system, Archetype, Storage};

pub(super) mod full;
pub(super) mod partial;

type LockedStorage<A, C> =
    ArcRwLockWriteGuard<parking_lot::RawRwLock, <C as comp::SimpleOrIsotope<A>>::Storage>;

fn own_storage<A: Archetype, C: comp::Isotope<A>>(
    discrim: C::Discrim,
    storage: &Arc<RwLock<C::Storage>>,
) -> LockedStorage<A, C> {
    let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
    match storage.try_write_arc() {
        Some(guard) => guard,
        None => panic!(
            "The component {}/{}/{:?} is currently used by another system. Maybe scheduler bug?",
            type_name::<A>(),
            type_name::<C>(),
            discrim,
        ),
    }
}

pub(super) trait StorageGetMut<A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
    Self: isotope::read::StorageGet<A, C>,
{
    /// Retrieves a storage by key.
    /// Panics if the key is not supported.
    ///
    /// For partial accessors, this should return the storage
    /// for the discriminant indexed by the key,
    /// or panic if the key is out of bounds.
    ///
    /// For full accessors, this should return the storage for the given discriminant,
    /// or initialize the storage lazily.
    fn get_storage_mut(&mut self, key: Self::Key) -> &mut C::Storage;

    /// Retrieves storages by disjoint keys.
    /// Panics if any key is not supported or is equal to another key.
    fn get_storage_mut_many<const N: usize>(
        &mut self,
        keys: [Self::Key; N],
    ) -> [&mut C::Storage; N];
}

impl<A, C, GetterT> system::WriteIsotope<A, C, GetterT::Key> for isotope::Base<GetterT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    GetterT: StorageGetMut<A, C>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: GetterT::Key,
    ) -> Option<&mut C> {
        let storage = self.getter.get_storage_mut(key);
        storage.get_mut(entity.id())
    }

    fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: GetterT::Key,
        value: Option<C>,
    ) -> Option<C> {
        let storage = self.getter.get_storage_mut(key);
        storage.set(entity.id(), value)
    }

    type IterMut<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    fn iter_mut(&mut self, key: GetterT::Key) -> Self::IterMut<'_> {
        let storage = self.getter.get_storage_mut(key);
        storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type SplitDiscrim<'t> = impl system::Write<A, C> + 't
    where
        Self: 't;
    fn split_isotopes<const N: usize>(
        &mut self,
        keys: [GetterT::Key; N],
    ) -> [Self::SplitDiscrim<'_>; N] {
        let storages = self.getter.get_storage_mut_many(keys);
        storages.map(|storage| SplitWriter { storage, _ph: PhantomData })
    }
}

struct SplitWriter<'t, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    storage: &'t mut C::Storage,
    _ph:     PhantomData<(A, C)>,
}

impl<'u, A, C> system::Read<A, C> for SplitWriter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
        self.storage.get(entity.id())
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)> where Self: 't;

    fn iter(&self) -> Self::Iter<'_> {
        self.storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type DuplicateImmut<'t> = impl system::Read<A, C> + 't where Self: 't;

    fn duplicate_immut(&self) -> (Self::DuplicateImmut<'_>, Self::DuplicateImmut<'_>) {
        (
            isotope::read::SplitReader { storage: self.storage, _ph: PhantomData },
            isotope::read::SplitReader { storage: self.storage, _ph: PhantomData },
        )
    }

    type ParIter<'t> = impl rayon::iter::ParallelIterator<Item = (entity::TempRef<'t, A>, &'t C)> where Self: 't, C: comp::Must<A>;
    fn par_iter<'t>(
        &'t self,
        snapshot: &'t ealloc::Snapshot<<A as Archetype>::RawEntity>,
    ) -> Self::ParIter<'t>
    where
        C: comp::Must<A>,
    {
        rayon::iter::split(snapshot.as_slice(), |slice| slice.split()).flat_map_iter(|slice| {
            slice.iter_chunks().flat_map(<A::RawEntity as entity::Raw>::range).map(|id| {
                let entity = entity::TempRef::new(id);
                let data = self.get(entity);
                (entity, data)
            })
        })
    }
}

impl<'u, A, C> system::ReadChunk<A, C> for SplitWriter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A> + comp::Must<A>,
    C::Storage: storage::Chunked,
{
    fn get_chunk(&self, chunk: entity::TempRefChunk<'_, A>) -> &[C] {
        self.storage.get_chunk(chunk.start, chunk.end).expect("chunk is not completely filled")
    }

    type ParIterChunks<'t> = impl rayon::iter::ParallelIterator<Item = (entity::TempRefChunk<'t, A>, &'t [C])> where Self: 't;
    fn par_iter_chunks<'t>(
        &'t self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> Self::ParIterChunks<'t> {
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

impl<'u, A, C> system::Mut<A, C> for SplitWriter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
        self.storage.get_mut(entity.id())
    }

    type IterMut<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}

impl<'u, A, C> system::MutFull<A, C> for SplitWriter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    type Partition<'t> =
        rw::PartitionAccessor<'t, A, C, <C::Storage as Storage>::Partition<'t>> where Self: 't;
    fn as_partition(&mut self) -> Self::Partition<'_> {
        rw::PartitionAccessor { partition: self.storage.as_partition(), _ph: PhantomData }
    }

    type ParIterMut<'t> = impl ParallelIterator<Item = (entity::TempRef<'t, A>, &'t mut C)> where Self: 't, C: comp::Must<A>;
    fn par_iter_mut<'t>(
        &'t mut self,
        snapshot: &'t ealloc::Snapshot<A::RawEntity>,
    ) -> Self::ParIterMut<'t>
    where
        C: comp::Must<A>,
    {
        rw::mut_owned_par_iter_mut(self.as_partition(), snapshot)
    }
}

impl<'u, A, C> system::MutChunk<A, C> for SplitWriter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A> + comp::Must<A>,
    C::Storage: storage::Chunked,
{
    fn get_chunk_mut(&mut self, chunk: entity::TempRefChunk<'_, A>) -> &mut [C]
    where
        C: comp::Must<A>,
    {
        self.storage.get_chunk_mut(chunk.start, chunk.end).expect("chunk is not completely filled")
    }
}

impl<'u, A, C> system::MutFullChunk<A, C> for SplitWriter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A> + comp::Must<A>,
    C::Storage: storage::Chunked,
{
    type Partition<'t> = impl system::MutPartitionChunk<'t, A, C>
    where
        Self: 't;
    fn as_partition_chunk(&mut self) -> Self::Partition<'_> {
        rw::PartitionAccessor {
            partition: self.storage.as_partition_chunk(),
            _ph:       PhantomData,
        }
    }

    type ParIterChunksMut<'t> = impl ParallelIterator<Item = (entity::TempRefChunk<'t, A>, &'t mut [C])>
    where
        Self: 't,
        C: comp::Must<A>;
    fn par_iter_chunks_mut<'t>(
        &'t mut self,
        snapshot: &'t ealloc::Snapshot<<A as Archetype>::RawEntity>,
    ) -> Self::ParIterChunksMut<'t>
    where
        C: comp::Must<A>,
    {
        rw::mut_owned_par_iter_chunks_mut(self.as_partition_chunk(), snapshot)
    }
}

impl<'u, A, C> system::Write<A, C> for SplitWriter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C> {
        self.storage.set(entity.id(), value)
    }
}
