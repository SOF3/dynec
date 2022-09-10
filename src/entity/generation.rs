//! Tracks the number of times an entity ID is allocated,
//! used for distinguishment of dangling weak references.

use std::any::TypeId;
use std::collections::HashMap;

use crate::util::DbgTypeId;
use crate::Archetype;

/// The number of times the same entry has been used for allocating an entity.
/// This type is fully ordered, where a greater generation implies newer version.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Generation(u32);

/// Stores generations of entities for a specific archetype.
#[derive(Default)]
pub struct Store {
    vec: Vec<Generation>,
}

impl Store {
    /// Bumps the generation of the entity.
    pub fn next(&mut self, id: usize) -> Generation {
        if self.vec.len() <= id {
            self.vec.resize(id + 1, Generation::default());
        }

        let generation = self.vec.get_mut(id).expect("just resized");
        generation.0 = generation.0.wrapping_add(1);
        *generation
    }

    /// Gets the generation of the last created entity with the given `id`.
    pub fn get(&self, id: usize) -> Generation { self.vec.get(id).copied().unwrap_or_default() }
}

/// A map of generation stores for each archetype.
#[crate::global(dynec_as(crate))]
#[derive(Default)]
pub struct StoreMap {
    map: HashMap<DbgTypeId, Store>,
}

impl StoreMap {
    /// Bumps the generation of the entity with the given archetype.
    pub fn next<A: Archetype>(&mut self, id: usize) -> Generation {
        self.map.entry(DbgTypeId::of::<A>()).or_default().next(id)
    }

    /// Gets the generation of the last created entity with the given archetype and `id`.
    pub fn get<A: Archetype>(&self, id: usize) -> Generation {
        match self.map.get(&TypeId::of::<A>()) {
            Some(store) => store.get(id),
            None => Generation::default(),
        }
    }
}

/// Parameter to [`super::Entity::weak`].
pub trait WeakStore {
    /// Resolves the actual generation store for the archetype.
    fn resolve<A: Archetype>(&self) -> Option<&Store>;
}

impl WeakStore for Store {
    fn resolve<A: Archetype>(&self) -> Option<&Store> { Some(self) }
}

impl WeakStore for StoreMap {
    fn resolve<A: Archetype>(&self) -> Option<&Store> { self.map.get(&TypeId::of::<A>()) }
}
