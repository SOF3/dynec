use std::any::{type_name, TypeId};
use std::marker::PhantomData;
use std::sync::Arc;
use std::{fmt, ops};

use parking_lot::lock_api::ArcRwLockWriteGuard;
use parking_lot::{RawRwLock, RwLock};

use super::PartitionAccessor;
use crate::comp::{self, discrim};
use crate::storage::Chunked;
use crate::{entity, storage, system, util, world, Archetype, Storage};

mod read_full;
mod read_partial;
mod write_full;
mod write_partial;

impl world::Components {
    fn isotope_storage_map<A: Archetype, C: comp::Isotope<A>>(&self) -> &storage::IsotopeMap<A, C> {
        match self.archetype::<A>().isotope_storage_maps.get(&TypeId::of::<C>()) {
            Some(storage) => storage.downcast_ref::<C>(),
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                type_name::<A>(),
                type_name::<C>()
            ),
        }
    }

    /// Returns the isotope storage map for the type.
    /// Do not insert new discriminants to the returned map.
    fn isotope_storage_map_mut<A: Archetype, C: comp::Isotope<A>>(
        &mut self,
    ) -> &mut storage::IsotopeMapInner<A, C> {
        let typed = self.archetype_mut::<A>();
        let storage_map = match typed.isotope_storage_maps.get_mut(&TypeId::of::<C>()) {
            Some(map) => {
                Arc::get_mut(map).expect("map arc was leaked").downcast_mut::<C>().map.get_mut()
            }
            None => panic!(
                "The component {} cannot be retrieved because it is not used in any systems",
                type_name::<C>(),
            ),
        };
        storage_map
    }

    /// Iterate over all isotope entity components in offline mode.
    ///
    /// Requires a mutable reference to the world to ensure that the world is offline.
    pub fn iter_isotope<A: Archetype, C: comp::Isotope<A>>(
        &mut self,
    ) -> impl Iterator<Item = (C::Discrim, impl Iterator<Item = (entity::TempRef<'_, A>, &mut C)>)>
    {
        self.isotope_storage_map_mut::<A, C>().iter_mut().map(|(discrim, storage)| {
            (discrim, {
                let storage: &mut C::Storage =
                    Arc::get_mut(storage).expect("storage arc was leaked").get_mut();
                storage.iter_mut().map(|(entity, value)| (entity::TempRef::new(entity), value))
            })
        })
    }

    /// Gets a reference to an isotope entity component in offline mode.
    ///
    /// Requires a mutable reference to the world to ensure that the world is offline.
    pub fn get_isotope<A: Archetype, C: comp::Isotope<A>, E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        discrim: C::Discrim,
    ) -> Option<&mut C> {
        let storage = self.isotope_storage_map_mut::<A, C>().get_mut(discrim)?;
        let storage = Arc::get_mut(storage).expect("storage arc was leaked").get_mut();
        storage.get_mut(entity.id())
    }
}

type LockedIsotopeStorage<A, C> =
    ArcRwLockWriteGuard<RawRwLock, <C as comp::SimpleOrIsotope<A>>::Storage>;

fn own_read_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
    discrim: C::Discrim,
    storage: &Arc<RwLock<C::Storage>>,
) -> impl ops::Deref<Target = C::Storage> {
    let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
    match storage.try_read_arc() {
        Some(guard) => guard,
        None => panic!(
            "The component {}/{}/{:?} is currently used by another system. Maybe scheduler bug?",
            type_name::<A>(),
            type_name::<C>(),
            discrim,
        ),
    }
}

fn own_write_isotope_storage<A: Archetype, C: comp::Isotope<A>>(
    discrim: C::Discrim,
    storage: &Arc<RwLock<C::Storage>>,
) -> LockedIsotopeStorage<A, C> {
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

struct IsotopeAccessor<A, C, S, M, P> {
    /// Cloned arcs of the actual storage.
    storages:  M,
    processor: P,
    _ph:       PhantomData<(A, C, S)>,
}

fn panic_invalid_key<A, C>(key: impl fmt::Debug) -> ! {
    panic!(
        "The index {key:?} is not available in the isotope request for {}/{}",
        type_name::<A>(),
        type_name::<C>(),
    )
}

impl<A, C, StorageRef, DiscrimMapped, ProcT> IsotopeAccessor<A, C, StorageRef, DiscrimMapped, ProcT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim>,
    ProcT: StorageMapProcessorRef<Input = DiscrimMapped::Value, Output = StorageRef>,
{
    fn get_all_raw(
        &self,
        index: A::RawEntity,
    ) -> impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &C)> {
        self.storages.iter_values().filter_map(move |(discrim, storage)| {
            let storage = ProcT::admit(storage)?;
            let comp = storage.get(index)?;
            Some((discrim, comp))
        })
    }
}

