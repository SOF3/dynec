use std::any::type_name;
use std::sync::Arc;

use parking_lot::lock_api::ArcRwLockReadGuard;
use parking_lot::RwLock;

use crate::{comp, Archetype};

pub(crate) mod full;
pub(crate) mod partial;

type LockedStorage<A, C> =
    ArcRwLockReadGuard<parking_lot::RawRwLock, <C as comp::SimpleOrIsotope<A>>::Storage>;

fn own_storage<A: Archetype, C: comp::Isotope<A>>(
    discrim: C::Discrim,
    storage: &Arc<RwLock<C::Storage>>,
) -> LockedStorage<A, C> {
    let storage: Arc<RwLock<C::Storage>> = Arc::clone(storage);
    match storage.try_read_arc() {
        Some(guard) => guard,
        None => panic!(
            "The component {}/{}/{:?} is currently uniquely locked by another system. Maybe \
             scheduler bug?",
            type_name::<A>(),
            type_name::<C>(),
            discrim,
        ),
    }
}
