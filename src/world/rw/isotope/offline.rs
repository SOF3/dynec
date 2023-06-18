use std::any::{type_name, TypeId};
use std::sync::Arc;

use crate::{comp, entity, world, Archetype, Storage as _};

impl world::Components {
    fn offline_isotope_storage<A, C>(&mut self, discrim: C::Discrim) -> Option<&mut C::Storage>
    where
        A: Archetype,
        C: comp::Isotope<A>,
    {
        let Some(map) = self.archetype_mut::<A>().isotope_storage_maps.get_mut(&TypeId::of::<C>()) else {
            panic!(
                "The component {}/{} cannot be retrieved because it is not used in any systems",
                type_name::<A>(),
                type_name::<C>(),
            )
        };
        let map = Arc::get_mut(map).expect("map arc was leaked").downcast_mut::<C>();
        let inner = map.map.get_mut();
        let storage = inner.get_mut(discrim)?;
        let storage = Arc::get_mut(storage).expect("storage arc was leaked");
        Some(storage.get_mut())
    }

    pub fn get_isotope<A, C, E>(&mut self, entity: E, discrim: C::Discrim) -> Option<&mut C>
    where
        A: Archetype,
        C: comp::Isotope<A>,
        E: entity::Ref<Archetype = A>,
    {
        let storage = self.offline_isotope_storage::<A, C>(discrim)?;
        storage.get_mut(entity.id())
    }
}
