use std::any::Any;
use std::sync::Arc;

use parking_lot::RwLock;

use super::Storage;
use crate::{comp, Archetype};

pub(crate) struct Isotope<A: Archetype> {
    /// The actual storage object. Downcasts to `C::Storage`.
    pub(crate) storage:           Arc<RwLock<dyn Any + Send + Sync>>,
    /// This is a function pointer to [`fn@fill_init_isotope`] with the correct type parameters.
    pub(crate) fill_init_isotope: fn(&mut dyn Any, A::RawEntity, Box<dyn Any>),
}

impl<A: Archetype> Isotope<A> {
    pub(crate) fn new<C: comp::Isotope<A>>() -> Self {
        Self {
            storage:           Arc::new(RwLock::new(C::Storage::default()))
                as Arc<RwLock<dyn Any + Send + Sync>>,
            fill_init_isotope: fill_init_isotope::<A, C>,
        }
    }
}

fn fill_init_isotope<A: Archetype, C: comp::Isotope<A>>(
    storage: &mut dyn Any,
    entity: A::RawEntity,
    comp: Box<dyn Any>,
) {
    let storage: &mut C::Storage = storage.downcast_mut().expect("function pointer mismatch");
    let comp = *comp.downcast::<C>().expect("function pointer and TypeId mismatch");
    storage.set(entity, Some(comp));
}

pub(crate) struct Factory<A: Archetype> {
    builder: fn() -> Isotope<A>, // TODO
}

impl<A: Archetype> Factory<A> {
    pub(crate) fn new<C: comp::Isotope<A>>() -> Self { Self { builder: Isotope::<A>::new::<C> } }

    pub(crate) fn build(&self) -> Isotope<A> { (self.builder)() }
}
