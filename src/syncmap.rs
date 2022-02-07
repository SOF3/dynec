use std::collections::BTreeMap;
use std::ops;
use std::sync::{Arc, RwLock};

pub(crate) struct SyncMap<K: Eq + Ord, V: ?Sized> {
    map: RwLock<BTreeMap<K, Arc<V>>>,
}

impl<K: Eq + Ord, V: ?Sized> Default for SyncMap<K, V> {
    fn default() -> Self {
        SyncMap {
            map: RwLock::new(BTreeMap::new()),
        }
    }
}

impl<K: Eq + Ord, V: ?Sized> From<BTreeMap<K, Arc<V>>> for SyncMap<K, V> {
    fn from(map: BTreeMap<K, Arc<V>>) -> Self {
        SyncMap {
            map: RwLock::new(map),
        }
    }
}

impl<K: Eq + Ord, V: ?Sized> SyncMap<K, V> {
    /// Gets a cloned arc of the value for the given key, or initialize it with the given function.
    pub fn get_or_init(&self, k: K, create: impl FnOnce() -> Arc<V>) -> Arc<V> {
        {
            let map = self.map.read().expect("another thread panicked");
            if let Some(v) = map.get(&k) {
                return Arc::clone(v);
            }
        }

        {
            let mut map = self.map.write().expect("another thread panicked");
            let v = map.entry(k).or_insert_with(create);
            Arc::clone(v)
        }
    }

    /// Gets a cloned arc of the value for the given key.
    pub fn get(&self, k: K) -> Option<Arc<V>> {
        let map = self.map.read().expect("another thread panicked");
        map.get(&k).cloned()
    }

    /// Returns the underlying map under unique access.
    pub fn map(&mut self) -> &mut BTreeMap<K, Arc<V>> {
        self.map.get_mut().expect("another thread panicked")
    }
}

/// A smart pointer that derives a reference from a value behind an [`Arc`].
pub(crate) struct ArcMap<T: ?Sized, U: ?Sized> {
    arc: Arc<T>,
    ref_fn: fn(&T) -> &U,
    ref_mut_fn: fn(&mut T) -> &mut U,
}

impl<T: ?Sized, U: ?Sized> ArcMap<T, U> {
    /// Creates a new `ArcMap` from the given `Arc` and projection function.
    pub fn new(arc: Arc<T>, ref_fn: fn(&T) -> &U, ref_mut_fn: fn(&mut T) -> &mut U) -> Self {
        ArcMap {
            arc,
            ref_fn,
            ref_mut_fn,
        }
    }
}

impl<T: ?Sized, U: ?Sized> ops::Deref for ArcMap<T, U> {
    type Target = U;

    fn deref(&self) -> &U {
        (self.ref_fn)(&*self.arc)
    }
}

impl<T: ?Sized, U: ?Sized> ops::DerefMut for ArcMap<T, U> {
    fn deref_mut(&mut self) -> &mut U {
        (self.ref_mut_fn)(Arc::get_mut(&mut self.arc).expect("Arc leaked"))
    }
}
