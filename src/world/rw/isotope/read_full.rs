use std::marker::PhantomData;
use std::ops;

use crate::comp::{self, discrim, Discrim};
use crate::entity::ealloc;
use crate::{storage, system, world, Archetype};

impl world::Components {
    /// Creates a read-only, shared accessor to all discriminants of the given archetyped isotope component.
    ///
    /// # Panics
    /// - if the archetyped component is not used in any systems.
    /// - if another thread is exclusively accessing any discriminants of the isotope component.
    pub fn read_full_isotope_storage<A, C>(
        &self,
        snapshot: ealloc::Snapshot<A::RawEntity>,
    ) -> impl system::ReadIsotope<A, C> + '_
    where
        A: Archetype,
        C: comp::Isotope<A>,
    {
        let storage_map = self.isotope_storage_map::<A, C>();

        let storages: <C::Discrim as Discrim>::FullMap<_> = {
            // see documentation of storage_map.map for contention analysis.
            let map = storage_map.map.lock();

            map.map()
                .iter()
                .map(|(&discrim, storage)| {
                    (discrim, super::own_read_isotope_storage::<A, C>(discrim, storage))
                })
                .collect()
        };

        super::IsotopeAccessor {
            storages,
            view: View { snapshot, map: storage_map, _ph: PhantomData },
            _ph: PhantomData,
        }
    }
}

struct View<'t, S, A: Archetype, C: comp::Isotope<A>, DiscrimMapped> {
    snapshot: ealloc::Snapshot<A::RawEntity>,
    map:      &'t storage::IsotopeMap<A, C>,
    _ph:      PhantomData<(S, DiscrimMapped)>,
}
impl<'t, A, C, StorageRef, DiscrimMapped> super::StorageMapView<A, C>
    for View<'t, StorageRef, A, C, DiscrimMapped>
where
    A: Archetype,
    C: comp::Isotope<A>,
    StorageRef: ops::Deref<Target = C::Storage>,
    DiscrimMapped: discrim::Mapped<Discrim = C::Discrim, Value = StorageRef>,
{
    type StorageRef = StorageRef;
    type DiscrimMapped = DiscrimMapped;

    fn view<'u>(
        &self,
        key: DiscrimMapped::Key,
        storages: &'u DiscrimMapped,
    ) -> Option<&'u StorageRef> {
        storages.get_by(key)
    }
}
