use parking_lot::Mutex;

use super::{SendSystemIndex, UnsendSystemIndex};
use crate::system;

pub(in crate::world::scheduler) struct SyncState {
    pub(in crate::world::scheduler) send_systems: Vec<(String, Mutex<Box<dyn system::Sendable>>)>,
}

impl SyncState {
    pub(in crate::world::scheduler) fn get_send_system(
        &self,
        index: SendSystemIndex,
    ) -> &Mutex<Box<dyn system::Sendable>> {
        &self.send_systems.get(index.0).expect("invalid node index").1
    }
}

pub(in crate::world::scheduler) struct UnsyncState {
    pub(in crate::world::scheduler) unsend_systems: Vec<(String, Box<dyn system::Unsendable>)>,
}

impl UnsyncState {
    pub(in crate::world::scheduler) fn get_unsend_system_mut(
        &mut self,
        index: UnsendSystemIndex,
    ) -> &mut dyn system::Unsendable {
        &mut *self.unsend_systems.get_mut(index.0).expect("invalid node index").1
    }
}

#[cfg(test)]
static_assertions::assert_impl_all!(SyncState: Send, Sync);
