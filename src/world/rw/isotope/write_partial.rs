use std::marker::PhantomData;
use std::ops;

use crate::comp::{self, discrim};
use crate::entity::ealloc;
use crate::{system, world, Archetype};

impl world::Components {
    /// Creates a writable, exclusive accessor to the given archetyped isotope component,
    /// initializing new discriminants if not previously created.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write_partial_isotope_storage<'t, A, C, DiscrimSet>(
        &'t self,
        discrims: &'t DiscrimSet,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> impl system::WriteIsotope<A, C, DiscrimSet::Key> + 't
    where
        A: Archetype,
        C: comp::Isotope<A>,
        DiscrimSet: discrim::Set<C::Discrim>,
    {
        let storage_map = self.isotope_storage_map::<A, C>();

        let storages = {
            // see documentation of storage_map.map for contention analysis.
            let mut map = storage_map.map.lock();

            discrims.map(|discrim| {
                let storage = map.get_or_create(discrim, snapshot.iter_allocated_chunks());
                super::own_write_isotope_storage::<A, C>(discrim, storage)
            })
        };

        super::IsotopeAccessor { storages, view: View(PhantomData), _ph: PhantomData }
    }
}

struct View<A, C, StorageRef, DiscrimMapped>(PhantomData<(A, C, StorageRef, DiscrimMapped)>);
impl<A, C, StorageRef, DiscrimMapped> super::StorageMapView<A, C>
    for View<A, C, StorageRef, DiscrimMapped>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::DerefMut<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim, Value = StorageRef>,
{
    type StorageRef = StorageRef;
    type DiscrimMapped = DiscrimMapped;

    fn view<'t>(
        &self,
        key: DiscrimMapped::Key,
        storages: &'t DiscrimMapped,
    ) -> Option<&'t StorageRef> {
        let storage = storages.get_by(key);
        match storage {
            Some(input) => Some(input),
            None => super::panic_invalid_key::<A, C>(key),
        }
    }
}
impl<A, C, StorageRef, DiscrimMapped> super::MutStorageMapView<A, C>
    for View<A, C, StorageRef, DiscrimMapped>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::DerefMut<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim, Value = StorageRef>,
{
    fn view_mut<'t>(
        &mut self,
        key: DiscrimMapped::Key,
        storages: &'t mut DiscrimMapped,
    ) -> &'t mut C::Storage
    where
        StorageRef: 't,
    {
        match storages.get_mut_by(key).map(|s| &mut **s) {
            Some(storage) => storage,
            None => panic!(
                "Cannot access isotope indexed by {key:?} because it is not in the list of \
                 requested discriminants",
            ),
        }
    }

    fn view_many<'u, const N: usize>(
        &mut self,
        keys: [DiscrimMapped::Key; N],
        storages: &'u mut DiscrimMapped,
    ) -> [&'u mut C::Storage; N]
    where
        StorageRef: 'u,
    {
        storages.get_mut_array_by(
            keys,
            |storage| -> &mut C::Storage { &mut *storage },
            |key| {
                panic!(
                    "Cannot access isotope indexed by {key:?} because it is not in the list of \
                     requested discriminants",
                )
            },
        )
    }
}
