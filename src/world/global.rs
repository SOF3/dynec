use std::any::{self, Any, TypeId};
use std::collections::HashMap;
use std::ops;

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::entity::referrer;
use crate::util::DbgTypeId;
use crate::Global;

/// Stores the thread-safe global states in a world.
pub struct SyncGlobals {
    /// Global states that can be concurrently accessed by systems on other threads.
    pub(in crate::world) sync_globals:
        HashMap<DbgTypeId, (referrer::SingleVtable, RwLock<Box<dyn Any + Send + Sync>>)>,
}

impl SyncGlobals {
    /// Creates a dummy, empty global store used for testing.
    pub fn empty() -> Self { Self { sync_globals: HashMap::new() } }

    /// Retrieves a read-only, shared reference to the given global state.
    ///
    /// # Panics
    /// - if the global state is not used in any systems
    /// - if another thread is exclusively accessing the same archetyped component.
    pub fn read<G: Global + Send + Sync>(&self) -> impl ops::Deref<Target = G> + '_ {
        let (_, lock) = match self.sync_globals.get(&TypeId::of::<G>()) {
            Some(lock) => lock,
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        };
        let guard = match lock.try_read() {
            Some(guard) => guard,
            None => panic!(
                "The global state {} is currently exclusively locked by another system. Maybe \
                 scheduler bug?",
                any::type_name::<G>()
            ),
        };
        RwLockReadGuard::map(guard, |guard| guard.downcast_ref::<G>().expect("TypeId mismatch"))
    }

    /// Retrieves a writable, exclusive reference to the given global state.
    ///
    /// # Panics
    /// - if the global state is not used in any systems
    /// - if another thread is accessing the same archetyped component.
    pub fn write<G: Global + Send + Sync>(&self) -> impl ops::DerefMut<Target = G> + '_ {
        let (_, lock) = match self.sync_globals.get(&TypeId::of::<G>()) {
            Some(lock) => lock,
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        };
        let guard = match lock.try_write() {
            Some(guard) => guard,
            None => panic!(
                "The global state {} is currently used exclusively by another system. Maybe \
                 scheduler bug?",
                any::type_name::<G>()
            ),
        };
        RwLockWriteGuard::map(guard, |guard| guard.downcast_mut::<G>().expect("TypeId mismatch"))
    }

    pub(crate) fn get_mut<G: Global + Send + Sync>(&mut self) -> &mut G {
        let (_, lock) = match self.sync_globals.get_mut(&TypeId::of::<G>()) {
            Some(lock) => lock,
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        };
        lock.get_mut().downcast_mut::<G>().expect("TypeId mismatch")
    }
}

/// Stores the thread-unsafe global states in a world.
pub struct UnsyncGlobals {
    /// Global states that must be accessed on the main thread.
    pub(in crate::world) unsync_globals: HashMap<DbgTypeId, (referrer::SingleVtable, Box<dyn Any>)>,
}

impl UnsyncGlobals {
    /// Creates a dummy, empty global store used for testing.
    pub fn empty() -> Self { Self { unsync_globals: HashMap::new() } }

    /// Gets a reference to the requested global state.
    /// The object must be marked as thread-unsafe during world creation.
    ///
    /// Since the system is run on main thread,
    /// it is expected that a mutable reference to `UnsyncGlobals` is available.
    pub fn get<G: Global>(&mut self) -> &mut G {
        match self.unsync_globals.get_mut(&TypeId::of::<G>()) {
            Some((_vtable, global)) => global.downcast_mut::<G>().expect("TypeId mismatch"),
            None => panic!(
                "The global state {} cannot be used because it is not used in any systems",
                any::type_name::<G>()
            ),
        }
    }
}