/// Generalizes different implementations of storage maps to handle the logic of optional storages.
trait StorageMapProcessorRef {
    /// The type returned by `storages.get_by`.
    type Input;
    /// Derefs to the actual storage type.
    type Output;

    /// Processes the result of `storages.get_by`.
    ///
    /// Returns `Some` if the storage is valid,
    /// or `None` if the storage should be assumed empty.
    /// May panic if the storage cannot be empty.
    fn process<'t, D: fmt::Debug, F: FnOnce() -> D>(
        &self,
        input: Option<&'t Self::Input>,
        key: F,
    ) -> Option<&'t Self::Output>;
    /// Converts the input type to the output type.
    fn admit(input: &Self::Input) -> Option<&Self::Output>;
}

impl<A, C, StorageRef, DiscrimMapped, ProcT> system::ReadIsotope<A, C, DiscrimMapped::Key>
    for IsotopeAccessor<A, C, StorageRef, DiscrimMapped, ProcT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim>,
    ProcT: StorageMapProcessorRef<Input = DiscrimMapped::Value, Output = StorageRef>,
{
    fn try_get<E: entity::Ref<Archetype = A>>(
        &self,
        entity: E,
        key: DiscrimMapped::Key,
    ) -> Option<&C> {
        let storage: Option<&DiscrimMapped::Value> = self.storages.get_by(key);
        let storage: Option<&StorageRef> = self.processor.process(storage, || key);
        storage.and_then(|storage| storage.get(entity.id()))
    }

    type GetAll<'t> = impl Iterator<Item = (<C as comp::Isotope<A>>::Discrim, &'t C)> + 't where Self: 't;
    fn get_all<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Self::GetAll<'_> {
        let index: A::RawEntity = entity.id();
        self.get_all_raw(index)
    }

    type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
    where
        Self: 't;
    fn iter(&self, key: DiscrimMapped::Key) -> Self::Iter<'_> {
        let storage: Option<&DiscrimMapped::Value> = self.storages.get_by(key);
        let storage: Option<&StorageRef> = self.processor.process(storage, || key);
        storage.into_iter().flat_map(|storage| {
            storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
        })
    }

    type Split<'t> = impl system::Read<A, C> + 't
    where
        Self: 't;
    fn split<const N: usize>(&self, keys: [DiscrimMapped::Key; N]) -> [Self::Split<'_>; N] {
        struct With<A, C, K, R: ops::Deref> {
            accessor: R,
            discrim:  K,
            _ph:      PhantomData<(A, C)>,
        }

        impl<A, C, DiscrimMapKeyT, ReadIsotopeRef> system::Read<A, C>
            for With<A, C, DiscrimMapKeyT, ReadIsotopeRef>
        where
            A: Archetype,
            C: comp::Isotope<A>,
            DiscrimMapKeyT: fmt::Debug + Copy + 'static,
            ReadIsotopeRef: ops::Deref,
            <ReadIsotopeRef as ops::Deref>::Target: system::ReadIsotope<A, C, DiscrimMapKeyT>,
        {
            fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
                system::ReadIsotope::try_get(&*self.accessor, entity, self.discrim)
            }

            type Iter<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t C)>
            where
                Self: 't;
            fn iter(&self) -> Self::Iter<'_> {
                system::ReadIsotope::iter(&*self.accessor, self.discrim)
            }

            type DuplicateImmut<'t> = impl system::Read<A, C> + 't where Self: 't;
            fn duplicate_immut(&self) -> (Self::DuplicateImmut<'_>, Self::DuplicateImmut<'_>) {
                (
                    With {
                        accessor: util::DoubleDeref(&self.accessor),
                        discrim:  self.discrim,
                        _ph:      PhantomData,
                    },
                    With {
                        accessor: util::DoubleDeref(&self.accessor),
                        discrim:  self.discrim,
                        _ph:      PhantomData,
                    },
                )
            }
        }

        keys.map(|key| With { accessor: self, discrim: key, _ph: PhantomData })
    }
}

/// Determines the strategy to fetch a storage from the storage map.
/// The two implementors from `write_full_isotope_storage` and `write_partial_isotope_storage`
/// would either lazily create or panic on missing storage.
///
/// This trait is not symmetric to `StorageMapProcessorRef` because
/// the equivalent API may lead to Polonius compile errors.
trait MutStorageAccessor<A, C, StorageRef, DiscrimMapped>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim>,
{
    fn get_storage<'t>(
        &mut self,
        key: DiscrimMapped::Key,
        storages: &'t mut DiscrimMapped,
    ) -> &'t mut C::Storage
    where
        StorageRef: 't;

    fn get_storage_multi<'t, const N: usize>(
        &mut self,
        keys: [DiscrimMapped::Key; N],
        storages: &'t mut DiscrimMapped,
    ) -> [&'t mut C::Storage; N]
    where
        StorageRef: 't;
}

