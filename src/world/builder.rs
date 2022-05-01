use std::any::Any;
use std::collections::HashMap;

use super::{scheduler, typed};
use crate::system::spec;
use crate::util::DbgTypeId;
use crate::{comp, system};

/// This type is used to build a world.
/// No more systems can be scheduled after the builder is built.
#[derive(Default)]
pub struct Builder {
    scheduler:  scheduler::Builder,
    archetypes: HashMap<DbgTypeId, Box<dyn typed::AnyBuilder>>,
    globals:    HashMap<DbgTypeId, DefaultableAny>,
}

enum DefaultableAny {
    Given(Box<dyn Any>),
    Missing(fn() -> Box<dyn Any>),
}

impl Builder {
    fn archetype(
        &mut self,
        archetype: spec::ArchetypeDescriptor,
    ) -> &mut Box<dyn typed::AnyBuilder> {
        self.archetypes.entry(archetype.id).or_insert_with(archetype.builder)
    }

    fn register_resources(&mut self, system: &dyn system::Spec, sync: bool) {
        system.for_each_global_request(&mut |request| {
            if request.sync {
                self.scheduler.send_globals.entry(request.global).or_default();
            } else if sync {
                panic!(
                    "Cannot schedule system {} as thread-safe because it requires thread-unsafe \
                     resources",
                    system.debug_name()
                );
            } else {
                self.scheduler.unsend_globals.entry(request.global).or_default();
            }
        });

        system.for_each_simple_request(&mut |request| {
            let builder = self.archetype(request.archetype);
            builder.add_simple_storage_if_missing(request.comp, request.storage_builder);
        });

        system.for_each_isotope_request(&mut |request| {
            let builder = self.archetype(request.archetype);
            builder.add_isotope_factory_if_missing(request.comp, request.factory_builder);
        });
    }

    fn create_partition(&mut self, partition: Box<dyn system::Partition>) -> scheduler::TaskId {
        let partition = system::PartitionWrapper(partition);
        match self.scheduler.partitions.get_index_of(&partition) {
            Some(index) => scheduler::TaskId { class: scheduler::TaskClass::Partition, index },
            None => {
                let index = self.scheduler.partitions.len();
                self.scheduler.partitions.insert(partition);
                scheduler::TaskId { class: scheduler::TaskClass::Partition, index }
            }
        }
    }

    fn add_dep(&mut self, earlier: scheduler::TaskId, later: scheduler::TaskId) {
        self.scheduler.dependencies.entry(later).or_default().push(earlier);
        self.scheduler.dependencies.entry(earlier).or_default().push(later);
    }

    fn add_deps(&mut self, system: &dyn system::Spec, system_id: scheduler::TaskId) {
        system.for_each_dependency(&mut |dep| match dep {
            spec::Dependency::Before(partition) => {
                let partition_id = self.create_partition(partition);
                self.add_dep(system_id, partition_id);
            }
            spec::Dependency::After(partition) => {
                let partition_id = self.create_partition(partition);
                self.add_dep(partition_id, system_id);
            }
        });
    }

    /// Schedules a thread-safe system.
    pub fn schedule(&mut self, system: Box<dyn system::Spec + Send>) {
        self.register_resources(&*system, true);

        let index = self.scheduler.send_systems.len();
        let id = scheduler::TaskId { class: scheduler::TaskClass::Send, index };

        self.scheduler.send_systems.push(system);
    }

    /// Schedules a system that must be run on the main thread.
    pub fn schedule_thread_unsafe(&mut self, system: Box<dyn system::Spec>) {
        self.register_resources(&*system, false);

        let index = self.scheduler.unsend_systems.len();
        let id = scheduler::TaskId { class: scheduler::TaskClass::Unsend, index };
        self.scheduler.unsend_systems.push(system);
    }

    /// Constructs the world from the builder.
    pub fn build(self) -> super::World {
        let storages = super::Storages {
            archetypes: self
                .archetypes
                .into_iter()
                .map(|(ty, builder)| (ty, builder.build()))
                .collect(),
        };

        let globals = self
            .globals
            .into_iter()
            .map(|(ty, da)| match da {
                DefaultableAny::Given(value) => (ty, value),
                DefaultableAny::Missing(default) => (ty, default()),
            })
            .collect();
        let globals = super::Globals { globals };

        super::World { storages, globals, scheduler: self.scheduler.build() }
    }
}

/// Identifies an archetype + component type + discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ComponentIdentifier {
    arch: DbgTypeId,
    comp: comp::any::Identifier,
}
