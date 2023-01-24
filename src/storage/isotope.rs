use std::any::{self, Any};
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use super::Storage;
use crate::entity::referrer;
use crate::{comp, Archetype};

pub(crate) type MapInner<A, C> = HashMap<
    <C as comp::Isotope<A>>::Discrim,
    Arc<RwLock<<C as comp::SimpleOrIsotope<A>>::Storage>>,
>;

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

    /// Fills all entries. Called during entity initialization.
    fn fill_init_isotope(
        &mut self,
        entity: A::RawEntity,
        comp_map: &mut comp::Map<A>,
        dep_getter: comp::any::DepGetter<'_, A>,
    );

    fn referrer_dyn<'t>(&'t mut self) -> Box<dyn referrer::Object + 't>;
}

impl<A: Archetype, C: comp::Isotope<A>> AnyMap<A> for Map<A, C> {
    fn as_any(&self) -> &(dyn Any + Send + Sync) { self }
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) { self }

    fn fill_init_isotope(
        &mut self,
        entity: <A as Archetype>::RawEntity,
        comp_map: &mut comp::Map<A>,
        dep_getter: comp::any::DepGetter<'_, A>,
    ) {
        let map = self.map.get_mut();
        let values = comp_map.remove_isotope::<C>();

        for (discrim, value) in values {
            let storage = map.entry(discrim).or_insert_with(Arc::<RwLock<C::Storage>>::default);
            let storage = Arc::get_mut(storage).expect("storage arc was leaked").get_mut();
            storage.set(entity, Some(value));
        }

        // TODO process init strategy
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
