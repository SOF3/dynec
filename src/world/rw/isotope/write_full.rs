use std::fmt;
use std::marker::PhantomData;

use parking_lot::RwLockWriteGuard;

use crate::comp::{self, discrim, Discrim};
use crate::entity::ealloc;
use crate::{storage, system, world, Archetype};

impl world::Components {
    /// Creates a writable, exclusive accessor to all discriminants of the given archetyped isotope component,
    /// with the capability of initializing creating new discriminants not previously created.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is accessing the same archetyped component.
    pub fn write_full_isotope_storage<A, C>(
        &self,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> impl system::WriteIsotope<A, C> + '_
    where
        A: Archetype,
        C: comp::Isotope<A>,
    {
        let storage_map = self.isotope_storage_map::<A, C>();

        let full_map: RwLockWriteGuard<'_, storage::IsotopeMapInner<A, C>> =
            storage_map.map.write();

        let accessor_storages: <C::Discrim as Discrim>::FullMap<super::LockedIsotopeStorage<A, C>> =
            full_map
                .map()
                .iter()
                .map(|(&discrim, storage)| {
                    (discrim, super::own_write_isotope_storage::<A, C>(discrim, storage))
                })
                .collect();

        super::IsotopeAccessor::<A, C, super::LockedIsotopeStorage<A, C>, _, _> {
            storages:  accessor_storages,
            processor: Proc::<'_, A, C> { persistent_map: full_map, snapshot, _ph: PhantomData },
            _ph:       PhantomData,
        }
    }
}

struct Proc<'t, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    /// The actual map that persists isotope storages over multiple systems.
    persistent_map: RwLockWriteGuard<'t, storage::IsotopeMapInner<A, C>>,
    snapshot:       ealloc::Snapshot<A::RawEntity>,
    _ph:            PhantomData<(A, C)>,
}
impl<'t, A, C> super::StorageMapProcessorRef for Proc<'t, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    type Input = super::LockedIsotopeStorage<A, C>;
    type Output = Self::Input;
    fn process<'u, D: fmt::Debug, F: FnOnce() -> D>(
        &self,
        input: Option<&'u Self::Input>,
        _: F,
    ) -> Option<&'u Self::Input> {
        input
    }
    fn admit(input: &Self::Input) -> Option<&Self::Input> { Some(input) }
}
impl<'t, A, C, M> super::MutStorageAccessor<A, C, super::LockedIsotopeStorage<A, C>, M>
    for Proc<'t, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
    M: discrim::FullMap<
        Discrim = C::Discrim,
        Key = C::Discrim,
        Value = super::LockedIsotopeStorage<A, C>,
    >,
{
    fn get_storage<'u>(&mut self, discrim: C::Discrim, storages: &'u mut M) -> &'u mut C::Storage
    where
        super::LockedIsotopeStorage<A, C>: 'u,
    {
        storages.get_by_or_insert(discrim, || {
            let storage =
                self.persistent_map.get_or_create(discrim, self.snapshot.iter_allocated_chunks());
            super::own_write_isotope_storage::<A, C>(discrim, storage)
        })
    }

    fn get_storage_multi<'u, const N: usize>(
        &mut self,
        keys: [C::Discrim; N],
        storages: &'u mut M,
    ) -> [&'u mut C::Storage; N]
    where
        super::LockedIsotopeStorage<A, C>: 'u,
    {
        storages.get_by_or_insert_array(
            keys,
            |discrim| {
                let storage = self
                    .persistent_map
                    .get_or_create(discrim, self.snapshot.iter_allocated_chunks());
                super::own_write_isotope_storage::<A, C>(discrim, storage)
            },
            |storage| &mut **storage,
        )
    }
}
