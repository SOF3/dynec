use std::any::type_name;
use std::marker::PhantomData;
use std::sync::Arc;

use parking_lot::lock_api::ArcRwLockWriteGuard;
use parking_lot::RwLock;

use crate::world::rw::{self, isotope};
use crate::{comp, entity, system, Archetype, Storage as _};

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

    type SplitEntitiesAt<'t> = impl system::Mut<A, C> + 't
    where
        Self: 't;
    fn split_entities_at<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
    ) -> (Self::SplitEntitiesAt<'_>, Self::SplitEntitiesAt<'_>) {
        let (left, right) = self.storage.partition_at(entity.id());
        (
            rw::PartitionAccessor { storage: left, _ph: PhantomData },
            rw::PartitionAccessor { storage: right, _ph: PhantomData },
        )
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
