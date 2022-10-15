//! The world stores the states of the game.

use std::any::{self, TypeId};
use std::sync::Arc;

use crate::entity::{deletion, ealloc, generation, Ealloc, Raw};
use crate::{comp, entity, system, Archetype, Entity, Global};

mod builder;
pub use builder::Builder;

pub(crate) mod global;
pub use global::{SyncGlobals, UnsyncGlobals};

pub(crate) mod accessor;
pub use accessor::Components;

pub mod storage;
pub use storage::Storage;
pub(crate) mod typed;

mod scheduler;
pub use scheduler::{Node as ScheduleNode, PartitionIndex, SendSystemIndex, UnsendSystemIndex};

pub mod tracer;
pub use tracer::Tracer;

pub mod offline;

/// A bundle encapsulates the systems and resources for a specific feature.
/// This can be used by library crates to expose their features as a single API.
pub trait Bundle {
    /// Schedules the systems used by this bundle.
    ///
    /// Scheduling a system automatically registers the archetypes and components used by the system.
    fn register(&self, _builder: &mut Builder) {}

    /// Populates the world with entities and global states.
    fn populate(&self, _world: &mut World) {}
}

/// Creates a dynec world from bundles.
pub fn new<'t>(bundles: impl IntoIterator<Item = &'t dyn Bundle> + Copy) -> World {
    new_with_concurrency(
        bundles,
        match std::thread::available_parallelism() {
            Ok(c) => c.get(),
            Err(err) => {
                log::error!("Cannot detect number of CPUs ({err}), parallelism disabled");
                0
            }
        },
    )
}

/// Creates a dynec world from bundles with threading disabled.
pub fn new_unthreaded<'t>(bundles: impl IntoIterator<Item = &'t dyn Bundle> + Copy) -> World {
    new_with_concurrency(bundles, 0)
}

/// Creates a dynec world from bundles and specify the number of worker threads
/// (not counting the main thread, which only executes thread-local tasks).
pub fn new_with_concurrency<'t>(
    bundles: impl IntoIterator<Item = &'t dyn Bundle> + Copy,
    concurrency: usize,
) -> World {
    let mut builder = Builder::new(concurrency);

    for bundle in bundles {
        bundle.register(&mut builder);
    }

    let mut world = builder.build();

    for bundle in bundles {
        bundle.populate(&mut world);
    }

    world
}

/// The data structure that stores all states in the game.
pub struct World {
    /// Stores the [`entity::Ealloc`] implementations for each archetype.
    ealloc_map:         ealloc::Map,
    /// Stores the component states in a world.
    pub components:     Components,
    /// Stores the system-local states and the scheduler topology.
    scheduler:          scheduler::Scheduler,
    /// Global states that can be concurrently accessed by systems on other threads.
    pub sync_globals:   SyncGlobals,
    /// Global states that must be accessed on the main thread.
    pub unsync_globals: UnsyncGlobals,
}

impl World {
    /// Executes all systems in the world.
    pub fn execute(&mut self, tracer: &impl Tracer) {
        self.scheduler.execute(
            tracer,
            &mut self.components,
            &mut self.sync_globals,
            &mut self.unsync_globals,
            &mut self.ealloc_map,
        );
    }

    /// Adds an entity to the world.
    pub fn create<A: Archetype>(&mut self, comp_map: comp::Map<A>) -> Entity<A> {
        self.create_with_hint::<A>(Default::default(), comp_map)
    }

