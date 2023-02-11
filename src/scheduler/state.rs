use parking_lot::Mutex;

use super::{SendSystemIndex, UnsendSystemIndex};
use crate::system;

pub(crate) struct SyncState {
    pub(crate) send_systems: Vec<(String, Mutex<Box<dyn system::Sendable>>)>,
}

impl SyncState {
    pub(crate) fn get_send_system(
        &self,
        index: SendSystemIndex,
    ) -> (&str, &Mutex<Box<dyn system::Sendable>>) {
        let (debug_name, system) = self.send_systems.get(index.0).expect("invalid node index");
        (debug_name, system)
    }
}

pub(crate) struct UnsyncState {
    pub(crate) unsend_systems: Vec<(String, Box<dyn system::Unsendable>)>,
}

impl UnsyncState {
    pub(crate) fn get_unsend_system_mut(
        &mut self,
        index: UnsendSystemIndex,
    ) -> (&str, &mut dyn system::Unsendable) {
        let (debug_name, system) =
            self.unsend_systems.get_mut(index.0).expect("invalid node index");
        (debug_name, &mut **system)
    }
}

#[cfg(test)]
#[allow(clippy::extra_unused_type_parameters)] // macro magic
mod _assert {
    static_assertions::assert_impl_all!(super::SyncState: Send, Sync);
}
