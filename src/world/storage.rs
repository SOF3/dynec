use std::any::{self, Any};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};
use std::sync::Arc;

use bitvec::prelude::BitVec;
use parking_lot::RwLock;

use crate::{comp, entity, Archetype};

pub(crate) type SharedSimple<A> = Arc<RwLock<dyn AnySimpleStorage<A> + Send + Sync>>;

pub(crate) fn shared_simple<A: Archetype, C: comp::Simple<A>>() -> SharedSimple<A> {
    Arc::new(RwLock::new(Storage::<A, C>::new_simple()))
}

pub(crate) trait AnySimpleStorage<A: Archetype> {
    fn init_strategy(&self) -> comp::SimpleInitStrategy<A>;

    fn init_with(&mut self, entity: A::RawEntity, components: &mut comp::Map<A>);

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(crate) struct Storage<A: Archetype, C: 'static> {
    inner:       Inner<A::RawEntity, C>,
    lazy_initer: LazyIniter<C>,
    _ph:         PhantomData<A>,
}

impl<A: Archetype, C: comp::Simple<A>> Storage<A, C> {
    pub(crate) fn new_simple() -> Self {
        Self {
            inner:       Inner::default(),
            lazy_initer: LazyIniter::Simple,
            _ph:         PhantomData,
        }
    }

    pub(crate) fn get(&self, id: A::RawEntity) -> Option<&C> { self.inner.get(id) }

    pub(crate) fn get_mut(&mut self, id: A::RawEntity) -> Option<&mut C> { self.inner.get_mut(id) }

    pub(crate) fn set(&mut self, id: A::RawEntity, value: Option<C>) -> Option<C> {
        match value {
            Some(value) => self.inner.insert(id, value),
            None => self.inner.remove(id),
        }
    }
}

impl<A: Archetype, C: comp::Simple<A>> AnySimpleStorage<A> for Storage<A, C> {
    fn init_strategy(&self) -> comp::SimpleInitStrategy<A> { C::INIT_STRATEGY }

    fn init_with(&mut self, entity: A::RawEntity, components: &mut comp::Map<A>) {
        if let Some(comp) = components.remove_simple::<C>() {
            self.inner.insert(entity, comp);
        } else if let comp::SimplePresence::Required = C::PRESENCE {
            panic!(
                "Cannot create an entity of type `{}` without explicitly passing a component of \
                 type `{}`",
                any::type_name::<A>(),
                any::type_name::<C>(),
            );
        }
    }

    fn as_any(&self) -> &dyn Any { self }

    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

enum Inner<R: entity::Raw, T> {
    Map(BTreeMap<R, T>),
    Vec(InnerVec<T>),
}

impl<R: entity::Raw, T> Default for Inner<R, T> {
    fn default() -> Self { Inner::Map(BTreeMap::new()) }
}

impl<R: entity::Raw, T> Inner<R, T> {
    pub(crate) fn get(&self, id: R) -> Option<&T> {
        match self {
            Self::Map(map) => map.get(&id),
            Self::Vec(vec) => {
                match vec.presence.get(id.to_primitive()) {
                    Some(presence) if *presence => {
                        let value = vec.data.get(id.to_primitive())?;
                        // Safety: presence is true
                        let value = unsafe { value.assume_init_ref() };
                        Some(value)
                    }
                    _ => None,
                }
            }
        }
    }

    pub(crate) fn get_mut(&mut self, id: R) -> Option<&mut T> {
        match self {
            Self::Map(map) => map.get_mut(&id),
            Self::Vec(vec) => {
                match vec.presence.get(id.to_primitive()) {
                    Some(presence) if *presence => {
                        let value = vec.data.get_mut(id.to_primitive())?;
                        // Safety: presence is true
                        let value = unsafe { value.assume_init_mut() };
                        Some(value)
                    }
                    _ => None,
                }
            }
        }
    }

    pub(crate) fn insert(&mut self, id: R, value: T) -> Option<T> {
        match self {
            Self::Map(map) => map.insert(id, value),
            Self::Vec(vec) => {
                let id = id.to_primitive();

                let required_len = id + 1;
                if vec.presence.len() < required_len {
                    vec.presence.reserve(required_len);
                    vec.data.reserve(required_len);

                    vec.presence.resize(required_len, false);
                    // Safety:
                    // 1. capacity is reserved above
                    // 2. value type is MaybeUninit and does not need initialization
                    // 3. presence is false
                    unsafe { vec.data.set_len(required_len) }
                }

                let mut presence = vec.presence.get_mut(id).expect("Resized above");
                let data = vec.data.get_mut(id).expect("Length set above");
                if *presence {
                    // Safety: presence is true
                    let data = unsafe { data.assume_init_mut() };

                    let original = mem::replace(data, value);
                    Some(original)
                } else {
                    *data = MaybeUninit::new(value);
                    *presence = true;
                    None
                }
            }
        }
    }

    pub(crate) fn remove(&mut self, id: R) -> Option<T> {
        match self {
            Self::Map(map) => map.remove(&id),
            Self::Vec(vec) => {
                let id = id.to_primitive();

                match vec.presence.get_mut(id) {
                    Some(mut presence) if *presence => {
                        let data = vec.data.get_mut(id).expect("presence is true");

                        // TODO: change to assume_init_read when it is stable
                        let value = mem::replace(data, MaybeUninit::uninit());
                        // Safety: presence is true
                        let value = unsafe { value.assume_init() };

                        *presence = false;
                        Some(value)
                    }
                    _ => None,
                }
            }
        }
    }
}

struct InnerVec<T> {
    presence: BitVec,
    data:     Vec<MaybeUninit<T>>,
}

enum LazyIniter<C: 'static> {
    // Simple components are not lazy-initialized.
    Simple,
    Isotope { c: C },
}

pub(crate) trait AnyIsotopeFactory<A: Archetype>: Send + Sync {}

struct IsotopeFactory<A: Archetype, C: comp::Isotope<A>> {
    _ph: PhantomData<(A, C)>,
}

impl<A: Archetype, C: comp::Isotope<A>> AnyIsotopeFactory<A> for IsotopeFactory<A, C> {}

pub(crate) fn isotope_factory<A: Archetype, C: comp::Isotope<A>>() -> Box<dyn AnyIsotopeFactory<A>>
{
    Box::new(IsotopeFactory::<A, C> { _ph: PhantomData }) as Box<dyn AnyIsotopeFactory<A>>
}
