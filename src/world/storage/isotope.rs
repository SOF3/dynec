use std::any::Any;
use std::sync::Arc;

use parking_lot::RwLock;

use super::Storage;
use crate::entity::referrer;
use crate::{comp, Archetype};

pub(crate) struct Isotope<A: Archetype> {
    /// The actual storage object. Downcasts to `C::Storage`.
    pub(crate) storage: Arc<RwLock<dyn AnyIsotopeStorage<A>>>,
}

impl<A: Archetype> Isotope<A> {
    pub(crate) fn new<C: comp::Isotope<A>>() -> Self {
        Self {
            storage: Arc::new(RwLock::new(IsotopeStorage::<A, C>(C::Storage::default())))
                as Arc<RwLock<dyn AnyIsotopeStorage<A>>>,
        }
    }
}

impl<A: Archetype> Clone for Isotope<A> {
    fn clone(&self) -> Self { Self { storage: Arc::clone(&self.storage) } }
}

/// Downcasts to `C::Storage`
pub(crate) trait AnyIsotopeStorage<A: Archetype>: Send + Sync {
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

    /// Adds an entry to the storage, used during entity initialization.
    /// `comp` downcasts to `C`.
    fn fill_init_isotope(&mut self, entity: A::RawEntity, comp: Box<dyn Any>);

    /// Returns a [`referrer::Dyn`] implementation that visits all components in this storage.
    fn referrer_dyn<'t>(&'t mut self) -> Box<dyn referrer::Object + 't>;
}

impl<A: Archetype> dyn AnyIsotopeStorage<A> {
    pub(crate) fn downcast_ref<C: comp::Isotope<A>>(&self) -> &C::Storage {
        &self.as_any().downcast_ref::<IsotopeStorage<A, C>>().expect("TypeId mismatch").0
    }

    pub(crate) fn downcast_mut<C: comp::Isotope<A>>(&mut self) -> &mut C::Storage {
        &mut self.as_any_mut().downcast_mut::<IsotopeStorage<A, C>>().expect("TypeId mismatch").0
    }
}

struct IsotopeStorage<A: Archetype, C: comp::Isotope<A>>(C::Storage);

impl<A: Archetype, C: comp::Isotope<A>> AnyIsotopeStorage<A> for IsotopeStorage<A, C> {
    fn as_any(&self) -> &(dyn Any + Send + Sync) { self }
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) { self }

    fn fill_init_isotope(&mut self, entity: A::RawEntity, comp: Box<dyn Any>) {
        let comp = *comp.downcast::<C>().expect("function pointer and TypeId mismatch");
        self.0.set(entity, Some(comp));
    }

    fn referrer_dyn<'t>(&'t mut self) -> Box<dyn referrer::Object + 't> {
        Box::new(referrer::ReferrerIter(self.0.iter_chunks_mut().flat_map(|chunk| chunk.slice)))
    }
}

pub(crate) struct Factory<A: Archetype> {
    builder: fn() -> Isotope<A>, // TODO
}

impl<A: Archetype> Factory<A> {
    pub(crate) fn new<C: comp::Isotope<A>>() -> Self { Self { builder: Isotope::<A>::new::<C> } }

    pub(crate) fn build(&self) -> Isotope<A> { (self.builder)() }
}
