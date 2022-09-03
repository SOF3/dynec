//! Last owner of reference-counted entity references,
//! used for identifying strong reference leaks.
#![cfg(any(
    all(debug_assertions, feature = "debug-entity-rc"),
    all(not(debug_assertions), feature = "release-entity-rc"),
))]

use std::any::TypeId;
use std::collections::HashMap;
use std::sync;

use crate::util::DbgTypeId;
use crate::Archetype;

/// Stores reference counters of entities for a specific archetype.
#[derive(Default)]
pub(crate) struct Store {
    vec: Vec<Option<sync::Arc<()>>>,
}

impl Store {
    pub(crate) fn set(&mut self, id: usize, rc: sync::Arc<()>) {
        if self.vec.len() <= id {
            self.vec.resize(id + 1, None);
        }

        let opt = self.vec.get_mut(id).expect("just resized");
        assert!(opt.is_none(), "Previous entity was not freed correctly");
        *opt = Some(rc);
    }

    pub(crate) fn remove(&mut self, id: usize) -> sync::Arc<()> {
        let opt =
            self.vec.get_mut(id).expect("call to rctrack::Store::remove() with nonexistent ID");
        opt.take().expect("double free of entity last strong reference")
    }
}

/// A map of generation stores for each archetype.
#[crate::global(dynec_as(crate))]
#[derive(Default)]
pub(crate) struct StoreMap {
    map: HashMap<DbgTypeId, Store>,
}

impl StoreMap {
    /// Starts tracking a strong reference.
    pub(crate) fn set<A: Archetype>(&mut self, id: usize, rc: sync::Arc<()>) {
        self.map.entry(DbgTypeId::of::<A>()).or_default().set(id, rc)
    }

    /// Removes and returns the current strong reference to an entity.
    pub(crate) fn remove<A: Archetype>(&mut self, id: usize) -> sync::Arc<()> {
        self.map
            .get_mut(&TypeId::of::<A>())
            .expect("call to remove() with unexpected archetype")
            .remove(id)
    }
}
