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
    scheduler:      scheduler::Builder,
    archetypes:     HashMap<DbgTypeId, Box<dyn typed::AnyBuilder>>,
    send_globals:   HashMap<DbgTypeId, DefaultableAny<dyn Any + Send + Sync>>,
    unsend_globals: HashMap<DbgTypeId, DefaultableAny<dyn Any>>,
}

enum DefaultableAny<A: ?Sized> {
    Given(Box<A>),
    Missing(fn() -> Box<A>),
}

impl Builder {
    fn archetype(
        &mut self,
        archetype: spec::ArchetypeDescriptor,
    ) -> &mut Box<dyn typed::AnyBuilder> {
        self.archetypes.entry(archetype.id).or_insert_with(archetype.builder)
    }

    fn register_resources(&mut self, system: &dyn system::Spec, sync: bool, id: scheduler::TaskId) {
        system.for_each_global_request(&mut |request| {
            match request.initial {
                spec::GlobalInitial::Sync(initial) => {
                    self.send_globals
                        .entry(request.ty)
                        .or_insert_with(|| DefaultableAny::Missing(initial));
                }
                _ if sync => {
                    panic!(
                        "Cannot schedule system {} as thread-safe because it requires \
                         thread-unsafe resources",
                        system.debug_name()
                    );
                }
                spec::GlobalInitial::Unsync(initial) => {
                    self.unsend_globals
                        .entry(request.ty)
                        .or_insert_with(|| DefaultableAny::Missing(initial));
                }
            }

            self.scheduler.globals.entry(request.ty).or_default().push((id, request.mutable));
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
        let index = self.scheduler.send_systems.len();
        let id = scheduler::TaskId { class: scheduler::TaskClass::Send, index };

        self.register_resources(&*system, true, id);

        self.scheduler.send_systems.push(system);
    }

    /// Schedules a system that must be run on the main thread.
    pub fn schedule_thread_unsafe(&mut self, system: Box<dyn system::Spec>) {
        let index = self.scheduler.unsend_systems.len();
        let id = scheduler::TaskId { class: scheduler::TaskClass::Unsend, index };

        self.register_resources(&*system, false, id);

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

        let send_globals = self
            .send_globals
            .into_iter()
            .map(|(ty, da)| match da {
                DefaultableAny::Given(value) => (ty, value),
                DefaultableAny::Missing(default) => (ty, default()),
            })
            .collect();
        let unsend_globals = self
            .unsend_globals
            .into_iter()
            .map(|(ty, da)| match da {
                DefaultableAny::Given(value) => (ty, value),
                DefaultableAny::Missing(default) => (ty, default()),
            })
            .collect();

        super::World { storages, send_globals, unsend_globals, scheduler: self.scheduler.build() }
    }
}

/// Identifies an archetype + component type + discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ComponentIdentifier {
    arch: DbgTypeId,
    comp: comp::any::Identifier,
}
