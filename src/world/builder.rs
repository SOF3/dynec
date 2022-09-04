use std::any::Any;
use std::collections::HashMap;

use parking_lot::RwLock;

use super::{scheduler, typed};
use crate::entity::{deletion, ealloc, generation, referrer};
use crate::system::spec;
use crate::util::DbgTypeId;
use crate::{system, Global};

/// This type is used to build a world.
/// No more systems can be scheduled after the builder is built.
pub struct Builder {
    scheduler:      scheduler::Builder,
    archetypes:     HashMap<DbgTypeId, (ealloc::AnyBuilder, Box<dyn typed::AnyBuilder>)>,
    sync_globals:   GlobalBuilderMap<dyn Any + Send + Sync>,
    unsync_globals: GlobalBuilderMap<dyn Any>,
}

enum GlobalBuilder<G: ?Sized> {
    Provided(Box<G>),
    Missing(fn() -> Box<G>),
}

impl Builder {
    /// Creates a new builder with the specified concurrency.
    pub fn new(concurrency: usize) -> Self {
        Self {
            scheduler:      scheduler::Builder::new(concurrency),
            archetypes:     HashMap::new(),
            sync_globals:   {
                let mut map = HashMap::new();
                populate_default_globals(&mut map);
                map
            },
            unsync_globals: HashMap::new(),
        }
    }

    fn archetype(
        &mut self,
        archetype: spec::ArchetypeDescriptor,
    ) -> &mut Box<dyn typed::AnyBuilder> {
        &mut self.archetypes.entry(archetype.id).or_insert_with(archetype.builder).1
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
                        .or_insert_with(|| (request.vtable, GlobalBuilder::Missing(initial)));

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
                        .or_insert_with(|| (request.vtable, GlobalBuilder::Missing(initial)));

                    self.scheduler.use_resource(
                        node,
                        scheduler::ResourceType::Global(request.ty),
                        scheduler::ResourceAccess::new(request.mutable),
                    );
                }
            }

            for &strong_ref in &request.strong_refs {
                self.scheduler.add_dependencies(
                    vec![spec::Dependency::Before(Box::new(system::EntityCreationPartition {
                        ty: strong_ref,
                    }))],
                    node,
                );
            }
        }

        for request in system.simple_requests {
            let builder = self.archetype(request.arch);
            builder.add_simple_storage_if_missing(
                request.comp,
                request.storage_builder,
                request.vtable,
            );

            self.scheduler.use_resource(
                node,
                scheduler::ResourceType::Simple { arch: request.arch.id, comp: request.comp },
                scheduler::ResourceAccess::new(request.mutable),
            );

            for &strong_ref in &request.strong_refs {
                self.scheduler.add_dependencies(
                    vec![spec::Dependency::Before(Box::new(system::EntityCreationPartition {
                        ty: strong_ref,
                    }))],
                    node,
                );
            }
        }

        for request in system.isotope_requests {
            let builder = self.archetype(request.arch);
            builder.add_isotope_factory_if_missing(
                request.comp,
                request.factory_builder,
                request.vtable,
            );

            self.scheduler.use_resource(
                node,
                scheduler::ResourceType::Isotope { arch: request.arch.id, comp: request.comp },
                scheduler::ResourceAccess::with_discrim(request.mutable, request.discrim.clone()),
            );

            for &strong_ref in &request.strong_refs {
                self.scheduler.add_dependencies(
                    vec![spec::Dependency::Before(Box::new(system::EntityCreationPartition {
                        ty: strong_ref,
                    }))],
                    node,
                );
            }
        }

        for request in system.entity_creator_requests {
            if !request.no_partition {
                self.scheduler.add_dependencies(
                    vec![spec::Dependency::After(Box::new(system::EntityCreationPartition {
                        ty: request.arch,
                    }))],
                    node,
                );
            }
        }

        self.scheduler.add_dependencies(system.dependencies, node);
    }

    /// Schedules a thread-safe system.
    pub fn schedule(&mut self, system: Box<dyn system::Sendable>) {
        let spec = system.get_spec();
        let (node, _spec) = self.scheduler.push_send_system(system);
        self.register_resources(spec, true, node);
    }

    /// Schedules a system that must be run on the main thread.
    pub fn schedule_thread_unsafe(&mut self, system: Box<dyn system::Unsendable>) {
        let spec = system.get_spec();
        let (node, _spec) = self.scheduler.push_unsend_system(system);
        self.register_resources(spec, false, node);
    }

    /// Provides a thread-safe global resource.
    pub fn global<G: Global + Send + Sync>(&mut self, value: G) {
        self.sync_globals.insert(
            DbgTypeId::of::<G>(),
            (referrer::SingleVtable::of::<G>(), GlobalBuilder::Provided(Box::new(value))),
        );
    }

    /// Provides a thread-unsafe global resource.
    pub fn global_thread_unsafe<G: Global>(&mut self, value: G) {
        self.unsync_globals.insert(
            DbgTypeId::of::<G>(),
            (referrer::SingleVtable::of::<G>(), GlobalBuilder::Provided(Box::new(value))),
        );
    }

    /// Adjust the concurrency of the scheduler.
    /// Pass `0` to disable parallelism.
    pub fn set_concurrency(&mut self, concurrency: usize) {
        self.scheduler.concurrency = concurrency;
    }

    /// Constructs the world from the builder.
    pub fn build(self) -> super::World {
        let (ealloc_map, storages) = self
            .archetypes
            .into_iter()
            .map(|(ty, (ealloc, storages))| {
                ((ty, ealloc(self.scheduler.concurrency + 1)), (ty, storages.build()))
            })
            .unzip();

        let ealloc_map = ealloc::Map::new(ealloc_map);
        let storages = super::Components { archetypes: storages };

        let sync_globals = self
            .sync_globals
            .into_iter()
            .map(|(ty, (vtable, global_builder))| {
                (
                    ty,
                    (
                        vtable,
                        RwLock::new(match global_builder {
                            GlobalBuilder::Provided(value) => value,
                            GlobalBuilder::Missing(default) => default(),
                        }),
                    ),
                )
            })
            .collect();
        let sync_globals = super::SyncGlobals { sync_globals };

        let unsync_globals = self
            .unsync_globals
            .into_iter()
            .map(|(ty, (vtable, global_builder))| {
                (
                    ty,
                    (
                        vtable,
                        match global_builder {
                            GlobalBuilder::Provided(value) => value,
                            GlobalBuilder::Missing(default) => default(),
                        },
                    ),
                )
            })
            .collect();
        let unsync_globals = super::UnsyncGlobals { unsync_globals };

        super::World {
            ealloc_map,
            components: storages,
            sync_globals,
            unsync_globals,
            scheduler: self.scheduler.build(),
        }
    }
}

fn populate_default_globals(map: &mut GlobalBuilderMap<dyn Any + Send + Sync>) {
    fn put_global<T: Any + Send + Sync + referrer::Referrer>(
        map: &mut GlobalBuilderMap<dyn Any + Send + Sync>,
        value: T,
    ) {
        map.insert(
            DbgTypeId::of::<T>(),
            (referrer::SingleVtable::of::<T>(), GlobalBuilder::Provided(Box::new(value))),
        );
    }

    put_global(map, generation::StoreMap::default());
    put_global(map, deletion::Flags::default());

    #[cfg(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ))]
    {
        use crate::entity::rctrack;

        put_global(map, rctrack::StoreMap::default());
    }
}

type GlobalBuilderMap<T> = HashMap<DbgTypeId, (referrer::SingleVtable, GlobalBuilder<T>)>;
