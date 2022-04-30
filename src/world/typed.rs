use std::any::{self, Any, TypeId};
use std::collections::HashMap;

use parking_lot::RwLock;

use super::storage;
use crate::{component, entity, Archetype};

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

    fn build(self: Box<Self>) -> Box<dyn AnyTyped> {
        Box::new(Typed::<A> {
            ealloc:            entity::Ealloc::default(),
            simple_storages:   self.simple_storages,
            isotope_storages:  RwLock::new(HashMap::new()),
            isotope_factories: self.isotope_factories,
        })
    }
}

#[derive(Default)]
pub(crate) struct Typed<A: Archetype> {
    pub(crate) ealloc:            entity::Ealloc,
    pub(crate) simple_storages:   HashMap<TypeId, storage::SharedSimple<A>>,
    pub(crate) isotope_storages:
        RwLock<HashMap<component::any::Identifier, storage::SharedSimple<A>>>,
    pub(crate) isotope_factories: HashMap<TypeId, Box<dyn storage::AnyIsotopeFactory<A>>>,
}

pub(crate) trait AnyTyped {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<A: Archetype> AnyTyped for Typed<A> {
    fn as_any(&self) -> &dyn Any { self }

    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
