use std::any::TypeId;
use std::collections::BTreeMap;

use parking_lot::RwLock;

use crate::{component, entity, Archetype, Entity};

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

/// Identifies an archetype + component type.
struct ArchComp {
    arch: TypeId,
    comp: TypeId,
}

/// Identifies an archetype + component type + discriminant.
struct ComponentIdentifier {
    arch: TypeId,
    comp: component::any::Identifier,
}

/// Describes a simple component type.
struct SimpleSpec {
    presence:     component::SimplePresence,
    // TODO wrap SimpleInitStrategy in a trait object
    is_finalizer: bool,
}

impl SimpleSpec {
    fn of<A: Archetype, C: component::Simple<A>>() -> Self {
        Self { presence: C::PRESENCE, is_finalizer: C::IS_FINALIZER }
    }
}

/// Describes an isotope component type.
struct IsotopeSpec {
    // TODO wrap IsotopeInitStrategy in a trait object
}

impl IsotopeSpec {
    fn of<A: Archetype, C: component::Isotope<A>>() -> Self { Self {} }
}

mod builder;
pub use builder::Builder;

mod storage;

pub struct World {
    storages: RwLock<BTreeMap<ComponentIdentifier, storage::Ref>>,
}

impl World {
    /// Adds an entity to the world.
    pub fn create<A: Archetype>(&mut self, components: component::Map<A>) -> Entity<A> {
        self.create_near::<A, entity::Entity<A>>(None, components)
    }

    /// Adds an entity to the world near another entity.
    pub fn create_near<A: Archetype, E: entity::Ref<A>>(
        &mut self,
        near: Option<E>,
        components: component::Map<A>,
    ) -> Entity<A> {
        todo!()
    }
}
