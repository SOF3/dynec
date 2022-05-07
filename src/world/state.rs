use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

use parking_lot::Mutex;

use super::typed;
use crate::util::DbgTypeId;

/// Stores the component states in a world.
pub struct Components {
    pub(in crate::world) archetypes: HashMap<DbgTypeId, Box<dyn typed::AnyTyped>>,
}

impl Components {
    /// Creates a dummy, empty component store used for testing.
    pub(crate) fn empty() -> Self { Self { archetypes: HashMap::new() } }
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
