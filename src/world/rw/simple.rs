use std::any::{type_name, TypeId};
use std::marker::PhantomData;
use std::ops;

use parking_lot::{RwLockReadGuard, RwLockWriteGuard};
use rayon::prelude::ParallelIterator;

use crate::entity::ealloc;
use crate::storage::{self, Chunked};
use crate::world::{self, rw};
use crate::{comp, entity, system, util, Archetype, Storage};

impl world::Components {
    /// Creates a read-only, shared accessor to the given archetyped simple component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_simple_storage<A: Archetype, C: comp::Simple<A>>(
        &self,
    ) -> impl system::ReadSimple<A, C> + '_ {
        let storage = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = match storage.storage.try_read() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently exclusively locked by another system. Maybe \
                 scheduler bug?",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = RwLockReadGuard::map(guard, |storage| storage.downcast_ref::<C>());

        SimpleRw { storage: guard }
    }

    /// Creates a writable, exclusive accessor to the given archetyped simple component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is accessing the same archetyped component.
    pub fn write_simple_storage<A: Archetype, C: comp::Simple<A>>(
        &self,
    ) -> impl system::WriteSimple<A, C> + '_ {
        let storage = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = match storage.storage.try_write() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently used by another system. Maybe scheduler bug?",
                type_name::<A>(),
                type_name::<C>()
            ),
        };
        let guard = RwLockWriteGuard::map(guard, |storage| storage.downcast_mut::<C>());

        SimpleRw { storage: guard }
    }

    /// Iterates over all simple entity components in offline mode.
    ///
    /// Requires a mutable reference to the world to ensure that the world is offline.
    pub fn iter_simple<A: Archetype, C: comp::Simple<A>>(
        &mut self,
    ) -> impl Iterator<Item = (entity::TempRef<'_, A>, &mut C)> {
        let typed = self.archetype_mut::<A>();
        let storage = match typed.simple_storages.get_mut(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {} cannot be retrieved because it is not used in any systems",
                type_name::<C>()
            ),
        };
        let storage = storage.get_storage::<C>();
        storage.iter_mut().map(|(entity, value)| (entity::TempRef::new(entity), value))
    }

    /// Gets a reference to a simple entity component in offline mode.
    ///
    /// Requires a mutable reference to the world to ensure that the world is offline.
    pub fn get_simple<A: Archetype, C: comp::Simple<A>, E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
    ) -> Option<&mut C> {
        let typed = self.archetype_mut::<A>();
        let storage = match typed.simple_storages.get_mut(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {} cannot be retrieved because it is not used in any systems",
                type_name::<C>()
            ),
        };
        let storage = storage.get_storage::<C>();
        storage.get_mut(entity.id())
    }
}

#[derive(Clone, Copy)]
struct SimpleRw<S> {
    // S is a MappedRwLock(Read|Write)Guard<C::Storage>
    storage: S,
}

impl<A, C, StorageRef> system::Read<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::Deref<Target = C::Storage> + Sync,
{
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
        self.storage.get(entity.id())
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)> where Self: 't;
    fn iter(&self) -> Self::Iter<'_> {
        self.storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type DuplicateImmut<'t> = SimpleRw<util::DoubleDeref<&'t StorageRef>> where Self: 't;
    fn duplicate_immut(
        &self,
    ) -> (SimpleRw<util::DoubleDeref<&'_ StorageRef>>, SimpleRw<util::DoubleDeref<&'_ StorageRef>>)
    {
        let dup = SimpleRw { storage: util::DoubleDeref(&self.storage) };
        (dup, dup)
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

impl<A, C, StorageRef> system::ReadChunk<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A> + comp::Must<A>,
    StorageRef: ops::Deref<Target = C::Storage> + Sync,
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

impl<A, C, StorageRef> system::ReadSimple<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::Deref<Target = C::Storage> + Sync,
{
    fn access_chunk(&self) -> system::accessor::MustReadChunkSimple<A, C> {
        system::accessor::MustReadChunkSimple { storage: &self.storage }
    }
}

impl<A, C, StorageRef> system::Mut<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
        self.storage.get_mut(entity.id())
    }

    type IterMut<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)> where Self: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }
}

impl<A, C, StorageRef> system::MutFull<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage>,
{
    type Partition<'t> = rw::PartitionAccessor<'t, A, C, <C::Storage as Storage>::Partition<'t>> where Self: 't;
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

impl<A, C, StorageRef> system::MutChunk<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage> + Sync,
    C::Storage: storage::Chunked,
{
    fn get_chunk_mut(&mut self, chunk: entity::TempRefChunk<'_, A>) -> &mut [C]
    where
        C: comp::Must<A>,
    {
        self.storage.get_chunk_mut(chunk.start, chunk.end).expect("chunk is not completely filled")
    }
}

impl<A, C, StorageRef> system::MutFullChunk<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage> + Sync,
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

impl<A, C, StorageRef> system::Write<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage> + Sync,
{
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C> {
        self.storage.set(entity.id(), value)
    }
}

impl<A, C, StorageRef> system::WriteSimple<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage> + Sync,
{
    fn access_chunk_mut(&mut self) -> system::accessor::MustWriteChunkSimple<'_, A, C> {
        system::accessor::MustWriteChunkSimple { storage: &mut self.storage }
    }
}
