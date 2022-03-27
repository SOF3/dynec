use std::any::{Any, TypeId};
use std::collections::BTreeMap;

use super::{ArchComp, IsotopeSpec, SimpleSpec};
use crate::system;

/// This type is used to build a world.
/// No more systems can be scheduled after the builder is built.
#[derive(Default)]
pub struct Builder {
    simple:  BTreeMap<ArchComp, SimpleSpec>,
    isotope: BTreeMap<ArchComp, IsotopeSpec>,

    // systems that can be scheduled to other threads.
    send_systems:   Vec<Box<dyn system::Spec + Send>>,
    // systems that must be scheduled to the main thread.
    unsend_systems: Vec<Box<dyn system::Spec>>,

    // global states that can be concurrently accessed by systems on other threads.
    globals:        BTreeMap<TypeId, Box<dyn Any + Sync>>,
    // global states that must be accessed on the main thread.
    unsync_globals: BTreeMap<TypeId, Box<dyn Any>>,
}

impl Builder {
    pub fn schedule(&mut self, system: Box<dyn system::Spec + Send>) { todo!() }

    pub fn schedule_thread_local(&mut self, system: Box<dyn system::Spec>) { todo!() }

    pub fn build(self) -> super::World { todo!() }
}
