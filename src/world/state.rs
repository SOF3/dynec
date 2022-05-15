use std::any::{self, Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops;

use parking_lot::{Mutex, RwLockReadGuard};

use super::storage::Storage;
use super::typed;
use crate::util::DbgTypeId;
use crate::{comp, Archetype};

/// Stores the component states in a world.
pub struct Components {
    pub(in crate::world) archetypes: HashMap<DbgTypeId, Box<dyn typed::AnyTyped>>,
}

impl Components {
    /// Creates a dummy, empty component store used for testing.
    pub fn empty() -> Self { Self { archetypes: HashMap::new() } }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype<A: Archetype>(&self) -> &typed::Typed<A> {
        match self.archetypes.get(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any().downcast_ref().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    /// Fetches the [`Typed`](typed::Typed) for the requested archetype.
    pub(crate) fn archetype_mut<A: Archetype>(&mut self) -> &mut typed::Typed<A> {
        match self.archetypes.get_mut(&DbgTypeId::of::<A>()) {
            Some(typed) => typed.as_any_mut().downcast_mut().expect("TypeId mismatch"),
            None => panic!(
                "The archetype {} cannot be used because it is not used in any systems",
                any::type_name::<A>()
            ),
        }
    }

    /// Gets a shared reference to the storage for the given component.
    ///
    /// # Panics
    /// - if the archetype or component is not used in any systems
    pub(crate) fn simple_storage_shared<A: Archetype, C: comp::Simple<A>>(
        &self,
    ) -> impl ops::Deref<Target = Storage<A, C>> + '_ {
        let storage_lock = match self.archetype::<A>().simple_storages.get(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {}/{} cannot be used because it is not used in any systems",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        let guard = match storage_lock.try_read() {
            Some(guard) => guard,
            None => panic!(
                "The component {}/{} is currently used by another system. Maybe scheduler leak?",
                any::type_name::<A>(),
                any::type_name::<C>()
            ),
        };
        RwLockReadGuard::map(guard, |storage| {
            storage.as_any().downcast_ref::<Storage<A, C>>().expect("TypeId mismatch")
        })
    }
}

#[cfg(test)]
static_assertions::assert_impl_all!(Components: Send, Sync);

/// Stores the thread-safe global states in a world.
pub struct SyncGlobals {
    /// Global states that can be concurrently accessed by systems on other threads.
    pub(in crate::world) sync_globals: HashMap<DbgTypeId, Mutex<Box<dyn Any + Send + Sync>>>,
}

impl SyncGlobals {
    /// Creates a dummy, empty global store used for testing.
    pub(crate) fn empty() -> Self { Self { sync_globals: HashMap::new() } }
}

/// Stores the thread-unsafe global states in a world.
pub struct UnsyncGlobals {
    /// Global states that must be accessed on the main thread.
    pub(in crate::world) unsync_globals: HashMap<DbgTypeId, RefCell<Box<dyn Any>>>,
}

impl UnsyncGlobals {
    /// Creates a dummy, empty global store used for testing.
    pub(crate) fn empty() -> Self { Self { unsync_globals: HashMap::new() } }
}
