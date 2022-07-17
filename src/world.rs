//! The world stores the states of the game.

use std::any::{self, TypeId};

use crate::entity::ealloc;
use crate::util::DbgTypeId;
use crate::{comp, entity, Archetype, Entity, Global};

mod builder;
pub use builder::Builder;

pub(crate) mod state;
use ealloc::Ealloc;
pub use state::{Components, SyncGlobals, UnsyncGlobals};

pub mod storage;
pub use storage::Storage;
pub(crate) mod typed;

mod scheduler;
pub use scheduler::{Node as ScheduleNode, PartitionIndex, SendSystemIndex, UnsendSystemIndex};

pub mod tracer;
pub use tracer::Tracer;

/// A bundle encapsulates the systems and resources for a specific feature.
/// This can be used by library crates to expose their features as a single API.
pub trait Bundle {
    /// Schedules the systems used by this bundle.
    ///
    /// Scheduling a system automatically registers the archetypes and components used by the system.
    fn register(&self, builder: &mut Builder) {}

    /// Populates the world with entities and global states.
    fn populate(&self, world: &mut World) {}
}

/// Creates a dynec world from bundles.
pub fn new<'t>(bundles: impl IntoIterator<Item = &'t dyn Bundle> + Copy) -> World {
    new_with_concurrency(
        bundles,
        match std::thread::available_parallelism() {
            Ok(c) => c.get(),
            Err(err) => {
                log::error!("Cannot detect number of CPUs, parallelism disabled");
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

/// Identifies an archetype + component type.
pub(crate) struct ArchComp {
    arch: DbgTypeId,
    comp: DbgTypeId,
}

/// Describes a simple component type.
pub(crate) struct SimpleSpec {
    presence:     comp::SimplePresence,
    is_finalizer: bool,
}

impl SimpleSpec {
    fn of<A: Archetype, C: comp::Simple<A>>() -> Self {
        Self { presence: C::PRESENCE, is_finalizer: C::IS_FINALIZER }
    }
}

/// The data structure that stores all states in the game.
pub struct World {
    ealloc_map:     ealloc::Map,
    /// Stores the component states in a world.
    components:     Components,
    /// Stores the system-local states and the scheduler topology.
    scheduler:      scheduler::Scheduler,
    /// Global states that can be concurrently accessed by systems on other threads.
    sync_globals:   SyncGlobals,
    /// Global states that must be accessed on the main thread.
    unsync_globals: UnsyncGlobals,
}

impl World {
    /// Executes all systems in the world.
    pub fn execute(&mut self, tracer: &impl Tracer) {
        self.scheduler.execute(
            tracer,
            &self.components,
            &self.sync_globals,
            &mut self.unsync_globals,
            &mut self.ealloc_map,
        );
    }

    /// Adds an entity to the world.
    pub fn create<A: Archetype>(&mut self, components: comp::Map<A>) -> Entity<A> {
        self.create_near::<entity::Entity<A>>(Default::default(), components)
    }

    /// Adds an entity to the world near another entity.
    pub fn create_near<E: entity::Ref>(
        &mut self,
        hint: <<<E as entity::Ref>::Archetype as Archetype>::Ealloc as Ealloc>::AllocHint,
        components: comp::Map<<E as entity::Ref>::Archetype>,
    ) -> Entity<<E as entity::Ref>::Archetype> {
        let ealloc = match self.ealloc_map.map.get_mut(&TypeId::of::<E::Archetype>()) {
            Some(ealloc) => ealloc,
            None => panic!(
                "Cannot create entity for archetype {} because it is not used in any systems",
                any::type_name::<E::Archetype>()
            ),
        };
        let ealloc = ealloc
            .as_any_mut()
            .downcast_mut::<<E::Archetype as Archetype>::Ealloc>()
            .expect("TypeId mismatch");
        let id = ealloc.allocate(hint);

        let typed = self.components.archetype_mut::<E::Archetype>();
        typed.init_entity(id, components);

        Entity::new_allocated(id)
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
            Some(global) => global,
            None => panic!(
                "The global state {} cannot be retrieved becaues it is not used in any systems",
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
            Some(global) => global,
            None => panic!(
                "The global state {} cannot be retrieved becaues it is not used in any systems",
                any::type_name::<G>()
            ),
        };
        global.downcast_mut::<G>().expect("TypeId mismatch")
    }
}

#[cfg(test)]
pub(crate) mod tests;
