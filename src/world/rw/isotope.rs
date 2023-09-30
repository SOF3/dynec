use std::any::{type_name, TypeId};
use std::fmt;

use crate::{comp, storage, world, Archetype};

mod offline;
pub(crate) mod read;
pub(crate) mod write;

fn storage_map<A: Archetype, C: comp::Isotope<A>>(
    comps: &world::Components,
) -> &storage::IsotopeMap<A, C> {
    let typed = comps.archetype::<A>();
    match typed.isotope_storage_maps.get(&TypeId::of::<C>()) {
        Some(map) => map.downcast_ref::<C>(),
        None => panic!(
            "The component {}/{} cannot be retrieved because it is not used in any systems",
            type_name::<A>(),
            type_name::<C>(),
        ),
    }
}

fn panic_invalid_key<A, C>(key: impl fmt::Debug) -> ! {
    panic!(
        "The index {key:?} is not available in the isotope request for {}/{}",
        type_name::<A>(),
        type_name::<C>(),
    )
}
