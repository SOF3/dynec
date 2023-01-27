use std::any::{type_name, TypeId};
use std::marker::PhantomData;
use std::ops;

use parking_lot::{RwLockReadGuard, RwLockWriteGuard};

use super::PartitionAccessor;
use crate::storage::{self, Chunked};
use crate::{comp, entity, system, util, world, Archetype, Storage};

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

impl<S: ops::Deref> SimpleRw<S> {}

impl<A, C, StorageRef> system::Read<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
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
}
impl<A, C, StorageRef> system::ReadChunk<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
    C::Storage: storage::Chunked,
{
    fn get_chunk(&self, chunk: entity::TempRefChunk<'_, A>) -> &[C]
    where
        C: comp::Must<A>,
    {
        self.storage.get_chunk(chunk.start, chunk.end).expect("chunk is not completely filled")
    }
}
impl<A, C, StorageRef> system::ReadSimple<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
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

    type SplitEntitiesAt<'u> = impl system::Mut<A, C> + 'u where Self: 'u;
    fn split_entities_at<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
    ) -> (Self::SplitEntitiesAt<'_>, Self::SplitEntitiesAt<'_>) {
        let (left, right) = self.storage.partition_at(entity.id());
        (
            PartitionAccessor { storage: left, _ph: PhantomData },
            PartitionAccessor { storage: right, _ph: PhantomData },
        )
    }
}

impl<A, C, StorageRef> system::Write<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage>,
{
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C> {
        self.storage.set(entity.id(), value)
    }
}
impl<A, C, StorageRef> system::WriteChunk<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage>,
    C::Storage: storage::Chunked,
{
    fn get_chunk_mut(&mut self, chunk: entity::TempRefChunk<'_, A>) -> &mut [C]
    where
        C: comp::Must<A>,
    {
        self.storage.get_chunk_mut(chunk.start, chunk.end).expect("chunk is not completely filled")
    }
}
impl<A, C, StorageRef> system::WriteSimple<A, C> for SimpleRw<StorageRef>
where
    A: Archetype,
    C: comp::Simple<A>,
    StorageRef: ops::DerefMut<Target = C::Storage>,
{
    fn access_chunk_mut(&mut self) -> system::accessor::MustWriteChunkSimple<'_, A, C> {
        system::accessor::MustWriteChunkSimple { storage: &mut self.storage }
    }
}
