use std::any::{self, Any};
use std::collections::HashMap;
use std::ops;
use std::sync::Arc;

use parking_lot::lock_api::ArcRwLockWriteGuard;
use parking_lot::RwLock;

use super::Storage;
use crate::entity::{self, referrer, Ealloc};
use crate::{comp, storage, Archetype};

pub(crate) struct MapInner<A: Archetype, C: comp::Isotope<A>> {
    map: HashMap<C::Discrim, Arc<RwLock<C::Storage>>>,
}

impl<A: Archetype, C: comp::Isotope<A>> Default for MapInner<A, C> {
    fn default() -> Self { Self { map: HashMap::new() } }
}

/// We do not expose mutability to the HashMap
/// to protect other code from creating an empty storage directly.
/// However immutable reference is fine.
impl<A: Archetype, C: comp::Isotope<A>> MapInner<A, C> {
    pub(crate) fn map(&self) -> &HashMap<C::Discrim, Arc<RwLock<C::Storage>>> { &self.map }

    pub(crate) fn get_or_create(
        &mut self,
        discrim: C::Discrim,
        entities: impl Iterator<Item = ops::Range<A::RawEntity>>,
    ) -> &mut Arc<RwLock<C::Storage>> {
        self.map.entry(discrim).or_insert_with(|| {
            let mut storage = C::Storage::default();

            if let comp::InitStrategy::Auto(initer) = C::INIT_STRATEGY {
                for entity in entities.flat_map(<A::RawEntity as entity::Raw>::range) {
                    struct PanicDepGetter;
                    impl<A: Archetype> comp::any::DepGetterInner<A> for PanicDepGetter {
                        fn get(
                            &self,
                            _ty: crate::util::DbgTypeId,
                        ) -> ArcRwLockWriteGuard<
                            parking_lot::RawRwLock,
                            dyn storage::simple::AnySimpleStorage<A>,
                        > {
                            unimplemented!(
                                "Isotope initializers with dependencies are currently not \
                                 supported if discriminants are dynamically instantiated during \
                                 online"
                            )
                        }
                    }
                    storage.set(
                        entity,
                        Some(
                            initer.f.init(comp::any::DepGetter { inner: &PanicDepGetter, entity }),
                        ),
                    );
                }
            }

            Arc::new(RwLock::new(storage))
        })
    }

    pub(crate) fn get_mut(&mut self, discrim: C::Discrim) -> Option<&mut Arc<RwLock<C::Storage>>> {
        self.map.get_mut(&discrim)
    }

    pub(crate) fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (C::Discrim, &mut Arc<RwLock<C::Storage>>)> {
        self.map.iter_mut().map(|(discrim, storage)| (*discrim, storage))
    }

    pub(crate) fn len(&self) -> usize { self.map.len() }
}

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
        ealloc: &mut A::Ealloc,
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
        ealloc: &mut A::Ealloc,
    ) {
        let map = self.map.get_mut();
        let values = comp_map.remove_isotope::<C>();
        let value_count = values.len();

        for (discrim, value) in values {
            let storage = map.get_or_create(discrim, ealloc.snapshot().iter_allocated_chunks());
            let storage = Arc::get_mut(storage).expect("storage arc was leaked").get_mut();
            storage.set(entity, Some(value));
        }

        if let comp::InitStrategy::Auto(initer) = C::INIT_STRATEGY {
            for (_discrim, storage) in map.iter_mut() {
                let storage: &mut C::Storage =
                    Arc::get_mut(storage).expect("storage arc was leaked").get_mut();
                if storage.get(entity).is_none() {
                    storage.set(entity, Some(initer.f.init(dep_getter)));
                }
            }
        } else if let comp::Presence::Required = C::PRESENCE {
            if value_count != map.len() {
                panic!(
                    "Isotope type `{}`/`{}` cannot declare `Required` presence without an \
                     auto-initializer",
                    any::type_name::<A>(),
                    any::type_name::<C>(),
                );
            }
        }
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