    /// Adds an entity to the world near another entity.
    pub fn create_with_hint<A: Archetype>(
        &mut self,
        hint: <A::Ealloc as Ealloc>::AllocHint,
        comp_map: comp::Map<A>,
    ) -> Entity<A> {
        let ealloc = match self.ealloc_map.map.get_mut(&TypeId::of::<A>()) {
            Some(ealloc) => ealloc,
            None => panic!(
                "Cannot create entity for archetype {} because it is not used in any systems",
                any::type_name::<A>()
            ),
        };
        let ealloc = ealloc.as_any_mut().downcast_mut::<A::Ealloc>().expect("TypeId mismatch");
        let id = ealloc.allocate(hint);

        let allocated = Entity::new_allocated(id);

        init_entity(
            &mut self.sync_globals,
            id,
            allocated.rc.clone(),
            &mut self.components,
            comp_map,
        );

        allocated
    }

    /// Gets a reference to an entity component in offline mode.
    ///
    /// Requires a mutable reference to the world to ensure that the world is offline.
    pub fn get_simple<A: Archetype, C: comp::Simple<A>, E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
    ) -> Option<&mut C> {
        let typed = self.components.archetype_mut::<A>();
        log::debug!("{:?}", typed.simple_storages.keys().collect::<Vec<_>>());
        let storage = match typed.simple_storages.get_mut(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {} cannot be retrieved because it is not used in any systems",
                any::type_name::<C>()
            ),
        };
        let storage = storage.get_storage::<C>();
        storage.get_mut(entity.id())
    }

    /// Gets a thread-safe global state in offline mode.
    pub fn get_global<G: Global + Send + Sync>(&mut self) -> &mut G {
        let global = match self.sync_globals.sync_globals.get_mut(&TypeId::of::<G>()) {
            Some((_, global)) => global,
            None => panic!(
                "The global state {} cannot be retrieved because it is not used in any systems, \
                 or was registered as an unsync global instead of a sync global",
                any::type_name::<G>()
            ),
        };
        global.get_mut().downcast_mut::<G>().expect("TypeId mismatch")
    }

    /// Gets a thread-unsafe global state in offline mode.
    ///
    /// Although permitted by the compiler, this method does not support types
    /// registered as thread-safe global states.
    pub fn get_global_unsync<G: Global>(&mut self) -> &mut G {
        let global = match self.unsync_globals.unsync_globals.get_mut(&TypeId::of::<G>()) {
            Some((_, global)) => global,
            None => panic!(
                "The global state {} cannot be retrieved because it is not used in any systems, \
                 or was registered as a sync global instead of an unsync global",
                any::type_name::<G>()
            ),
        };
        global.downcast_mut::<G>().expect("TypeId mismatch")
    }
}

/// Initializes an entity after allocation.
fn init_entity<A: Archetype>(
    sync_globals: &mut SyncGlobals,
    id: A::RawEntity,
    _rc: entity::MaybeArc,
    components: &mut Components,
    comp_map: comp::Map<A>,
) {
    sync_globals.get_mut::<generation::StoreMap>().next::<A>(id.to_primitive());

    #[cfg(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ))]
    {
        use entity::rctrack;

        sync_globals.get_mut::<rctrack::StoreMap>().set::<A>(id.to_primitive(), _rc);
    }

    let typed = components.archetype_mut::<A>();
    typed.init_entity(id, comp_map);
}

enum DeleteResult {
    /// Deleted
    Deleted,
    /// Finalizers pending
    Terminating,
}

/// Flags an entity for deletion, and deletes it immediately if there are no finalizers.
fn flag_delete_entity<A: Archetype>(
    id: A::RawEntity,
    components: &mut Components,
    sync_globals: &mut SyncGlobals,
    unsync_globals: &mut UnsyncGlobals,
    systems: &mut [(&str, &mut dyn system::Descriptor)],
    ealloc_map: &mut ealloc::Map,
) -> DeleteResult {
    let storage = components
        .archetype_mut::<A>()
        .simple_storages
        .get_mut(&TypeId::of::<deletion::Flag>())
        .expect("deletion::Flags storage is always available");
    storage.get_storage::<deletion::Flag>().set(id, Some(deletion::Flag(())));

    try_real_delete_entity::<A>(components, id, sync_globals, unsync_globals, systems, ealloc_map)
}

