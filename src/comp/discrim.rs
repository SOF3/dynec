//! Discriminants distinguish different isotopes of the same component type.

use core::fmt;
use std::any;
use std::hash::Hash;
use std::iter::FromIterator;

/// A discriminant value that distinguishes different isotopes of the same component type.
///
/// For compact storage, the discriminant should have a one-to-one mapping to the `usize` type.
/// The `usize` needs not be a small number; it can be any valid `usize`
/// as long as it is one-to-one and consistent.
pub trait Discrim: fmt::Debug + Copy + PartialEq + Eq + Hash + Send + Sync + 'static {
    /// The optimized storage for indexing data of type `T` by discriminant.
    type FullMap<T>: FullMap<T>;

    /// Constructs a discriminant from the usize.
    ///
    /// Can panic if the usize is not supported.
    fn from_usize(usize: usize) -> Self;

    /// Converts the discriminant to a usize.
    fn into_usize(self) -> usize;
}

impl Discrim for usize {
    /// The default implementation uses linear search,
    /// which has reasonably small worst-case scenario for normal use.
    type FullMap<T> = LinearVecMap<T>;

    fn from_usize(usize: usize) -> Self { usize }

    fn into_usize(self) -> usize { self }
}

/// A map-like collection with discriminants as keys,
/// used to index storages when all discriminants are selected.
pub trait FullMap<T>: FromIterator<(usize, T)> + Extend<(usize, T)> {
    /// Returns an immutable reference to the value indexed by the discriminant.
    fn find(&self, discrim: usize) -> Option<&T>;

    /// Returns a mutable reference to the value indexed by the discriminant.
    fn find_mut(&mut self, discrim: usize) -> Option<&mut T>;

    /// Inserts an entry if it is missing. Returns a mutable reference to the entry.
    fn get_or_insert<F: FnOnce() -> T>(&mut self, discrim: usize, factory: F) -> &mut T;

    /// Return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = (usize, &'t T)>
    where
        Self: 't,
        T: 't;
    /// Returns an iterator over the map.
    ///
    /// The iterator yields all items along with their discriminant values.
    /// The iteration order is undefined.
    fn iter(&self) -> Self::Iter<'_>;

    /// Return value of [`iter_mut`](Self::iter_mut).
    type IterMut<'t>: Iterator<Item = (usize, &'t mut T)>
    where
        Self: 't,
        T: 't;
    /// Returns an iterator over the map.
    ///
    /// The iterator yields all items along with their discriminant values.
    /// The iteration order is undefined.
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
}

/// Implements [`Map`] with O(n) search complexity, where `n` is the number of storages.
/// Useful for discriminants with a small number of possible values
/// but have an unbounded range, e.g. arbitrarily large IDs.
pub struct LinearVecMap<T> {
    vec: Vec<(usize, T)>,
}

impl<T> FromIterator<(usize, T)> for LinearVecMap<T> {
    fn from_iter<I: IntoIterator<Item = (usize, T)>>(iter: I) -> Self {
        Self { vec: Vec::from_iter(iter) }
    }
}

impl<T> Extend<(usize, T)> for LinearVecMap<T> {
    fn extend<I: IntoIterator<Item = (usize, T)>>(&mut self, iter: I) { self.vec.extend(iter); }
}

impl<T> FullMap<T> for LinearVecMap<T> {
    fn find(&self, needle: usize) -> Option<&T> {
        self.vec.iter().find(|&&(discrim, _)| discrim == needle).map(|(_, item)| item)
    }
    fn find_mut(&mut self, needle: usize) -> Option<&mut T> {
        self.vec.iter_mut().find(|&&mut (discrim, _)| discrim == needle).map(|(_, item)| item)
    }

    fn get_or_insert<F: FnOnce() -> T>(&mut self, needle: usize, factory: F) -> &mut T {
        if let Some(position) = self.vec.iter().position(|&(discrim, _)| discrim == needle) {
            let (_, value) =
                self.vec.get_mut(position).expect("position returned by .iter().position()");
            return value;
        }

        self.vec.push((needle, factory()));
        let (_, item) = self.vec.last_mut().expect("vec is nonempty after push");
        item
    }

