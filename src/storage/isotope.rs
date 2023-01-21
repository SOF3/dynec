use std::any::{self, Any};
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use super::Storage;
use crate::entity::referrer;
use crate::{comp, Archetype};

pub(crate) type MapInner<A, C> =
    HashMap<<C as comp::Isotope<A>>::Discrim, Arc<RwLock<<C as comp::Isotope<A>>::Storage>>>;

/// Isotope storages of the same type but different discriminants.
pub(crate) struct Map<A: Archetype, C: comp::Isotope<A>> {
    pub(crate) map: RwLock<MapInner<A, C>>,
}

impl<A: Archetype, C: comp::Isotope<A>> Map<A, C> {
    pub(crate) fn new_any() -> Arc<dyn AnyMap<A>> { Arc::new(Self { map: RwLock::default() }) }
}

/// Downcastable trait object of [`Map`].
pub(crate) trait AnyMap<A: Archetype>: Send + Sync {
    fn as_any(&self) -> &(dyn Any + Send + Sync);
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

    fn init_strategy(&self) -> &'static comp::InitStrategy<A>;

    /// Fills an entry. Called during entity initialization.
    fn fill_init(
        &mut self,
        discrim: usize,
        entity: A::RawEntity,
        value: Box<dyn Any + Send + Sync>,
    );

    fn referrer_dyn<'t>(&'t mut self) -> Box<dyn referrer::Object + 't>;
}

impl<A: Archetype, C: comp::Isotope<A>> AnyMap<A> for Map<A, C> {
    fn as_any(&self) -> &(dyn Any + Send + Sync) { self }
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) { self }

    fn init_strategy(&self) -> &'static comp::InitStrategy<A> { &C::INIT_STRATEGY }

    fn fill_init(
        &mut self,
        discrim: usize,
        entity: A::RawEntity,
        value: Box<dyn Any + Send + Sync>,
    ) {
        let storage: &mut Arc<RwLock<C::Storage>> = self
            .map
            .get_mut()
            .entry(<C::Discrim as comp::Discrim>::from_usize(discrim))
            .or_insert_with(Arc::<RwLock<C::Storage>>::default);
        let storage = Arc::get_mut(storage).expect("storage arc was leaked");
        let value = value.downcast::<C>().expect("TypeId mismatch");
        storage.get_mut().set(entity, Some(*value));
    }

    fn referrer_dyn<'t>(&'t mut self) -> Box<dyn referrer::Object + 't> {
        Box::new(referrer::NamedIter(self.map.get_mut().iter_mut().map(|(discrim, value)| {
            let storage: &mut C::Storage =
                Arc::get_mut(value).expect("storage arc was leaked").get_mut();
            (
                Some(format!(
                    "{} / {} # {discrim:?}",
                    any::type_name::<A>(),
                    any::type_name::<C>()
                )),
                referrer::UnnamedIter(storage.iter_chunks_mut().flat_map(|chunk| chunk.slice)),
            )
        })))
    }
}

impl<A: Archetype> dyn AnyMap<A> {
    pub(crate) fn downcast_ref<C: comp::Isotope<A>>(&self) -> &Map<A, C> {
        self.as_any().downcast_ref().expect("TypeId mismatch")
    }

    pub(crate) fn downcast_mut<C: comp::Isotope<A>>(&mut self) -> &mut Map<A, C> {
        self.as_any_mut().downcast_mut().expect("TypeId mismatch")
    }
}
