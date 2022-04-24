use std::collections::BTreeMap;
use std::mem::{self, MaybeUninit};
use std::sync::Arc;

use bitvec::prelude::BitVec;
use parking_lot::RwLock;

use crate::{component, entity, Archetype};

pub(crate) type Shared = Arc<RwLock<dyn AnyStorage>>;

pub(crate) fn shared_simple<A: Archetype, C: component::Simple<A>>() -> Shared {
    Arc::new(RwLock::new(Storage::<C>::new())) as Shared
}

pub(crate) trait AnyStorage {}

pub(crate) struct Storage<T> {
    inner: Inner<T>,
}

impl<T> Storage<T> {
    pub(crate) fn new() -> Self { Self { inner: Inner::default() } }
}

impl<T> AnyStorage for Storage<T> {}

enum Inner<T> {
    Map(BTreeMap<entity::Raw, T>),
    Vec(InnerVec<T>),
}

impl<T> Default for Inner<T> {
    fn default() -> Self { Inner::Map(BTreeMap::new()) }
}

impl<T> Inner<T> {
    pub(crate) fn get(&self, id: entity::Raw) -> Option<&T> {
        match self {
            Self::Map(map) => map.get(&id),
            Self::Vec(vec) => {
                match vec.presence.get(id.usize()) {
                    Some(presence) if *presence => {
                        let value = vec.data.get(id.usize())?;
                        // Safety: presence is true
                        let value = unsafe { value.assume_init_ref() };
                        Some(value)
                    }
                    _ => None,
                }
            }
        }
    }

    pub(crate) fn get_mut(&mut self, id: entity::Raw) -> Option<&mut T> {
        match self {
            Self::Map(map) => map.get_mut(&id),
            Self::Vec(vec) => {
                match vec.presence.get(id.usize()) {
                    Some(presence) if *presence => {
                        let value = vec.data.get_mut(id.usize())?;
                        // Safety: presence is true
                        let value = unsafe { value.assume_init_mut() };
                        Some(value)
                    }
                    _ => None,
                }
            }
        }
    }

    pub(crate) fn insert(&mut self, id: entity::Raw, value: T) -> Option<T> {
        match self {
            Self::Map(map) => map.insert(id, value),
            Self::Vec(vec) => {
                let id = id.usize();

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

    pub(crate) fn remove(&mut self, id: entity::Raw) -> Option<T> {
        match self {
            Self::Map(map) => map.remove(&id),
            Self::Vec(vec) => {
                let id = id.usize();

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

pub(crate) trait IsotopeFactory {}