    type Iter<'t> = impl Iterator<Item = (usize, &'t T)> where Self: 't, T: 't;
    fn iter(&self) -> Self::Iter<'_> { self.vec.iter().map(|(discrim, value)| (*discrim, value)) }

    type IterMut<'t> = impl Iterator<Item = (usize, &'t mut T)> where Self: 't, T: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.vec.iter_mut().map(|(discrim, value)| (*discrim, value))
    }
}

/// Implements [`Map`] with O(1) search complexity and O(m) memory,
/// where `m` is the greatest discriminant value.
/// Useful for discriminants that are known to be IDs counting from 0
/// and hence have a reasonably bounded small value.
pub struct BoundedVecMap<T> {
    vec: Vec<Option<T>>,
}

impl<T> FromIterator<(usize, T)> for BoundedVecMap<T> {
    fn from_iter<I: IntoIterator<Item = (usize, T)>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut this = Self { vec: (0..iter.size_hint().0).map(|_| None).collect() };
        this.extend(iter);
        this
    }
}

impl<T> Extend<(usize, T)> for BoundedVecMap<T> {
    fn extend<I: IntoIterator<Item = (usize, T)>>(&mut self, iter: I) {
        for (discrim, value) in iter {
            if self.vec.len() <= discrim {
                self.vec.resize_with(discrim + 1, || None);
            }
            let entry = self.vec.get_mut(discrim).expect("just reserved");
            *entry = Some(value);
        }
    }
}

impl<T> FullMap<T> for BoundedVecMap<T> {
    fn find(&self, discrim: usize) -> Option<&T> { self.vec.get(discrim).and_then(Option::as_ref) }

    fn find_mut(&mut self, discrim: usize) -> Option<&mut T> {
        self.vec.get_mut(discrim).and_then(Option::as_mut)
    }

    fn get_or_insert<F: FnOnce() -> T>(&mut self, discrim: usize, factory: F) -> &mut T {
        if self.vec.len() <= discrim {
            self.vec.resize_with(discrim + 1, || None);
        }
        let entry = self.vec.get_mut(discrim).expect("just reserved");
        entry.get_or_insert_with(factory)
    }

    type Iter<'t> = impl Iterator<Item = (usize, &'t T)> where Self: 't, T: 't;
    fn iter(&self) -> Self::Iter<'_> {
        self.vec.iter().enumerate().filter_map(|(discrim, value)| Some((discrim, value.as_ref()?)))
    }

    type IterMut<'t> = impl Iterator<Item = (usize, &'t mut T)> where Self: 't, T: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.vec
            .iter_mut()
            .enumerate()
            .filter_map(|(discrim, value)| Some((discrim, value.as_mut()?)))
    }
}

/// A wrapper for [`usize`] that implements [`Discrim`] with [`BoundedVecMap`] instead.
#[repr(transparent)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, dynec_codegen::Discrim,
)]
#[dynec(dynec_as(crate), map = BoundedVecMap)]
pub struct BoundedUsize(
    /// The underlying value
    pub usize,
);

/// Implements [`Map`] with O(1) search complexity and O(N) memory,
/// where `N` is a number known at compile time.
/// Useful for discriminants that are have a statically bounded small value,
/// e.g. if they are derived from an enum.
pub struct ArrayMap<T, const N: usize> {
    array: [Option<T>; N],
}

impl<T, const N: usize> FromIterator<(usize, T)> for ArrayMap<T, N> {
    fn from_iter<I: IntoIterator<Item = (usize, T)>>(iter: I) -> Self {
        let mut this = Self { array: [(); N].map(|()| None) };
        this.extend(iter);
        this
    }
}

