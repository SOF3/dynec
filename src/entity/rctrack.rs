//! Last owner of reference-counted entity references,
//! used for identifying strong reference leaks.

use crate::{entity, Archetype, Entity};

#[cfg(any(
    all(debug_assertions, feature = "debug-entity-rc"),
    all(not(debug_assertions), feature = "release-entity-rc"),
))]
mod inner {
    use std::any::TypeId;
    use std::collections::HashMap;
    use std::sync;
    use std::sync::Arc;

    use crate::entity::Raw;
    use crate::util::DbgTypeId;
    use crate::{entity, Archetype, Entity};

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

        pub fn get(&self, id: usize) -> Option<&sync::Arc<()>> {
            self.vec.get(id).and_then(Option::as_ref)
        }
    }

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

        pub(super) fn to_strong<A: Archetype>(&self, entity: entity::TempRef<'_, A>) -> Entity<A> {
            let archetype = self.map.get(&TypeId::of::<A>()).expect("entity archetype is unknown");
            let arc = archetype.get(entity.value.to_primitive()).expect("entity does not exist");
            Entity { id: entity.value, rc: Arc::clone(arc) }
        }
    }
}

#[cfg(not(any(
    all(debug_assertions, feature = "debug-entity-rc"),
    all(not(debug_assertions), feature = "release-entity-rc"),
)))]
mod inner {
    use crate::{entity, Archetype, Entity};

    /// A dummy StoreMap that implements `to_string` without any lookup or arc clone.
    #[derive(Default)]
    pub(crate) struct StoreMap(());

    impl StoreMap {
        pub(super) fn to_strong<A: Archetype>(&self, entity: entity::TempRef<'_, A>) -> Entity<A> {
            Entity { id: entity.value, rc: entity::maybe::MaybeArc }
        }
    }
}

/// A map of rctrack stores for each archetype.
#[derive(Default)]
pub struct MaybeStoreMap(pub(crate) inner::StoreMap);

impl MaybeStoreMap {
    /// Converts a temporary reference to a `'static` strong reference.
    pub fn to_strong<A: Archetype>(&self, entity: entity::TempRef<'_, A>) -> Entity<A> {
        self.0.to_strong(entity)
    }
}
