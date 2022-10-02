//! Discriminants distinguish different isotopes of the same component type.

use std::fmt;
use std::hash::Hash;

/// A discriminant value that distinguishes different isotopes of the same component type.
///
/// A discriminant should have a one-to-one mapping to `usize`,
/// which is used to represent the discriminant in type-erased code (such as scheduling).
/// Furthermore, if [`FullSet`](Self::FullSet) is
/// [`LinearVecMap`], [`SortedVecMap`] or [`BoundedVecMap`],
/// this `usize` is used for indexing storages during all-isotopes read/write access.
/// The range of mapped `usize`s should be bounded to a small number if [`BoundedVecMap`] is used.
pub trait Discrim: fmt::Debug + Copy + PartialEq + Eq + Hash + Send + Sync + 'static {
    /// The data structure to index objects by all known discriminants.
    ///
    /// This is only used when storages of all isotopes are read/written in the same accessor
    /// (through [`Components::read_full_isotope_storage`][read_full_isotope_storage],
    /// or `system::ReadIsotope` without `#[dynec(isotope(discrim = xxx))]`).
    ///
    /// [read_full_isotope_storage]: dynec::world::Components::read_full_isotope_storage
    type FullSet<S>: Mapped<Discrim = Self, Key = Self, Value = S>
        + FromIterator<(Self, S)>
        + Extend<(Self, S)>;

    // TODO: can we remove usize conversion?
    // Currently it is used in scheduler for type-agnostic collision checking.

    /// Constructs a discriminant from the usize.
    ///
    /// The returned value must be consistent and inverse of [`into_usize`](Self::into_usize).
    ///
    /// Can panic if the usize is not supported.
    fn from_usize(usize: usize) -> Self;

    /// Converts the discriminant to a usize.
    ///
    /// The returned value must be consistent and inverse of [`from_usize`](Self::from_usize).
    /// `discrim1 == discrim2` if and only if `discrim1.into_usize() == discrim2.into_usize()`.
    fn into_usize(self) -> usize;
}

/// A set of discriminants, used for specifying partial access in
/// [`#[system]`](macro@crate::system).
pub trait Set<D: Discrim> {
    /// Return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = D>
    where
        Self: 't;
    /// Iterates over the discriminants in this set.
    fn iter(&self) -> Self::Iter<'_>;

    /// The key used in mapping types.
    type Key;
    /// Return value of [`map`](Self::iter).
    type Mapped<U>: Mapped<Discrim = D, Key = Self::Key, Value = U>;
    /// Transforms each discriminant to another value.
    fn map<U, F: FnMut(D) -> U>(&self, func: F) -> Self::Mapped<U>;
}

impl<D: Discrim, const N: usize> Set<D> for [D; N] {
    type Iter<'t> = impl Iterator<Item = D> where Self: 't;
    fn iter(&self) -> Self::Iter<'_> { (*self).into_iter() }

    type Key = usize;
    type Mapped<U> = [(D, U); N];
    fn map<U, F: FnMut(D) -> U>(&self, mut func: F) -> Self::Mapped<U> {
        <[D; N]>::map(*self, |discrim| (discrim, func(discrim)))
    }
}

impl<D: Discrim> Set<D> for Vec<D> {
    type Iter<'t> = impl Iterator<Item = D> where Self: 't;
    fn iter(&self) -> Self::Iter<'_> { self[..].iter().copied() }

    type Key = usize;
    type Mapped<U> = Vec<(D, U)>;
    fn map<U, F: FnMut(D) -> U>(&self, mut func: F) -> Self::Mapped<U> {
        self[..].iter().map(|&discrim| (discrim, func(discrim))).collect()
    }
}

/// A data structure derived from a [discriminant set](Set)
/// that can efficiently access an item using [`Key`](Self::Key),
/// which is based on the shape of the set.
///
/// This type is also used for the collection of isotope storages
/// when all discriminants are selected.
pub trait Mapped {
    /// The discriminant type.
    type Discrim: Discrim;
    /// The type used for indexing data.
    type Key: fmt::Debug;
    type Value;

    /// Gets a shared reference to an element.
    fn get_by(&self, key: &Self::Key) -> Option<&Self::Value>;

    /// Executes functions with mutable reference to an entry.
    fn get_mut_by(&mut self, key: &Self::Key) -> Option<&mut Self::Value>;

    /// return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = (Self::Discrim, &'t Self::Value)> + 't
    where
        Self: 't;
    /// Iterates over the values in this set with the discriminant.
    fn iter(&self) -> Self::Iter<'_>;

    /// return value of [`iter`](Self::iter).
    type IterMut<'t>: Iterator<Item = (Self::Discrim, &'t mut Self::Value)> + 't
    where
        Self: 't;
    /// Iterates over the values in this set with the discriminant.
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
}

impl<D: Discrim, V, const N: usize> Mapped for [(D, V); N] {
    type Discrim = D;
    type Key = usize;
    type Value = V;

    fn get_by(&self, &key: &usize) -> Option<&V> {
        let (_, value) = self.get(key)?;
        Some(value)
    }

    fn get_mut_by(&mut self, &key: &usize) -> Option<&mut V> {
        let (_, value) = self.get_mut(key)?;
        Some(value)
    }

    type Iter<'t> = impl Iterator<Item = (D, &'t V)> + 't where Self: 't;
    fn iter(&self) -> Self::Iter<'_> { self[..].iter().map(|(discrim, value)| (*discrim, value)) }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut V)> + 't where Self: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self[..].iter_mut().map(|(discrim, value)| (*discrim, value))
    }
}

impl<D: Discrim, V> Mapped for Vec<(D, V)> {
    type Discrim = D;
    type Key = usize;
    type Value = V;

    fn get_by(&self, &key: &usize) -> Option<&V> {
        let (_, value) = self.get(key)?;
        Some(value)
    }

    fn get_mut_by(&mut self, &key: &usize) -> Option<&mut V> {
        let (_, value) = self.get_mut(key)?;
        Some(value)
    }

    type Iter<'t> = impl Iterator<Item = (D, &'t V)> + 't where Self: 't;
    fn iter(&self) -> Self::Iter<'_> { self[..].iter().map(|(discrim, value)| (*discrim, value)) }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut V)> + 't where Self: 't;
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self[..].iter_mut().map(|(discrim, value)| (*discrim, value))
    }
}

pub struct LinearVecMap<D: Discrim, T> {
    vec: Vec<(D, T)>,
}

pub struct SortedVecMap<D: Discrim, T> {
    vec: Vec<(D, T)>,
}

pub struct BoundedVecMap<D: Discrim, T> {
    vec: Vec<T>,
}
