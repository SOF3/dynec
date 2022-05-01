use std::any::{self, Any, TypeId};
use std::collections::{hash_map, HashMap};
use std::sync::Arc;

use parking_lot::RwLock;

use super::storage;
use crate::{comp, entity, Archetype};

pub(crate) trait AnyBuilder {
    fn add_simple_storage_if_missing(&mut self, component: TypeId, shared: fn() -> Box<dyn Any>);

    fn add_isotope_factory_if_missing(&mut self, component: TypeId, shared: fn() -> Box<dyn Any>);

    fn build(self: Box<Self>) -> Box<dyn AnyTyped>;
}

pub(crate) fn builder<A: Archetype>() -> impl AnyBuilder {
    Builder::<A> { simple_storages: HashMap::new(), isotope_factories: HashMap::new() }
}

struct Builder<A: Archetype> {
    simple_storages:   HashMap<TypeId, storage::SharedSimple<A>>,
    isotope_factories: HashMap<TypeId, Box<dyn storage::AnyIsotopeFactory<A>>>,
}

impl<A: Archetype> AnyBuilder for Builder<A> {
    fn add_simple_storage_if_missing(&mut self, component: TypeId, shared: fn() -> Box<dyn Any>) {
        let shared: storage::SharedSimple<A> = match shared().downcast() {
            Ok(ss) => *ss,
            Err(_) => panic!(
                "Expected storage::SharedSimple<{}>, got {:?}",
                any::type_name::<A>(),
                shared.type_id()
            ),
        };
        self.simple_storages.entry(component).or_insert_with(|| shared);
    }

    fn add_isotope_factory_if_missing(&mut self, component: TypeId, shared: fn() -> Box<dyn Any>) {
        todo!()
    }

    fn build(mut self: Box<Self>) -> Box<dyn AnyTyped> {
        let populators = toposort_populators(&mut self.simple_storages);

        Box::new(Typed::<A> {
            ealloc: entity::Ealloc::default(),
            simple_storages: self.simple_storages,
            isotope_storages: RwLock::new(HashMap::new()),
            isotope_factories: self.isotope_factories,
            populators,
        })
    }
}

fn toposort_populators<A: Archetype>(
    storages: &mut HashMap<TypeId, storage::SharedSimple<A>>,
) -> Vec<Box<dyn Fn(&mut comp::Map<A>)>> {
    let mut populators = Vec::new();

    struct Request<A: Archetype> {
        dep_count: usize,
        populator: Box<dyn Fn(&mut comp::Map<A>)>,
    }

    let mut unprocessed = Vec::new();
    for (&ty, storage) in storages {
        let storage =
            Arc::get_mut(storage).expect("builder should own unique reference to storages");
        let storage = storage.get_mut();
        match storage.init_strategy() {
            comp::SimpleInitStrategy::None => continue, /* direct requirement, does not affect population */
            comp::SimpleInitStrategy::Auto(initer) => unprocessed.push((ty, initer.f)),
        };
    }

    let mut requests = HashMap::<TypeId, Request<A>>::new();
    let mut dependents_map = HashMap::<TypeId, Vec<TypeId>>::new(); // all values here must also have an entry in requests before popping
    let mut heads = Vec::<TypeId>::new(); // all entries here must also have an entry in requests

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
            any::type_name::<A>()
        );
    }

    populators
}
// TODO unit test toposort_populators

#[derive(Default)]
pub(crate) struct Typed<A: Archetype> {
    pub(crate) ealloc:            entity::Ealloc,
    pub(crate) simple_storages:   HashMap<TypeId, storage::SharedSimple<A>>,
    pub(crate) isotope_storages:  RwLock<HashMap<comp::any::Identifier, storage::SharedSimple<A>>>,
    pub(crate) isotope_factories: HashMap<TypeId, Box<dyn storage::AnyIsotopeFactory<A>>>,
    pub(crate) populators:        Vec<Box<dyn Fn(&mut comp::Map<A>)>>,
}

impl<A: Archetype> Typed<A> {
    pub(crate) fn create_near(
        &mut self,
        near: Option<entity::Raw>,
        mut components: comp::Map<A>,
    ) -> entity::Raw {
        let id = match near {
            Some(hint) => self.ealloc.allocate_near(hint),
            None => self.ealloc.allocate(),
        };

        for populate in &self.populators {
            populate(&mut components);
        }

        for storage in self.simple_storages.values_mut() {
            let storage = Arc::get_mut(storage).expect("storage arc was leaked");
            let storage = storage.get_mut();
            storage.init_with(id, &mut components);
        }

        // TODO extract isotope components

        id
    }
}

pub(crate) trait AnyTyped {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<A: Archetype> AnyTyped for Typed<A> {
    fn as_any(&self) -> &dyn Any { self }

    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
