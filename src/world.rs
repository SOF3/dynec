//! The world stores the states of the game.

use std::any::{self, TypeId};
use std::sync::Arc;

use crate::util::DbgTypeId;
use crate::{comp, entity, Archetype, Entity};

mod builder;
pub use builder::Builder;

mod state;
pub use state::{Components, SyncGlobals, UnsyncGlobals};

pub(crate) mod storage;
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
    // TODO wrap SimpleInitStrategy in a trait object
    is_finalizer: bool,
}

impl SimpleSpec {
    fn of<A: Archetype, C: comp::Simple<A>>() -> Self {
        Self { presence: C::PRESENCE, is_finalizer: C::IS_FINALIZER }
    }
}

/// The data structure that stores all states in the game.
pub struct World {
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
        );
    }

    /// Adds an entity to the world.
    pub fn create<A: Archetype>(&mut self, components: comp::Map<A>) -> Entity<A> {
        self.create_near::<entity::Entity<A>>(None, components)
    }

    /// Adds an entity to the world near another entity.
    pub fn create_near<E: entity::Ref>(
        &mut self,
        near: Option<E>,
        components: comp::Map<<E as entity::Ref>::Archetype>,
    ) -> Entity<<E as entity::Ref>::Archetype> {
        let typed = self.components.archetype_mut::<E::Archetype>();
        let id = typed.create_near(near.map(|raw| raw.id().0), components);
        Entity::new_allocated(id)
    }

    /// Gets a reference to an entity component when the world is not running.
    ///
    /// Requires a mutable reference to the world to ensure that the world is not executing in
    /// other systems.
    pub fn get_simple<A: Archetype, C: comp::Simple<A>, E: entity::Ref<Archetype = A>>(
        &mut self,
        entity: E,
    ) -> Option<&mut C> {
        let typed = self.components.archetype_mut::<A>();
        let storage = match typed.simple_storages.get_mut(&TypeId::of::<C>()) {
            Some(storage) => storage,
            None => panic!(
                "The component {} cannot be retrieved because it is not used in any systems",
                any::type_name::<C>()
            ),
        };
        let storage =
            Arc::get_mut(storage).expect("Storage Arc clones should not outlive system execution");
        let storage = storage.get_mut();
        let storage =
            storage.as_any_mut().downcast_mut::<storage::Storage<A, C>>().expect("TypeId mismatch");
        storage.get_mut(entity.id().0)
    }
}

#[cfg(test)]
mod tests;
