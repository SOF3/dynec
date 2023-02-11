use std::marker::PhantomData;
use std::ops;

use crate::comp::{self, discrim};
use crate::entity::ealloc;
use crate::{system, world, Archetype};

impl world::Components {
    /// Creates a read-only, shared accessor to specific discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any of the requested discriminants.
    pub fn read_partial_isotope_storage<'t, A, C, DiscrimSet>(
        &'t self,
        discrims: &'t DiscrimSet,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> impl system::ReadIsotope<A, C, DiscrimSet::Key> + 't
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
                super::own_read_isotope_storage::<A, C>(discrim, storage)
            })
        };

        super::IsotopeAccessor { storages, view: View::<A, C, _, _>(PhantomData), _ph: PhantomData }
    }
}

struct View<A, C, StorageRef, DiscrimMapped>(PhantomData<(A, C, StorageRef, DiscrimMapped)>);
impl<A, C, StorageRef, DiscrimMapped> super::StorageMapView<A, C>
    for View<A, C, StorageRef, DiscrimMapped>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim, Value = StorageRef>,
{
    type StorageRef = StorageRef;
    type DiscrimMapped = DiscrimMapped;

    fn view<'t>(
        &self,
        key: DiscrimMapped::Key,
        storages: &'t DiscrimMapped,
    ) -> Option<&'t StorageRef> {
        match storages.get_by(key) {
            Some(storage) => Some(storage),
            None => super::panic_invalid_key::<A, C>(key),
        }
    }
}
