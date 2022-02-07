//! The actual storage where components are owned.

use std::any::{self, Any};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::RwLock;

use xias::Xias;

use crate::optvec::OptVec;
use crate::{archetype, component, entity, Archetype, Component};

/// Stores data for a specific component type and instance ord.
pub struct Column<A: Archetype, C: Component>
where
    A: archetype::Contains<C>,
{
    pub(crate) storage: RwLock<ColumnStorage<C>>,
    factory: component::Factory<C>,
    _ph: PhantomData<&'static A>,
}

impl<A: Archetype, C: Component> Column<A, C>
where
    A: archetype::Contains<C>,
{
    pub(crate) fn new(factory: component::Factory<C>) -> Self {
        Self {
            storage: RwLock::new(ColumnStorage::default()),
            factory,
            _ph: PhantomData,
        }
    }
}

/// Trait object for columns of any types.
pub(crate) trait AnyColumn: Any {
    fn init(&mut self, entity: entity::RawId, initial: Option<Box<dyn Any>>);

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<A: Archetype, C: Component> AnyColumn for Column<A, C>
where
    A: archetype::Contains<C>,
{
    fn init(&mut self, entity: entity::RawId, initial: Option<Box<dyn Any>>) {
        let instance = if let Some(initial) = initial {
            match initial.downcast::<C>() {
                Ok(instance) => *instance,
                Err(any) => {
                    panic!(
                        "component::Initial TypeId mismatch, got type ID {:?}",
                        any.type_id()
                    );
                }
            }
        } else {
            let factory = match &self.factory {
                component::Factory::Optional => return,
                component::Factory::RequiredInput => panic!(
                    "Component type {c} is required for {a} construction but was not provided",
                    a = any::type_name::<A>(),
                    c = any::type_name::<C>(),
                ),
                component::Factory::AutoInit(factory) => factory,
            };

            factory()
        };

        match self.storage.get_mut().expect("another thread panicked") {
            ColumnStorage::Tree(map) => {
                map.insert(entity, instance);
            }
            ColumnStorage::Vec(vec) => {
                let offset = entity.small_int::<usize>();
                vec.resize_at_least(offset);
                vec.replace(offset, Some(instance));
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) enum ColumnStorage<C: Component> {
    Vec(OptVec<C>),
    Tree(BTreeMap<entity::RawId, C>),
}

impl<C: Component> Default for ColumnStorage<C> {
    fn default() -> Self {
        Self::Vec(OptVec::default())
    }
}

impl<C: Component> ColumnStorage<C> {
    pub(crate) fn get(&self, index: u32) -> Option<&C> {
        match self {
            Self::Vec(vec) => vec.get(index.small_int()),
            Self::Tree(map) => map.get(&index),
        }
    }
    pub(crate) fn get_mut(&mut self, index: u32) -> Option<&mut C> {
        match self {
            Self::Vec(vec) => vec.get_mut(index.small_int()),
            Self::Tree(map) => map.get_mut(&index),
        }
    }
}
