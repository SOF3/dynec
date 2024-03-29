use std::any::type_name;
use std::marker::PhantomData;

use parking_lot::MutexGuard;

use crate::comp::discrim::{FullMap as _, Mapped as _};
use crate::comp::{self, Discrim};
use crate::entity::ealloc;
use crate::system::access::{StorageMap, StorageMapMut};
use crate::world::rw::isotope;
use crate::{storage, system, world, Archetype};

/// Provides access to an isotope component in a specific archetype.
///
/// Getters require a mutable receiver to allow lazy initialization of new discriminants.
/// Consider [splitting](system::AccessIsotope::split) accessors,
/// which returns a [`system::AccessSingle`] with a shared receiver.
/// If it can be asserted that no uninitialized discriminants will be encountered,
/// use with [`known_discrims`](system::AccessIsotope::known_discrims).
pub type WriteIsotopeFull<'t, A, C> = system::AccessIsotope<A, C, Storages<'t, A, C>>;

impl world::Components {
    /// Mutably access all discriminants of an isotope storage,
    /// lazily initializing new isotopes during usage.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is accessing the same archetyped component.
    pub fn write_full_isotope_storage<A, C>(
        &self,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> WriteIsotopeFull<A, C>
    where
        A: Archetype,
        C: comp::Isotope<A>,
    {
        let storage_map = isotope::storage_map::<A, C>(self);

        // Lock the entire map since no other systems can access it
        let Some(full_map) = storage_map.map.try_lock() else {
            panic!(
                "Cannot access full isotope storage of {}/{} mutably because another thread is \
                 locking it. Scheduler error?",
                type_name::<A>(),
                type_name::<C>(),
            )
        };
        let accessor_storages: <C::Discrim as Discrim>::FullMap<_> = full_map
            .map()
            .iter()
            .map(|(&discrim, storage)| {
                (discrim, isotope::write::own_storage::<A, C>(discrim, storage))
            })
            .collect();

        system::AccessIsotope::new(Storages {
            full_map,
            accessor_storages,
            snapshot,
            _ph: PhantomData,
        })
    }
}

pub struct Storages<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    full_map:          MutexGuard<'u, storage::IsotopeMapInner<A, C>>,
    accessor_storages: <C::Discrim as Discrim>::FullMap<isotope::write::LockedStorage<A, C>>,
    snapshot:          ealloc::Snapshot<A::RawEntity>,
    _ph:               PhantomData<(A, C)>,
}

impl<'u, A, C> StorageMap<A, C> for Storages<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    type Key = C::Discrim;

    fn get_storage(&mut self, discrim: C::Discrim) -> &C::Storage {
        self.accessor_storages.get_by_or_insert(discrim, || {
            let storage =
                self.full_map.get_or_create(discrim, self.snapshot.iter_allocated_chunks());
            isotope::write::own_storage::<A, C>(discrim, storage)
        })
    }

    fn get_storage_many<const N: usize>(
        &mut self,
        discrims: [C::Discrim; N],
    ) -> [&<C>::Storage; N] {
        self.accessor_storages.get_by_or_insert_array(
            discrims,
            |discrim| {
                let storage =
                    self.full_map.get_or_create(discrim, self.snapshot.iter_allocated_chunks());
                isotope::write::own_storage::<A, C>(discrim, storage)
            },
            |_discrim, storage| &**storage,
        )
    }

    type IterKeys<'t> = impl Iterator<Item = (Self::Key, C::Discrim)> + 't
    where
        Self: 't;
    fn iter_keys(&self) -> Self::IterKeys<'_> {
        self.accessor_storages.iter_mapped().map(|(_, discrim, _)| (discrim, discrim))
    }

    type IterValue = isotope::write::LockedStorage<A, C>;
    type IterValues<'t> = impl Iterator<Item = (Self::Key, C::Discrim, &'t Self::IterValue)> + 't
    where
        Self: 't;
    fn iter_values(&self) -> Self::IterValues<'_> {
        self.accessor_storages
            .iter_mapped()
            .map(|(_, discrim, storage)| (discrim, discrim, storage))
    }
}

impl<'u, A, C> StorageMapMut<A, C> for Storages<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    fn get_storage_mut(&mut self, discrim: C::Discrim) -> &mut C::Storage {
        self.accessor_storages.get_by_or_insert(discrim, || {
            let storage =
                self.full_map.get_or_create(discrim, self.snapshot.iter_allocated_chunks());
            isotope::write::own_storage::<A, C>(discrim, storage)
        })
    }

    fn get_storage_mut_many<const N: usize>(
        &mut self,
        discrims: [C::Discrim; N],
    ) -> [&mut C::Storage; N] {
        self.accessor_storages.get_by_or_insert_array(
            discrims,
            |discrim| {
                let storage =
                    self.full_map.get_or_create(discrim, self.snapshot.iter_allocated_chunks());
                isotope::write::own_storage::<A, C>(discrim, storage)
            },
            |_discrim, storage| &mut **storage,
        )
    }
}
