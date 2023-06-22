use std::marker::PhantomData;

use crate::comp::discrim::{FullMap, Mapped as _};
use crate::comp::Discrim;
use crate::entity::ealloc;
use crate::world::rw::isotope;
use crate::{comp, storage, system, world, Archetype};

impl world::Components {
    /// Immutably access all discriminants of an isotope storage,
    /// lazily initializing new isotopes during usage.
    ///
    /// The returned storage requires mutable receiver
    /// in order to lazily initialize new isotopes,
    /// but multiple immutable accessors can still run concurrently
    /// with lock contention only occurring when new discriminants are encountered.
    /// See the documentation of [`ReadIsotope`] for details.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_full_isotope_storage<A, C>(
        &self,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> impl system::ReadIsotope<A, C> + '_
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

        isotope::Base {
            getter: Getter { full_map: storage_map, accessor_storages, snapshot, _ph: PhantomData },
        }
    }
}

struct Getter<'u, A, C>
where
    A: Archetype,
    C: comp::Isotope<A>,
{
    full_map:          &'u storage::IsotopeMap<A, C>,
    accessor_storages: <C::Discrim as Discrim>::FullMap<isotope::read::LockedStorage<A, C>>,
    snapshot:          ealloc::Snapshot<A::RawEntity>,
    _ph:               PhantomData<(A, C)>,
}

impl<'u, A, C> isotope::read::StorageGet<A, C> for Getter<'u, A, C>
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
