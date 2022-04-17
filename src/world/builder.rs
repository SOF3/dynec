use std::any::{Any, TypeId};
use std::collections::{BTreeMap, HashMap};

use indexmap::IndexSet;

use super::{scheduler, ArchComp, IsotopeSpec, SimpleSpec};
use crate::system;

/// This type is used to build a world.
/// No more systems can be scheduled after the builder is built.
#[derive(Default)]
pub struct Builder {
    pub(crate) simple:  BTreeMap<ArchComp, SimpleSpec>,
    pub(crate) isotope: BTreeMap<ArchComp, IsotopeSpec>,

    /// Systems that can be scheduled to other threads.
    pub(crate) send_systems:   Vec<Box<dyn system::Spec + Send>>,
    /// Systems that must be scheduled to the main thread.
    pub(crate) unsend_systems: Vec<Box<dyn system::Spec>>,

    /// Global states that can be concurrently accessed by systems on other threads.
    pub(crate) send_globals:   BTreeMap<TypeId, Option<Box<dyn Any + Send>>>,
    /// Global states that must be accessed on the main thread.
    pub(crate) unsend_globals: BTreeMap<TypeId, Option<Box<dyn Any>>>,

    pub(crate) partitions: IndexSet<system::PartitionWrapper>,

    /// Indexes systems that access a component.
    pub(crate) components: HashMap<TypeId, Vec<(scheduler::TaskId, scheduler::ComponentAccess)>>,
    /// Indexes systems that access a global.
    pub(crate) globals:    HashMap<TypeId, Vec<(scheduler::TaskId, bool)>>,

    /// If `dependencies[a].contains(b)`, `b` runs before `a`
    pub(crate) dependencies: HashMap<scheduler::TaskId, Vec<scheduler::TaskId>>,
    /// If `dependents[a].contains(b)`, `a` runs before `b`
    pub(crate) dependents:   HashMap<scheduler::TaskId, Vec<scheduler::TaskId>>,
}

impl Builder {
    fn register_resources(&mut self, system: &dyn system::Spec, sync: bool) {
        system.for_each_global_request(&mut |request| {
            if request.sync {
                self.send_globals.entry(request.global).or_default();
            } else if sync {
                panic!(
                    "Cannot schedule system {} as thread-safe because it requires thread-unsafe \
                     resources",
                    system.debug_name()
                );
            } else {
                self.unsend_globals.entry(request.global).or_default();
            }
        });
    }

    fn create_partition(&mut self, partition: Box<dyn system::Partition>) -> scheduler::TaskId {
        let partition = system::PartitionWrapper(partition);
        match self.partitions.get_index_of(&partition) {
            Some(index) => scheduler::TaskId { class: scheduler::TaskClass::Partition, index },
            None => {
                let index = self.partitions.len();
                self.partitions.insert(partition);
                scheduler::TaskId { class: scheduler::TaskClass::Partition, index }
            }
        }
    }

    fn add_dep(&mut self, earlier: scheduler::TaskId, later: scheduler::TaskId) {
        self.dependencies.entry(later).or_default().push(earlier);
        self.dependencies.entry(earlier).or_default().push(later);
    }

    fn add_deps(&mut self, system: &dyn system::Spec, system_id: scheduler::TaskId) {
        system.for_each_dependency(&mut |dep| match dep {
            system::spec::Dependency::Before(partition) => {
                let partition_id = self.create_partition(partition);
                self.add_dep(system_id, partition_id);
            }
            system::spec::Dependency::After(partition) => {
                let partition_id = self.create_partition(partition);
                self.add_dep(partition_id, system_id);
            }
        });
    }

    /// Schedules a thread-safe system.
    pub fn schedule(&mut self, system: Box<dyn system::Spec + Send>) {
        self.register_resources(&*system, true);

        let index = self.send_systems.len();
        let id = scheduler::TaskId { class: scheduler::TaskClass::Send, index };

        self.send_systems.push(system);
    }

    /// Schedules a system that must be run on the main thread.
    pub fn schedule_thread_unsafe(&mut self, system: Box<dyn system::Spec>) {
        self.register_resources(&*system, false);

        let index = self.unsend_systems.len();
        let id = scheduler::TaskId { class: scheduler::TaskClass::Unsend, index };
        self.unsend_systems.push(system);
    }

    /// Constructs the world from the builder.
    pub fn build(self) -> super::World { todo!() }
}