/// Deletes an entity immediately if there are no finalizers.
fn try_real_delete_entity<A: Archetype>(
    components: &mut Components,
    entity: <A as Archetype>::RawEntity,
    _sync_globals: &mut SyncGlobals,
    _unsync_globals: &mut UnsyncGlobals,
    _systems: &mut [(&str, &mut dyn system::Descriptor)],
    ealloc_map: &mut ealloc::Map,
) -> DeleteResult {
    let storages = &mut components.archetype_mut::<A>().simple_storages;
    let has_finalizer = storages.values_mut().any(|storage| {
        Arc::get_mut(&mut storage.storage)
            .expect("storage arc was leaked")
            .get_mut()
            .has_finalizer(entity)
    });
    if has_finalizer {
        return DeleteResult::Terminating;
    }

    for storage in storages.values_mut() {
        Arc::get_mut(&mut storage.storage)
            .expect("storage arc was leaked")
            .get_mut()
            .clear_entry(entity);
    }

    #[cfg(any(
        all(debug_assertions, feature = "debug-entity-rc"),
        all(not(debug_assertions), feature = "release-entity-rc"),
    ))]
    {
        use entity::rctrack;

        use crate::util::DbgTypeId;

        let rc = _sync_globals.get_mut::<rctrack::StoreMap>().remove::<A>(entity.to_primitive());
        if Arc::try_unwrap(rc).is_err() {
            let found = search_references(
                components,
                _sync_globals,
                _unsync_globals,
                _systems,
                DbgTypeId::of::<A>(),
                entity.to_primitive(),
            );
            panic!(
                "Detected dangling strong reference to entity {}#{entity:?} in {}. All strong \
                 references to an entity must be dropped before queuing for deletion and removing \
                 all finalizers.",
                any::type_name::<A>(),
                found.join(", ")
            );
        }
    }

    let ealloc = ealloc_map
        .map
        .get_mut(&TypeId::of::<A>())
        .expect("Attempted to delete entity of unknown archetype");
    let ealloc: &mut A::Ealloc = ealloc.as_any_mut().downcast_mut().expect("TypeId mismatch");

    ealloc.queue_deallocate(entity);

    DeleteResult::Deleted
}

#[cfg(any(
    all(debug_assertions, feature = "debug-entity-rc"),
    all(not(debug_assertions), feature = "release-entity-rc"),
))]
fn search_references(
    components: &mut Components,
    sync_globals: &mut SyncGlobals,
    unsync_globals: &mut UnsyncGlobals,
    systems: &mut [(&str, &mut dyn system::Descriptor)],
    archetype: crate::util::DbgTypeId,
    entity: usize,
) -> Vec<String> {
    use std::any::Any;

    use crate::entity::referrer::search_single::SearchSingleStrong;
    use crate::entity::referrer::VisitMutArg;

    let mut state = SearchSingleStrong::new(archetype, entity);

    for (name, system) in systems {
        let mut object = system.visit_mut();
        state._set_debug_name(format!("system {name}"));
        object.0.search_single_strong(&mut state);
    }

    let globals = sync_globals
        .sync_globals
        .iter_mut()
        .map(|(global_ty, (vtable, value))| {
            (global_ty, vtable, &mut **value.get_mut() as &mut dyn Any)
        })
        .chain(
            unsync_globals
                .unsync_globals
                .iter_mut()
                .map(|(global_ty, (vtable, value))| (global_ty, vtable, &mut **value)),
        );
    for (global_ty, vtable, value) in globals {
        state._set_debug_name(format!("global state {global_ty}"));
        vtable.search_single_strong(value, &mut state);
    }

    for (iter_archetype, typed) in &mut components.archetypes {
        typed.referrer_dyn_iter(&iter_archetype.to_string()).search_single_strong(&mut state);
    }

    state.found
}

#[cfg(test)]
pub(crate) mod tests;
