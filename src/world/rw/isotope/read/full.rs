use std::marker::PhantomData;

use crate::comp::discrim::{FullMap, Mapped as _};
use crate::comp::Discrim;
use crate::entity::ealloc;
use crate::system::access::StorageMap;
use crate::world::rw::isotope;
use crate::{comp, storage, system, world, Archetype};

/// Provides access to an isotope component in a specific archetype.
///
/// Getters require a mutable receiver to allow lazy initialization of new discriminants.
/// Consider [splitting](system::AccessIsotope::split) accessors,
/// which returns a [`system::AccessSingle`] with a shared receiver.
/// If it can be asserted that no uninitialized discriminants will be encountered,
/// use with [`known_discrims`](system::AccessIsotope::known_discrims).
pub type ReadIsotopeFull<'t, A, C> = system::AccessIsotope<A, C, Storages<'t, A, C>>;

impl world::Components {
    /// Immutably access all discriminants of an isotope storage,
    /// lazily initializing new isotopes during usage.
    ///
    /// The returned storage requires mutable receiver
    /// in order to lazily initialize new isotopes,
    /// but multiple immutable accessors can still run concurrently
    /// with lock contention only occurring when new discriminants are encountered.
    /// See the documentation of [`ReadIsotopeFull`](system::ReadIsotopeFull) for details.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_full_isotope_storage<A, C>(
        &self,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> ReadIsotopeFull<A, C>
    where
        A: Archetype,
        C: comp::Isotope<A>,
    {
        let storage_map = isotope::storage_map::<A, C>(self);

        let accessor_storages: <C::Discrim as Discrim>::FullMap<_> = {
            // This block blocks all other systems reading the same storage
            // for the period of cloning each isotope.
            let full_map = storage_map.map.lock();
            full_map
                .map()
                .iter()
                .map(|(&discrim, storage)| {
                    (discrim, isotope::read::own_storage::<A, C>(discrim, storage))
                })
                .collect()
        };

        system::AccessIsotope::new(Storages {
            full_map: storage_map,
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
    full_map:          &'u storage::IsotopeMap<A, C>,
    accessor_storages: <C::Discrim as Discrim>::FullMap<isotope::read::LockedStorage<A, C>>,
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
            let mut full_map = self.full_map.map.lock();
            let storage = full_map.get_or_create(discrim, self.snapshot.iter_allocated_chunks());
            isotope::read::own_storage::<A, C>(discrim, storage)
        })
    }

    fn get_storage_many<const N: usize>(
        &mut self,
        discrims: [C::Discrim; N],
    ) -> [&<C>::Storage; N] {
        self.accessor_storages.get_by_or_insert_array(
            discrims,
            |discrim| {
                let mut full_map = self.full_map.map.lock();
                let storage =
                    full_map.get_or_create(discrim, self.snapshot.iter_allocated_chunks());
                isotope::read::own_storage::<A, C>(discrim, storage)
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

    type IterValue = isotope::read::LockedStorage<A, C>;
    type IterValues<'t> = impl Iterator<Item = (Self::Key, C::Discrim, &'t Self::IterValue)> + 't
    where
        Self: 't;
    fn iter_values(&self) -> Self::IterValues<'_> {
        self.accessor_storages
            .iter_mapped()
            .map(|(_, discrim, storage)| (discrim, discrim, storage))
    }
}
