use std::any::{self, Any};
use std::collections::{hash_map, HashMap};
use std::sync::Arc;

use parking_lot::RwLock;

use super::storage;
use crate::util::DbgTypeId;
use crate::{comp, Archetype};

pub(crate) trait AnyBuilder {
    fn add_simple_storage_if_missing(&mut self, component: DbgTypeId, shared: fn() -> Box<dyn Any>);

    fn add_isotope_factory_if_missing(
        &mut self,
        component: DbgTypeId,
        shared: fn() -> Box<dyn Any>,
    );

    fn build(self: Box<Self>) -> Box<dyn AnyTyped>;
}

pub(crate) fn builder<A: Archetype>() -> impl AnyBuilder {
    Builder::<A> { simple_storages: HashMap::new(), isotope_factories: HashMap::new() }
}

struct Builder<A: Archetype> {
    simple_storages:   HashMap<DbgTypeId, storage::Simple<A>>,
    isotope_factories: HashMap<DbgTypeId, storage::IsotopeFactory<A>>,
}

impl<A: Archetype> AnyBuilder for Builder<A> {
    fn add_simple_storage_if_missing(
        &mut self,
        component: DbgTypeId,
        shared: fn() -> Box<dyn Any>,
    ) {
        let shared: storage::Simple<A> = match shared().downcast() {
            Ok(ss) => *ss,
            Err(_) => panic!(
                "Expected storage::SharedSimple<{}>, got {:?}",
                any::type_name::<A>(),
                shared.type_id(),
            ),
        };
        self.simple_storages.entry(component).or_insert_with(|| shared);
    }

    fn add_isotope_factory_if_missing(
        &mut self,
        component: DbgTypeId,
        shared: fn() -> Box<dyn Any>,
    ) {
        todo!()
    }

    fn build(mut self: Box<Self>) -> Box<dyn AnyTyped> {
        let populators = toposort_populators(&mut self.simple_storages);

        Box::new(Typed::<A> {
            simple_storages: self.simple_storages,
            isotope_storages: RwLock::new(HashMap::new()),
            isotope_factories: self.isotope_factories,
            populators,
        })
    }
}

fn toposort_populators<A: Archetype>(
    storages: &mut HashMap<DbgTypeId, storage::Simple<A>>,
) -> Vec<Box<dyn Fn(&mut comp::Map<A>) + Send + Sync>> {
    let mut populators = Vec::new();

    struct Request<A: Archetype> {
        dep_count: usize,
        populator: Box<dyn Fn(&mut comp::Map<A>) + Send + Sync>,
    }

    let mut unprocessed = Vec::new();
    for (&ty, storage) in storages {
        match &storage.init_strategy {
            comp::SimpleInitStrategy::None => continue, /* direct requirement, does not affect population */
            comp::SimpleInitStrategy::Auto(initer) => unprocessed.push((ty, initer.f)),
        };
    }

    let mut requests = HashMap::<DbgTypeId, Request<A>>::new();
    let mut dependents_map = HashMap::<DbgTypeId, Vec<DbgTypeId>>::new(); // all values here must also have an entry in requests before popping
    let mut heads = Vec::<DbgTypeId>::new(); // all entries here must also have an entry in requests

    while let Some((ty, desc)) = unprocessed.pop() {
        let deps = desc.deps();

        let request = if let hash_map::Entry::Vacant(entry) = requests.entry(ty) {
            entry.insert(Request { dep_count: 0, populator: Box::new(|map| desc.populate(map)) })
        } else {
            continue;
        };

        for (dep_ty, dep_strategy) in deps {
            dependents_map.entry(dep_ty).or_default().push(ty); // ty is pushed to unprocessed, which will fill requests later
            match dep_strategy {
                // required dependency, does not affect population
                comp::SimpleInitStrategy::None => continue,
                // push to unprocessed again to recurse
                comp::SimpleInitStrategy::Auto(initer) => {
                    request.dep_count += 1;
                    unprocessed.push((dep_ty, initer.f));
                }
            }
        }

        if request.dep_count == 0 {
            heads.push(ty); // requests.entry(ty) inserted above
        }
    }

    while let Some(head) = heads.pop() {
        let request = requests.remove(&head).expect("type is in heads but not in requests");
        assert_eq!(request.dep_count, 0);
        populators.push(request.populator);

        if let Some(dependents) = dependents_map.get(&head) {
            for &dependent in dependents {
                let request = requests
                    .get_mut(&dependent)
                    .expect("type is a value in dependents_map but not in requests");
                request.dep_count -= 1;
                if request.dep_count == 0 {
                    heads.push(dependent); // requests.get_mut(&dependent) returned Some
                }
            }
        }
    }

    if !requests.is_empty() {
        panic!(
            "Cyclic dependency detected for component initializers of {}",
            any::type_name::<A>(),
        );
    }

    populators
}
// TODO unit test toposort_populators

/// Stores everything related to a specific archetype.
#[derive(Default)]
pub(crate) struct Typed<A: Archetype> {
    pub(crate) simple_storages:   HashMap<DbgTypeId, storage::Simple<A>>,
    pub(crate) isotope_storages:  RwLock<HashMap<comp::any::Identifier, storage::Isotope<A>>>,
    pub(crate) isotope_factories: HashMap<DbgTypeId, storage::IsotopeFactory<A>>,
    pub(crate) populators:        Vec<Box<dyn Fn(&mut comp::Map<A>) + Send + Sync>>,
}

impl<A: Archetype> Typed<A> {
    /// Initialize an entity. This function should only be called offline.
    pub(crate) fn init_entity(&mut self, id: A::RawEntity, mut components: comp::Map<A>) {
        for populate in &self.populators {
            populate(&mut components);
        }

        for storage in self.simple_storages.values_mut() {
            let any_storage = Arc::get_mut(&mut storage.storage).expect("storage arc was leaked");
            (storage.fill_init_simple)(any_storage.get_mut(), id, &mut components);
        }

        // TODO extract isotope components
    }
}

pub(crate) trait AnyTyped: Send + Sync {
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);
}

impl<A: Archetype> AnyTyped for Typed<A> {
    fn as_any(&self) -> &(dyn Any + Send + Sync) { self }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) { self }
}