impl<A, C, StorageRef, DiscrimMapped, ProcT> system::WriteIsotope<A, C, DiscrimMapped::Key>
    for IsotopeAccessor<A, C, StorageRef, DiscrimMapped, ProcT>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim>,
    ProcT: StorageMapProcessorRef<Input = DiscrimMapped::Value, Output = StorageRef>
        + MutStorageAccessor<A, C, StorageRef, DiscrimMapped>,
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: DiscrimMapped::Key,
    ) -> Option<&mut C> {
        let storage = self.processor.get_storage(key, &mut self.storages);

        storage.get_mut(entity.id())
    }

    fn set<E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
        key: DiscrimMapped::Key,
        value: Option<C>,
    ) -> Option<C> {
        let storage = self.processor.get_storage(key, &mut self.storages);
        storage.set(entity.id(), value)
    }

    type IterMut<'t> = impl Iterator<Item = (entity::TempRef<'t, A>, &'t mut C)>
    where
        Self: 't;
    /// Iterates over mutable references to all components of a specific discriminant.
    fn iter_mut(&mut self, key: DiscrimMapped::Key) -> Self::IterMut<'_> {
        let storage = self.processor.get_storage(key, &mut self.storages);
        storage.iter_mut().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type SplitDiscrim<'t> = impl system::Write<A, C> + 't where Self: 't;
    fn split_isotopes<const N: usize>(
        &mut self,
        keys: [DiscrimMapped::Key; N],
    ) -> [Self::SplitDiscrim<'_>; N] {
        self.processor
            .get_storage_multi(keys, &mut self.storages)
            .map(|storage| SplitDiscrim { storage, _ph: PhantomData })
    }
}

struct SplitDiscrim<A: Archetype, C: comp::Isotope<A>, S: ops::Deref<Target = C::Storage>> {
    storage: S,
    _ph:     PhantomData<(A, C)>,
}

impl<A: Archetype, C: comp::Isotope<A>, S: ops::Deref<Target = C::Storage>> system::Read<A, C>
    for SplitDiscrim<A, C, S>
{
    fn try_get<E: entity::Ref<Archetype = A>>(&self, entity: E) -> Option<&C> {
        self.storage.get(entity.id())
    }

    type Iter<'u> = impl Iterator<Item = (entity::TempRef<'u, A>, &'u C)>
    where
        Self: 'u;
    fn iter(&self) -> Self::Iter<'_> {
        self.storage.iter().map(|(entity, comp)| (entity::TempRef::new(entity), comp))
    }

    type DuplicateImmut<'t> = impl system::Read<A, C> + 't where Self: 't;
    fn duplicate_immut(&self) -> (Self::DuplicateImmut<'_>, Self::DuplicateImmut<'_>) {
        (
            SplitDiscrim { storage: &*self.storage, _ph: PhantomData },
            SplitDiscrim { storage: &*self.storage, _ph: PhantomData },
        )
    }
}
impl<A, C, StorageRef> system::ReadChunk<A, C> for SplitDiscrim<A, C, StorageRef>
where
    A: Archetype,
    C: comp::Isotope<A>,
    C::Storage: storage::Chunked,
    StorageRef: ops::Deref<Target = C::Storage>,
{
    fn get_chunk(&self, chunk: entity::TempRefChunk<'_, A>) -> &[C]
    where
        C: comp::Must<A>,
    {
        self.storage.get_chunk(chunk.start, chunk.end).expect("chunk is not completely filled")
    }
}

impl<'t, A: Archetype, C: comp::Isotope<A>> system::Mut<A, C>
    for SplitDiscrim<A, C, &'t mut C::Storage>
{
    fn try_get_mut<E: entity::Ref<Archetype = A>>(&mut self, entity: E) -> Option<&mut C> {
        self.storage.get_mut(entity.id())
    }

    type IterMut<'u> = impl Iterator<Item = (entity::TempRef<'u, A>, &'u mut C)> + 'u where Self: 'u;
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
impl<'t, A: Archetype, C: comp::Isotope<A>> system::Write<A, C>
    for SplitDiscrim<A, C, &'t mut C::Storage>
{
    fn set<E: entity::Ref<Archetype = A>>(&mut self, entity: E, value: Option<C>) -> Option<C> {
        self.storage.set(entity.id(), value)
    }
}
impl<'t, A, C> system::WriteChunk<A, C> for SplitDiscrim<A, C, &'t mut C::Storage>
where
    A: Archetype,
    C: comp::Isotope<A>,
    C::Storage: storage::Chunked,
{
    fn get_chunk_mut(&mut self, chunk: entity::TempRefChunk<'_, A>) -> &mut [C]
    where
        C: comp::Must<A>,
    {
        self.storage.get_chunk_mut(chunk.start, chunk.end).expect("chunk is not completely filled")
    }
}
