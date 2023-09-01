use std::any::{self, Any};
use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use parking_lot::lock_api::ArcRwLockWriteGuard;

use crate::entity::{self, referrer};
use crate::storage::simple::AnySimpleStorage;
use crate::util::DbgTypeId;
use crate::{comp, storage, Archetype};

pub(crate) trait AnyBuilder {
    fn add_simple_storage_if_missing(
        &mut self,
        component: DbgTypeId,
        storage_builder: fn() -> Box<dyn Any>,
    );

    fn add_isotope_map_if_missing(
        &mut self,
        component: DbgTypeId,
        map_builder: fn() -> Box<dyn Any>,
    );

    fn build(self: Box<Self>) -> Box<dyn AnyTyped>;
}

pub(crate) fn builder<A: Archetype>() -> impl AnyBuilder {
    let mut builder = Builder::<A> {
        simple_storages:      IndexMap::new(),
        isotope_storage_maps: HashMap::new(),
    };

    // Native components from dynec that must be present for every archetype.
    populate_native_component::<A, entity::deletion::Flag>(&mut builder);

    builder
}

fn populate_native_component<A: Archetype, C: comp::Simple<A>>(builder: &mut Builder<A>) {
    builder.simple_storages.insert(DbgTypeId::of::<C>(), storage::Simple::new::<C>());
}

struct Builder<A: Archetype> {
    simple_storages:      IndexMap<DbgTypeId, storage::Simple<A>>,
    isotope_storage_maps: HashMap<DbgTypeId, Arc<dyn storage::AnyIsotopeMap<A>>>,
}

impl<A: Archetype> AnyBuilder for Builder<A> {
    // TODO: add unit tests to ensure that simple_storages is always toposorted
    fn add_simple_storage_if_missing(
        &mut self,
        component: DbgTypeId,
        storage_builder: fn() -> Box<dyn Any>,
    ) {
        if !self.simple_storages.contains_key(&component) {
            let boxed = storage_builder();
            let storage = match boxed.downcast::<storage::Simple<A>>() {
                Ok(storage) => *storage,
                Err(boxed) => panic!(
                    "Expected storage::Simple<{}>, got {:?}",
                    any::type_name::<A>(),
                    (*boxed).type_id(),
                ),
            };

            let deps = &storage.dep_list;
            for &(dep_ty, dep_storage_builder) in deps {
                self.add_simple_storage_if_missing(dep_ty, dep_storage_builder);
            }

            // insert the new component after all dependencies have been inserted
            self.simple_storages.insert(component, storage);
        }
    }

    fn add_isotope_map_if_missing(
        &mut self,
        component: DbgTypeId,
        map_builder: fn() -> Box<dyn Any>,
    ) {
        self.isotope_storage_maps.entry(component).or_insert_with(|| {
            let boxed = map_builder();
            match boxed.downcast::<Arc<dyn storage::AnyIsotopeMap<A>>>() {
                Ok(factory) => *factory,
                Err(boxed) => panic!(
                    "Expected storage::isotope::AnyMap<{}>, got {:?}",
                    any::type_name::<A>(),
                    (*boxed).type_id(),
                ),
            }
        });
    }

    fn build(self: Box<Self>) -> Box<dyn AnyTyped> {
        Box::new(Typed::<A> {
            simple_storages:      self.simple_storages,
            isotope_storage_maps: self.isotope_storage_maps,
        })
    }
}

/// Stores everything related to a specific archetype.
#[derive(Default)]
pub(crate) struct Typed<A: Archetype> {
    pub(crate) simple_storages:      IndexMap<DbgTypeId, storage::Simple<A>>,
    pub(crate) isotope_storage_maps: HashMap<DbgTypeId, Arc<dyn storage::AnyIsotopeMap<A>>>,
}

impl<A: Archetype> Typed<A> {
    /// Initialize an entity. This function should only be called offline.
    pub(crate) fn init_entity(
        &mut self,
        entity: A::RawEntity,
        mut comp_map: comp::Map<A>,
        ealloc: &mut A::Ealloc,
    ) {
        struct DepGetter<'t, A: Archetype> {
            simple_storages: &'t IndexMap<DbgTypeId, storage::Simple<A>>,
            index:           Option<usize>,
            entity:          A::RawEntity,
        }
        impl<'t, A: Archetype> comp::any::DepGetterInner<A> for DepGetter<'t, A> {
            fn get(
                &self,
                ty: DbgTypeId,
            ) -> ArcRwLockWriteGuard<parking_lot::RawRwLock, dyn AnySimpleStorage<A>> {
                let (dep_index, _, dep_storage) = self
                    .simple_storages
                    .get_full(&ty)
                    .expect("dep storage does not exist, toposort bug");
                if let Some(index) = self.index {
                    assert!(dep_index < index, "{dep_index} >= {index}, toposort bug");
                }
                dep_storage.storage.try_write_arc().expect(
                    "mut access to indexmap and dep indices checked to be unique during toposort",
                )
            }
        }

        for (index, storage) in self.simple_storages.values().enumerate() {
            let mut any_storage = storage.storage.try_write().expect("storage arc was leaked");

            any_storage.fill_init_simple(
                entity,
                &mut comp_map,
                comp::any::DepGetter {
                    inner: &DepGetter {
                        simple_storages: &self.simple_storages,
                        index: Some(index),
                        entity,
                    },
                    entity,
                },
            );
        }

        for map in self.isotope_storage_maps.values_mut() {
            Arc::get_mut(map).expect("storage map arc was leaked").fill_init_isotope(
                entity,
                &mut comp_map,
                comp::any::DepGetter {
                    inner: &DepGetter {
                        simple_storages: &self.simple_storages,
                        index: None,
                        entity,
                    },
                    entity,
                },
                ealloc,
            );
        }
    }
}

pub(crate) trait AnyTyped: Send + Sync {
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

    fn referrer_dyn_iter<'t>(&'t mut self, archetype: &'t str) -> Box<dyn referrer::Object + 't>;
}

impl<A: Archetype> AnyTyped for Typed<A> {
    fn as_any(&self) -> &(dyn Any + Send + Sync) { self }
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) { self }

    fn referrer_dyn_iter<'t>(&'t mut self, archetype: &'t str) -> Box<dyn referrer::Object + 't> {
        Box::new(referrer::NamedBoxIter(
            self.simple_storages
                .iter_mut()
                .map(move |(comp_ty, storage)| {
                    let referrer_dyn = Arc::get_mut(&mut storage.storage)
                        .expect("storage arc was leaked")
                        .get_mut()
                        .referrer_dyn();
                    (Some(format!("{archetype} / {comp_ty}")), referrer_dyn)
                })
                .chain(self.isotope_storage_maps.iter_mut().map(move |(comp_ty, storage)| {
                    let storage = Arc::get_mut(storage).expect("storage arc was leaked");
                    (Some(format!("{archetype} / {comp_ty}")), storage.referrer_dyn())
                })),
        ))
    }
}
