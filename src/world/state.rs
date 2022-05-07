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

#[cfg(test)]
static_assertions::assert_impl_all!(Components: Send, Sync);

/// Stores the thread-safe global states in a world.
pub struct SendGlobals {
    /// Global states that can be concurrently accessed by systems on other threads.
    pub(in crate::world) data: HashMap<DbgTypeId, Mutex<Box<dyn Any + Send + Sync>>>,
}

/// Stores the thread-unsafe global states in a world.
pub struct UnsendGlobals {
    /// Global states that must be accessed on the main thread.
    pub(in crate::world) data: HashMap<DbgTypeId, RefCell<Box<dyn Any>>>,
}
