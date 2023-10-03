use std::marker::PhantomData;

use crate::comp::discrim::{self, Mapped as _};
use crate::entity::ealloc;
use crate::system::access::{PartialStorageMap, StorageMap};
use crate::world::rw::isotope;
use crate::{comp, system, world, Archetype};

/// Provides access to an isotope component in a specific archetype.
///
/// `DiscrimSetKey` is the type used to index the discriminant,
/// depending on the type of discriminant set given.
/// For vec and array discriminant sets, this should be `usize`.
/// For partial isotope access, `K` is usually `usize`.
/// For full isotope access, `K` is the discriminant type.
///
/// To share the same API as [`ReadIsotopeFull`](system::ReadIsotopeFull),
/// immutable getters still require `&mut self`,
/// but there are `*_ref` variants for these functions that just require `&self`.
pub type ReadIsotopePartial<A, C, DiscrimSet = Vec<<C as comp::Isotope<A>>::Discrim>> =
    system::AccessIsotope<A, C, Storages<A, C, DiscrimSet>>;

impl world::Components {
    /// Immutably access the requested discriminants of an isotope storage,
    /// lazily initializing new isotopes in `discrims` immediately.
    ///
    /// The return value provides `*_ref` getters,
    /// allowing shared use of this accessor on multiple workers.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read_partial_isotope_storage<'t, A, C, DiscrimSet>(
        &'t self,
        discrims: &'t DiscrimSet,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> ReadIsotopePartial<A, C, DiscrimSet>
    where
        A: Archetype,
        C: comp::Isotope<A>,
        DiscrimSet: discrim::Set<C::Discrim>,
    {
        let storage_map = isotope::storage_map::<A, C>(self);

        let storages = {
            let mut map = storage_map.map.lock();

            discrims.map(|discrim| {
                let storage = map.get_or_create(discrim, snapshot.iter_allocated_chunks());
                isotope::read::own_storage::<A, C>(discrim, storage)
            })
        };

        system::AccessIsotope::new(Storages::<A, C, DiscrimSet> { storages, _ph: PhantomData })
    }
}

pub struct Storages<A, C, DiscrimSet>
where
    A: Archetype,
    C: comp::Isotope<A>,
    DiscrimSet: discrim::Set<C::Discrim>,
{
    storages: DiscrimSet::Mapped<isotope::read::LockedStorage<A, C>>,
    _ph:      PhantomData<(A, C)>,
}

impl<A, C, DiscrimSet> StorageMap<A, C> for Storages<A, C, DiscrimSet>
where
    A: Archetype,
    C: comp::Isotope<A>,
    DiscrimSet: discrim::Set<C::Discrim>,
{
    type Key = DiscrimSet::Key;

    fn get_storage(&mut self, key: Self::Key) -> &C::Storage {
        match self.storages.get_by(key) {
            Some(storage) => storage,
            None => isotope::panic_invalid_key::<A, C>(key),
        }
    }

    fn get_storage_many<const N: usize>(&mut self, keys: [Self::Key; N]) -> [&C::Storage; N] {
        self.storages.get_mut_array_by(
            keys,
            |storage| &**storage,
            |key| isotope::panic_invalid_key::<A, C>(key),
        )
    }

    type IterKeys<'t> = impl Iterator<Item = (DiscrimSet::Key, C::Discrim)> + 't;
    fn iter_keys(&self) -> Self::IterKeys<'_> {
        self.storages.iter_mapped().map(|(key, discrim, _)| (key, discrim))
    }

    type IterValue = isotope::read::LockedStorage<A, C>;
    type IterValues<'t> = impl Iterator<Item = (Self::Key, C::Discrim, &'t Self::IterValue)> + 't
    where
        Self: 't;
    fn iter_values(&self) -> Self::IterValues<'_> {
        self.storages.iter_mapped().map(|(key, discrim, storage)| (key, discrim, storage))
    }
}

impl<A, C, DiscrimSet> PartialStorageMap<A, C> for Storages<A, C, DiscrimSet>
where
    A: Archetype,
    C: comp::Isotope<A>,
    DiscrimSet: discrim::Set<C::Discrim>,
{
    fn get_storage_ref(&self, key: Self::Key) -> &<C>::Storage {
        match self.storages.get_by(key) {
            Some(storage) => storage,
            None => isotope::panic_invalid_key::<A, C>(key),
        }
    }
}