impl<T, const N: usize> Extend<(usize, T)> for ArrayMap<T, N> {
    fn extend<I: IntoIterator<Item = (usize, T)>>(&mut self, iter: I) {
        for (discrim, value) in iter {
            let entry = match self.array.get_mut(discrim) {
                Some(entry) => entry,
                None => panic!(
                    "{} is too small to contain all possible discriminants",
                    any::type_name::<Self>()
                ),
            };
            *entry = Some(value);
        }
    }
}

impl<T, const N: usize> FullMap<T> for ArrayMap<T, N> {
    fn find(&self, discrim: usize) -> Option<&T> {
        self.array.get(discrim).and_then(Option::as_ref)
    }

    fn find_mut(&mut self, discrim: usize) -> Option<&mut T> {
        self.array.get_mut(discrim).and_then(Option::as_mut)
    }

    fn get_or_insert<F: FnOnce() -> T>(&mut self, discrim: usize, factory: F) -> &mut T {
        let entry = match self.array.get_mut(discrim) {
            Some(entry) => entry,
            None => panic!(
                "{} is too small to contain all possible discriminants",
                any::type_name::<Self>()
            ),
        };
        entry.get_or_insert_with(factory)
    }

    type Iter<'t> = impl Iterator<Item = (usize, &'t T)> where Self: 't, T: 't;
    fn iter(&self) -> Self::Iter<'_> {
        self.array
            .iter()
            .enumerate()
            .filter_map(|(discrim, value)| Some((discrim, value.as_ref()?)))
    }

    type IterMut<'t> = impl Iterator<Item = (usize, &'t mut T)> where Self: 't, T: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.array
            .iter_mut()
            .enumerate()
            .filter_map(|(discrim, value)| Some((discrim, value.as_mut()?)))
    }
}

/// An ordered collection of distinct discriminants.
pub trait Set {
    type Key;
    type Value;

    /// Return value of [`Set::map`].
    type Map<U>: Set<Value = U>;

    /// Transforms each element in this set stored,
    /// stored in a collection with a similar structure.
    fn map<U, F>(&self, f: F) -> Self::Map<U>
    where
        Self::Value: Copy,
        F: FnMut(Self::Value) -> U;

    /// Gets a value from the set by index.
    /// Panics if the key is invalid.
    fn get_by(&self, key: Self::Key) -> &Self::Value;

    /// Gets a value mutably from the set by index.
    /// Panics if the key is invalid.
    fn get_by_mut(&mut self, key: Self::Key) -> &mut Self::Value;

    type Iter<'t>: Iterator<Item = &'t Self::Value> + 't
    where
        Self: 't,
        Self::Value: 't;
    fn iter(&self) -> Self::Iter<'_>;
}

impl<T, const N: usize> Set for [T; N] {
    type Key = usize;
    type Value = T;

    type Map<U> = [U; N];

    fn map<U, F>(&self, f: F) -> [U; N]
    where
        T: Copy,
        F: FnMut(T) -> U,
    {
        <[T; N]>::map(*self, f)
    }

    fn get_by(&self, key: Self::Key) -> &T { self.get(key).expect("key out of bounds") }

    fn get_by_mut(&mut self, key: Self::Key) -> &mut T {
        self.get_mut(key).expect("key out of bounds")
    }

    type Iter<'t> = <&'t [T] as IntoIterator>::IntoIter
        where T: 't;
    fn iter(&self) -> Self::Iter<'_> { self[..].iter() }
}

impl<T> Set for Vec<T> {
    type Key = usize;
    type Value = T;

    type Map<U> = Vec<U>;

    fn map<U, F>(&self, f: F) -> Vec<U>
    where
        T: Copy,
        F: FnMut(T) -> U,
    {
        self.iter().copied().map(f).collect()
    }

    fn get_by(&self, key: Self::Key) -> &T { self.get(key).expect("key out of bounds") }

    fn get_by_mut(&mut self, key: Self::Key) -> &mut T {
        self.get_mut(key).expect("key out of bounds")
    }

    type Iter<'t> = <&'t [T] as IntoIterator>::IntoIter
        where T: 't;
    fn iter(&self) -> Self::Iter<'_> { self[..].iter() }
}
