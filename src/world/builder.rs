use std::any::Any;
use std::collections::HashMap;

use super::{scheduler, typed};
use crate::system::spec;
use crate::util::DbgTypeId;
use crate::{comp, system, Global};

/// This type is used to build a world.
/// No more systems can be scheduled after the builder is built.
#[derive(Default)]
pub struct Builder {
    scheduler:      scheduler::Builder,
    archetypes:     HashMap<DbgTypeId, Box<dyn typed::AnyBuilder>>,
    sync_globals:   HashMap<DbgTypeId, GlobalBuilder<dyn Any + Send + Sync>>,
    unsync_globals: HashMap<DbgTypeId, GlobalBuilder<dyn Any>>,
}

enum GlobalBuilder<A: ?Sized> {
    Provided(Box<A>),
    Missing(fn() -> Box<A>),
}

impl Builder {
    fn archetype(
        &mut self,
        archetype: spec::ArchetypeDescriptor,
    ) -> &mut Box<dyn typed::AnyBuilder> {
        self.archetypes.entry(archetype.id).or_insert_with(archetype.builder)
    }

    fn register_resources(&mut self, system: system::Spec, sync: bool, node: scheduler::Node) {
        for request in system.global_requests {
            match (request.initial, sync) {
                (spec::GlobalInitial::Sync(initial), _) => {
                    if self.unsync_globals.contains_key(&request.ty) {
                        panic!(
                            "Global type {} is used as both thread-safe and thread-local",
                            request.ty
                        );
                    }

                    self.sync_globals
                        .entry(request.ty)
                        .or_insert_with(|| GlobalBuilder::Missing(initial));

                    self.scheduler.use_resource(
                        node,
                        scheduler::ResourceType::Global(request.ty),
                        scheduler::ResourceAccess::new(request.mutable),
                    );
                }
                (spec::GlobalInitial::Unsync(_), true) => {
                    panic!(
                        "Cannot schedule system {} as thread-safe because it requires \
                         thread-unsafe resources",
                        system.debug_name,
                    );
                }
                (spec::GlobalInitial::Unsync(initial), false) => {
                    if self.sync_globals.contains_key(&request.ty) {
                        panic!(
                            "Global type {} is used as both thread-safe and thread-local",
                            request.ty
                        );
                    }

                    self.unsync_globals
                        .entry(request.ty)
                        .or_insert_with(|| GlobalBuilder::Missing(initial));

                    self.scheduler.use_resource(
                        node,
                        scheduler::ResourceType::Global(request.ty),
                        scheduler::ResourceAccess::new(request.mutable),
                    );
                }
            }
        }

        for request in system.simple_requests {
            let builder = self.archetype(request.arch);
            builder.add_simple_storage_if_missing(request.comp, request.storage_builder);

            self.scheduler.use_resource(
                node,
                scheduler::ResourceType::Simple { arch: request.arch.id, comp: request.comp },
                scheduler::ResourceAccess::new(request.mutable),
            );
        }

        for request in system.isotope_requests {
            let builder = self.archetype(request.arch);
            builder.add_isotope_factory_if_missing(request.comp, request.factory_builder);

            self.scheduler.use_resource(
                node,
                scheduler::ResourceType::Isotope { arch: request.arch.id, comp: request.comp },
                scheduler::ResourceAccess::with_discrim(request.mutable, request.discrim.clone()),
            );
        }

        self.scheduler.add_dependencies(system.dependencies, node);
    }

    /// Schedules a thread-safe system.
    pub fn schedule(&mut self, system: Box<dyn system::System + Send>) {
        let spec = system.get_spec();
        let (node, system) = self.scheduler.push_send_system(system);
        self.register_resources(spec, true, node);
    }

    /// Schedules a system that must be run on the main thread.
    pub fn schedule_thread_unsafe(&mut self, system: Box<dyn system::System>) {
        let spec = system.get_spec();
        let (node, system) = self.scheduler.push_unsend_system(system);
        self.register_resources(spec, false, node);
    }

    /// Provides a thread-safe global resource.
    pub fn global<G: Global + Send + Sync>(&mut self, value: G) {
        self.sync_globals.insert(DbgTypeId::of::<G>(), GlobalBuilder::Provided(Box::new(value)));
    }

    /// Provides a thread-unsafe global resource.
    pub fn global_thread_unsafe<G: Global>(&mut self, value: G) {
        self.unsync_globals.insert(DbgTypeId::of::<G>(), GlobalBuilder::Provided(Box::new(value)));
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
            .sync_globals
            .into_iter()
            .map(|(ty, da)| match da {
                GlobalBuilder::Provided(value) => (ty, value),
                GlobalBuilder::Missing(default) => (ty, default()),
            })
            .collect();
        let unsend_globals = self
            .unsync_globals
            .into_iter()
            .map(|(ty, da)| match da {
                GlobalBuilder::Provided(value) => (ty, value),
                GlobalBuilder::Missing(default) => (ty, default()),
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
