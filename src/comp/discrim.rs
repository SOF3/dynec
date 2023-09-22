//! Discriminants distinguish different isotopes of the same component type.

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

/// A discriminant value that distinguishes different isotopes of the same component type.
///
/// A discriminant should have a one-to-one mapping to `usize`,
/// which is used to represent the discriminant in type-erased code (such as scheduling).
/// Furthermore, if [`FullMap`](Self::FullMap) is
/// [`LinearVecMap`], [`SortedVecMap`] or [`BoundedVecMap`],
/// this `usize` is used for indexing storages during all-isotopes read/write access.
/// The range of mapped `usize`s should be bounded to a small number if [`BoundedVecMap`] is used.
pub trait Discrim: fmt::Debug + Copy + PartialEq + Eq + Hash + Send + Sync + 'static {
    /// The data structure that can efficiently access an item
    /// based on discriminants from a [`Set`].
    type FullMap<S>: FullMap<Discrim = Self, Key = Self, Value = S>;

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
pub trait Set<D: Discrim>: Send + Sync + 'static {
    /// Return value of [`iter_discrims`](Self::iter_discrims).
    type Iter<'t>: Iterator<Item = D>
    where
        Self: 't;
    /// Iterates over the discriminants in this set.
    fn iter_discrims(&self) -> Self::Iter<'_>;

    /// The key used in mapping types.
    type Key: fmt::Debug + Copy + 'static;
    /// Return value of [`map`](Self::map).
    type Mapped<U>: Mapped<Discrim = D, Key = Self::Key, Value = U>;
    /// Transforms each discriminant to another value.
    fn map<U, F: FnMut(D) -> U>(&self, func: F) -> Self::Mapped<U>;
}

impl<D: Discrim, const N: usize> Set<D> for [D; N] {
    type Iter<'t> = impl Iterator<Item = D> where Self: 't;
    fn iter_discrims(&self) -> Self::Iter<'_> { (*self).into_iter() }

    type Key = usize;
    type Mapped<U> = [(D, U); N];
    fn map<U, F: FnMut(D) -> U>(&self, mut func: F) -> Self::Mapped<U> {
        <[D; N]>::map(*self, |discrim| (discrim, func(discrim)))
    }
}

impl<D: Discrim> Set<D> for Vec<D> {
    type Iter<'t> = impl Iterator<Item = D> + 't where Self: 't;
    fn iter_discrims(&self) -> Self::Iter<'_> { self[..].iter().copied() }

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
    type Key: fmt::Debug + Copy + 'static;
    /// The value type stored in this data structure.
    type Value;

    /// Gets the discriminant value associated with this key.
    fn get_discrim(&self, key: Self::Key) -> Option<Self::Discrim>;

    /// Gets a shared reference to an element.
    fn get_by(&self, key: Self::Key) -> Option<&Self::Value>;

    /// Gets a mutable reference to an element.
    fn get_mut_by(&mut self, key: Self::Key) -> Option<&mut Self::Value>;

    /// Gets mutable references to `N` different elements,
    /// transforms them and stores them in an array.
    ///
    /// # Panics
    /// This method panics if `keys` contains duplicate discriminants,
    /// or if any keys do not exist in the map.
    /// For the latter case, `on_missing` is invoked, which should panic.
    fn get_mut_array_by<'t, U, TransformFn, OnMissing, const N: usize>(
        &'t mut self,
        keys: [Self::Key; N],
        transform: TransformFn,
        on_missing: OnMissing,
    ) -> [U; N]
    where
        U: RefToPtr,
        TransformFn: FnMut(&'t mut Self::Value) -> U,
        OnMissing: FnMut(Self::Key) -> !;

    /// Return value of [`iter_mapped`](Self::iter_mapped).
    type Iter<'t>: Iterator<Item = (Self::Key, Self::Discrim, &'t Self::Value)> + 't
    where
        Self: 't;
    /// Iterates over the values in this map with the discriminant.
    fn iter_mapped(&self) -> Self::Iter<'_>;

    /// Return value of [`iter_mapped_mut`](Self::iter_mapped_mut).
    type IterMut<'t>: Iterator<Item = (Self::Discrim, &'t mut Self::Value)> + 't
    where
        Self: 't;
    /// Iterates over the values in this map with the discriminant.
    fn iter_mapped_mut(&mut self) -> Self::IterMut<'_>;

    /// Return value of [`into_iter_mapped`](Self::into_iter_mapped).
    type IntoIter: Iterator<Item = (Self::Discrim, Self::Value)>;
    /// Iterates over the values in this map with the discriminant.
    fn into_iter_mapped(self) -> Self::IntoIter;
}

/// A data structure to index objects by all known discriminants.
///
/// This is only used when storages of all isotopes are read/written in the same accessor
/// (through [`Components::read_full_isotope_storage`][read_full_isotope_storage],
/// or `impl system::ReadIsotope` without `#[dynec(isotope(discrim = xxx))]`).
///
/// [read_full_isotope_storage]: crate::world::Components::read_full_isotope_storage
pub trait FullMap
where
    Self: Mapped<Key = <Self as Mapped>::Discrim> + FromIterator<(Self::Discrim, Self::Value)>,
{
    /// Return value of [`map_ref`](Self::map_ref).
    type MapRef<U>: Mapped<Discrim = Self::Discrim, Key = Self::Key, Value = U>;
    /// Maps all values in this map, taking self by shared reference,
    /// preserving a similar data structure.
    fn map_ref<U, F>(&self, f: F) -> Self::MapRef<U>
    where
        F: FnMut(&Self::Value) -> U;

    /// Lazily initializes the entry and returns a mutable reference
    fn get_by_or_insert<F: FnOnce() -> Self::Value>(
        &mut self,
        discrim: Self::Discrim,
        inserter: F,
    ) -> &mut Self::Value;

    /// Lazily initializes the entry and returns a mutable reference
    fn get_by_or_insert_array<'t, T, Inserter, Transform, const N: usize>(
        &'t mut self,
        discrims: [Self::Discrim; N],
        inserter: Inserter,
        transform: Transform,
    ) -> [T; N]
    where
        T: RefToPtr,
        Inserter: FnMut(Self::Discrim) -> Self::Value,
        Transform: FnMut(Self::Discrim, &'t mut Self::Value) -> T;
}

/// Splits an object to multiple mutable references and returns them in an array.
///
/// # Safety
/// `getter` must be an injection, i.e. for `a, b : usize`,
/// `getter(a)` aliases `getter(b)` if and only if `a == b`.
///
/// # Panics
/// Panics if `keys` is not disjoint.
unsafe fn split_mut_array<'t, C, Getter, T: 't, U, TransformFn, OnMissingFn, const N: usize>(
    getter_context: &'t mut C,
    mut getter: Getter,
    keys: [usize; N],
    mut transform: TransformFn,
    mut on_missing: OnMissingFn,
) -> [U; N]
where
    Getter: FnMut(&mut C, usize) -> Option<&mut T>,
    TransformFn: FnMut(usize, &'t mut T) -> U,
    OnMissingFn: FnMut(usize) -> !,
    U: RefToPtr,
{
    let mut array: [MaybeUninit<U>; N] = MaybeUninit::uninit_array();
    for (i, key) in keys.into_iter().enumerate() {
        assert!(
            keys[..i].iter().all(|&item| item != key),
            "Cannot split isotopes with duplicate key {key:?}"
        );

        // Safety: we assume that `getter` is injective, so `getter_context` can be aliased.
        let value = match getter(&mut *(getter_context as *mut C), key) {
            Some(value) => transform(i, value),
            None => on_missing(i),
        };

        assert!(
            array[..i].iter().all(|prev| {
                // Safety: previous item has been initialized
                let prev_ptr_ptr: &U = unsafe { prev.assume_init_ref() };
                let prev_ptr = prev_ptr_ptr.as_ptr();
                !std::ptr::eq(prev_ptr, value.as_ptr())
            }),
            "Cannot split isotopes with duplicate key {key:?}"
        );

        array[i] = MaybeUninit::new(value);
    }

    // Safety:
    // 1. All keys are checked to be unique from previous keys before calling `get_mut`.
    // 2. All keys in `0..M` have been filled to `array` before `array_assume_init` is
    //    called.

    MaybeUninit::array_assume_init(array)
}

mod sealed {
    pub trait Sealed {}
}
/// A shared reference or a mutable reference.
pub trait RefToPtr: sealed::Sealed {
    /// The referenced type.
    type Target: ?Sized;
    /// Converts the reference to a pointer.
    fn as_ptr(&self) -> *const Self::Target;
}
impl<'t, T: ?Sized> sealed::Sealed for &'t T {}
impl<'t, T: ?Sized> RefToPtr for &'t T {
    type Target = T;
    fn as_ptr(&self) -> *const T { *self as *const T }
}
impl<'t, T: ?Sized> sealed::Sealed for &'t mut T {}
impl<'t, T: ?Sized> RefToPtr for &'t mut T {
    type Target = T;
    fn as_ptr(&self) -> *const T { *self as *const T }
}

impl<D: Discrim, V, const N: usize> Mapped for [(D, V); N] {
    type Discrim = D;
    type Key = usize;
    type Value = V;

    fn get_discrim(&self, key: usize) -> Option<Self::Discrim> { self.get(key).map(|&(d, _)| d) }

    fn get_by(&self, key: usize) -> Option<&V> {
        let (_, value) = self.get(key)?;
        Some(value)
    }

    fn get_mut_by(&mut self, key: usize) -> Option<&mut V> {
        let (_, value) = self.get_mut(key)?;
        Some(value)
    }

    fn get_mut_array_by<'t, U, TransformFn, OnMissing, const M: usize>(
        &'t mut self,
        keys: [usize; M],
        mut transform: TransformFn,
        mut on_missing: OnMissing,
    ) -> [U; M]
    where
        U: RefToPtr,
        TransformFn: FnMut(&'t mut Self::Value) -> U,
        OnMissing: FnMut(Self::Key) -> !,
    {
        // Safety: `slice.get_mut` is an injection.
        unsafe {
            split_mut_array(
                self,
                |slice, index| slice.get_mut(index),
                keys,
                |_i, (_d, v)| transform(v),
                |i| on_missing(keys[i]),
            )
        }
    }

    type Iter<'t> = impl Iterator<Item = (usize, D, &'t V)> + 't where Self: 't;
    fn iter_mapped(&self) -> Self::Iter<'_> {
        self[..].iter().enumerate().map(|(key, (discrim, value))| (key, *discrim, value))
    }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut V)> + 't where Self: 't;
    fn iter_mapped_mut(&mut self) -> Self::IterMut<'_> {
        self[..].iter_mut().map(|(discrim, value)| (*discrim, value))
    }

    type IntoIter = impl Iterator<Item = (D, V)>;
    fn into_iter_mapped(self) -> Self::IntoIter { self.into_iter() }
}

impl<D: Discrim, V> Mapped for Vec<(D, V)> {
    type Discrim = D;
    type Key = usize;
    type Value = V;

    fn get_discrim(&self, key: usize) -> Option<Self::Discrim> { self.get(key).map(|&(d, _)| d) }

    fn get_by(&self, key: usize) -> Option<&V> {
        let (_, value) = self.get(key)?;
        Some(value)
    }

    fn get_mut_by(&mut self, key: usize) -> Option<&mut V> {
        let (_, value) = self.get_mut(key)?;
        Some(value)
    }

    fn get_mut_array_by<'t, U, TransformFn, OnMissing, const M: usize>(
        &'t mut self,
        keys: [usize; M],
        mut transform: TransformFn,
        mut on_missing: OnMissing,
    ) -> [U; M]
    where
        U: RefToPtr,
        TransformFn: FnMut(&'t mut Self::Value) -> U,
        OnMissing: FnMut(Self::Key) -> !,
    {
        // Safety: `slice.get_mut` is an injection.
        unsafe {
            split_mut_array(
                self,
                |slice, index| slice.get_mut(index),
                keys,
                |_i, (_d, v)| transform(v),
                |i| on_missing(keys[i]),
            )
        }
    }

    type Iter<'t> = impl Iterator<Item = (usize, D, &'t V)> + 't where Self: 't;
    fn iter_mapped(&self) -> Self::Iter<'_> {
        self[..].iter().enumerate().map(|(key, (discrim, value))| (key, *discrim, value))
    }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut V)> + 't where Self: 't;
    fn iter_mapped_mut(&mut self) -> Self::IterMut<'_> {
        self[..].iter_mut().map(|(discrim, value)| (*discrim, value))
    }

    type IntoIter = impl Iterator<Item = (D, V)>;
    fn into_iter_mapped(self) -> Self::IntoIter { self.into_iter() }
}

/// Implements the requirements of [`Discrim::FullMap`] with O(n) search complexity.
///
/// Optimized for discriminant types with unbounded domain but small cardinality.
pub struct LinearVecMap<D: Discrim, T> {
    vec: Vec<(D, T)>,
}
impl<D: Discrim, T> FromIterator<(D, T)> for LinearVecMap<D, T> {
    fn from_iter<I: IntoIterator<Item = (D, T)>>(iter: I) -> Self {
        Self { vec: iter.into_iter().collect() }
    }
}
impl<D: Discrim, T> Mapped for LinearVecMap<D, T> {
    type Discrim = D;
    type Key = D;
    type Value = T;

    fn get_discrim(&self, key: D) -> Option<Self::Discrim> { Some(key) }

    fn get_by(&self, key: D) -> Option<&T> {
        self.vec[..].iter().find(|&&(d, _)| d == key).map(|(_, s)| s)
    }

    fn get_mut_by(&mut self, key: D) -> Option<&mut T> {
        self.vec[..].iter_mut().find(|&&mut (d, _)| d == key).map(|(_, s)| s)
    }

    fn get_mut_array_by<'t, U, TransformFn, OnMissing, const M: usize>(
        &'t mut self,
        keys: [D; M],
        mut transform: TransformFn,
        mut on_missing: OnMissing,
    ) -> [U; M]
    where
        U: RefToPtr,
        TransformFn: FnMut(&'t mut Self::Value) -> U,
        OnMissing: FnMut(Self::Key) -> !,
    {
        // We first collect the matching indices to an array.
        // This operation is safe because duplicate indices would not immediately lead to UB
        // as long as no `&mut` is created.
        let indices = keys.map(|key| match self.vec[..].iter_mut().position(|(d, _)| *d == key) {
            Some(position) => position,
            None => on_missing(key),
        });

        // Safety: `self.vec.get_mut` is injective.
        // The implementation of `split_mut_array` will check for duplicates in `indices`,
        // so we don't need to check it here.
        unsafe {
            split_mut_array(
                &mut self.vec,
                |slice, index| slice.get_mut(index),
                indices,
                |_i, (_d, t)| transform(t),
                |_| unreachable!("result from iter_mut().position()"),
            )
        }
    }

    type Iter<'t> = impl Iterator<Item = (D, D, &'t T)> + 't where Self: 't;
    fn iter_mapped(&self) -> Self::Iter<'_> {
        self.vec[..].iter().map(|&(discrim, ref value)| (discrim, discrim, value))
    }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut T)> + 't where Self: 't;
    fn iter_mapped_mut(&mut self) -> Self::IterMut<'_> {
        self.vec[..].iter_mut().map(|(discrim, value)| (*discrim, value))
    }

    type IntoIter = impl Iterator<Item = (D, T)>;
    fn into_iter_mapped(self) -> Self::IntoIter { self.vec.into_iter() }
}
impl<D: Discrim, T> FullMap for LinearVecMap<D, T> {
    type MapRef<U> = LinearVecMap<D, U>;
    fn map_ref<U, F>(&self, mut f: F) -> Self::MapRef<U>
    where
        F: FnMut(&Self::Value) -> U,
    {
        LinearVecMap { vec: self.vec.iter().map(|&(d, ref t)| (d, f(t))).collect() }
    }

    fn get_by_or_insert<F: FnOnce() -> T>(
        &mut self,
        discrim: Self::Discrim,
        inserter: F,
    ) -> &mut T {
        // cannot use iter_mut() here due to borrowck bug
        for (i, &(d, _)) in self.vec.iter().enumerate() {
            if d == discrim {
                return &mut self.vec.get_mut(i).expect("i comes from iterator").1;
            }
        }

        self.vec.push((discrim, inserter()));
        &mut self.vec.last_mut().expect("vec should be nonempty after push").1
    }

    fn get_by_or_insert_array<'t, U, Inserter, Transform, const N: usize>(
        &'t mut self,
        discrims: [Self::Discrim; N],
        mut inserter: Inserter,
        mut transform: Transform,
    ) -> [U; N]
    where
        U: RefToPtr,
        Inserter: FnMut(Self::Discrim) -> Self::Value,
        Transform: FnMut(Self::Discrim, &'t mut Self::Value) -> U,
    {
        let indices =
            discrims.map(|discrim| match self.vec.iter().position(|&(d, _)| d == discrim) {
                Some(index) => index,
                None => {
                    self.vec.push((discrim, inserter(discrim)));
                    self.vec.len() - 1
                }
            });

        // Safety: self.vec.get_mut() is an injective function.
        unsafe {
            split_mut_array(
                &mut self.vec,
                |slice, index| slice.get_mut(index),
                indices,
                |_i, (d, t)| transform(*d, t),
                |_| unreachable!("returned by position() or below len()"),
            )
        }
    }
}

/// Implements the requirements of [`Discrim::FullMap`] with O(log n) search complexity.
///
/// Optimized for discriminant types with unbounded domain and large cardinality.
///
/// This type has O(n) insertion complexity,
/// but insertions are only expected to happen once per new discriminant value
/// in each lifecycle of the world.
#[derive(Clone)]
pub struct SortedVecMap<D: Discrim, T> {
    vec: Vec<(D, T)>,
}
impl<D: Discrim, T> FromIterator<(D, T)> for SortedVecMap<D, T> {
    fn from_iter<I: IntoIterator<Item = (D, T)>>(iter: I) -> Self {
        let mut vec: Vec<_> = iter.into_iter().collect();
        vec.sort_by_key(|(d, _)| d.into_usize());
        Self { vec }
    }
}
impl<D: Discrim, T> Mapped for SortedVecMap<D, T> {
    type Discrim = D;
    type Key = D;
    type Value = T;

    fn get_discrim(&self, key: D) -> Option<Self::Discrim> { Some(key) }

    fn get_by(&self, key: D) -> Option<&T> {
        match self.vec[..].binary_search_by_key(&key.into_usize(), |(di, _)| di.into_usize()) {
            Ok(index) => Some(&self.vec.get(index).expect("result of binary_search_by_key").1),
            Err(_) => None,
        }
    }

    fn get_mut_by(&mut self, key: D) -> Option<&mut T> {
        match self.vec[..].binary_search_by_key(&key.into_usize(), |(di, _)| di.into_usize()) {
            Ok(index) => {
                Some(&mut self.vec.get_mut(index).expect("result of binary_search_by_key").1)
            }
            Err(_) => None,
        }
    }

    fn get_mut_array_by<'t, U, TransformFn, OnMissing, const M: usize>(
        &'t mut self,
        keys: [D; M],
        mut transform: TransformFn,
        mut on_missing: OnMissing,
    ) -> [U; M]
    where
        U: RefToPtr,
        TransformFn: FnMut(&'t mut Self::Value) -> U,
        OnMissing: FnMut(Self::Key) -> !,
    {
        // We first collect the matching indices to an array.
        // This operation is safe because duplicate indices would not immediately lead to UB
        // as long as no `&mut` is created.
        let indices = keys.map(|key| {
            match self.vec[..].binary_search_by_key(&key.into_usize(), |(d, _)| d.into_usize()) {
                Ok(position) => position,
                Err(_) => on_missing(key),
            }
        });

        // Safety: `self.vec.get_mut` is injective.
        // The implementation of `split_mut_array` will check for duplicates in `indices`,
        // so we don't need to check it here.
        unsafe {
            split_mut_array(
                &mut self.vec,
                |slice, index| slice.get_mut(index),
                indices,
                |_i, (_d, t)| transform(t),
                |_| unreachable!("result from binary_search_by_key"),
            )
        }
    }

    type Iter<'t> = impl Iterator<Item = (D, D, &'t T)> + 't where Self: 't;
    fn iter_mapped(&self) -> Self::Iter<'_> {
        self.vec[..].iter().map(|&(discrim, ref value)| (discrim, discrim, value))
    }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut T)> + 't where Self: 't;
    fn iter_mapped_mut(&mut self) -> Self::IterMut<'_> {
        self.vec[..].iter_mut().map(|(discrim, value)| (*discrim, value))
    }

    type IntoIter = impl Iterator<Item = (D, T)>;
    fn into_iter_mapped(self) -> Self::IntoIter { self.vec.into_iter() }
}
impl<D: Discrim, T> FullMap for SortedVecMap<D, T> {
    type MapRef<U> = SortedVecMap<D, U>;
    fn map_ref<U, F>(&self, mut f: F) -> Self::MapRef<U>
    where
        F: FnMut(&Self::Value) -> U,
    {
        SortedVecMap { vec: self.vec.iter().map(|&(d, ref t)| (d, f(t))).collect() }
    }

    fn get_by_or_insert<F: FnOnce() -> T>(
        &mut self,
        discrim: Self::Discrim,
        inserter: F,
    ) -> &mut T {
        match self.vec[..].binary_search_by_key(&discrim.into_usize(), |(d, _)| d.into_usize()) {
            Ok(index) => &mut self.vec.get_mut(index).expect("result of binary_search_by_key").1,
            Err(index) => {
                self.vec.insert(index, (discrim, inserter()));
                &mut self.vec.get_mut(index).expect("vec.insert(index) called above").1
            }
        }
    }

    fn get_by_or_insert_array<'t, U, Inserter, Transform, const N: usize>(
        &'t mut self,
        discrims: [Self::Discrim; N],
        mut inserter: Inserter,
        mut transform: Transform,
    ) -> [U; N]
    where
        U: RefToPtr,
        Inserter: FnMut(Self::Discrim) -> Self::Value,
        Transform: FnMut(Self::Discrim, &'t mut Self::Value) -> U,
    {
        let indices = discrims.map(|discrim| {
            match self.vec[..].binary_search_by_key(&discrim.into_usize(), |(d, _)| d.into_usize())
            {
                Ok(index) => index,
                Err(index) => {
                    self.vec.insert(index, (discrim, inserter(discrim)));
                    index
                }
            }
        });

        // Safety: self.vec.get_mut() is an injective function.
        unsafe {
            split_mut_array(
                &mut self.vec,
                |slice, index| slice.get_mut(index),
                indices,
                |_i, (d, t)| transform(*d, t),
                |_| unreachable!("returned by binary_search_by_key() or below len()"),
            )
        }
    }
}

/// Implements the requirements of [`Discrim::FullMap`] with O(1) search complexity,
/// using the `into_usize()` value as the index.
///
/// Optimized for discriminant types with bounded domain,
/// e.g. if the discriminant values are based on incremental ID.
#[derive(Clone)]
pub struct BoundedVecMap<D: Discrim, T> {
    vec: Vec<Option<T>>,
    _ph: PhantomData<D>,
}
impl<D: Discrim, T> FromIterator<(D, T)> for BoundedVecMap<D, T> {
    fn from_iter<I: IntoIterator<Item = (D, T)>>(iter: I) -> Self {
        let iter = iter.into_iter();

        let vec: Vec<Option<T>> = (0..iter.size_hint().0).map(|_| None).collect();
        let mut this = Self { vec, _ph: PhantomData };
        this.extend(iter);
        this
    }
}
impl<D: Discrim, T> Extend<(D, T)> for BoundedVecMap<D, T> {
    fn extend<I: IntoIterator<Item = (D, T)>>(&mut self, iter: I) {
        for (d, s) in iter {
            let index = d.into_usize();

            let required_len = index + 1;
            if self.vec.len() < required_len {
                self.vec.resize_with(required_len, || None);
            }

            let entry = self.vec.get_mut(index).expect("just resized");
            *entry = Some(s);
        }
    }
}
impl<D: Discrim, T> Mapped for BoundedVecMap<D, T> {
    type Discrim = D;
    type Key = D;
    type Value = T;

    fn get_discrim(&self, key: D) -> Option<Self::Discrim> { Some(key) }

    fn get_by(&self, key: D) -> Option<&T> {
        self.vec.get(key.into_usize()).and_then(Option::as_ref)
    }

    fn get_mut_by(&mut self, key: D) -> Option<&mut T> {
        self.vec.get_mut(key.into_usize()).and_then(Option::as_mut)
    }

    fn get_mut_array_by<'t, U, TransformFn, OnMissing, const M: usize>(
        &'t mut self,
        keys: [D; M],
        mut transform: TransformFn,
        mut on_missing: OnMissing,
    ) -> [U; M]
    where
        U: RefToPtr,
        TransformFn: FnMut(&'t mut Self::Value) -> U,
        OnMissing: FnMut(Self::Key) -> !,
    {
        // We collect the keys into [usize] first,
        // because we cannot ensure that `self.vec.get_mut(d.into_usize())` is injective.
        let indices = keys.map(D::into_usize);

        // Safety: `self.vec.get_mut` is injective for discrete usize.
        unsafe {
            split_mut_array(
                &mut self.vec,
                |slice, index| match slice.get_mut(index) {
                    Some(Some(value)) => Some(value),
                    _ => None,
                },
                indices,
                |_, t| transform(t),
                |i| on_missing(keys[i]),
            )
        }
    }

    type Iter<'t> = impl Iterator<Item = (D, D, &'t T)> + 't where Self: 't;
    fn iter_mapped(&self) -> Self::Iter<'_> {
        self.vec[..].iter().enumerate().filter_map(|(discrim, value)| {
            Some((D::from_usize(discrim), D::from_usize(discrim), value.as_ref()?))
        })
    }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut T)> + 't where Self: 't;
    fn iter_mapped_mut(&mut self) -> Self::IterMut<'_> {
        self.vec[..]
            .iter_mut()
            .enumerate()
            .filter_map(|(discrim, value)| Some((D::from_usize(discrim), value.as_mut()?)))
    }

    type IntoIter = impl Iterator<Item = (D, T)>;
    fn into_iter_mapped(self) -> Self::IntoIter {
        self.vec
            .into_iter()
            .enumerate()
            .filter_map(|(discrim, value)| Some((D::from_usize(discrim), value?)))
    }
}
impl<D: Discrim, T> FullMap for BoundedVecMap<D, T> {
    type MapRef<U> = BoundedVecMap<D, U>;
    fn map_ref<U, F>(&self, mut f: F) -> Self::MapRef<U>
    where
        F: FnMut(&Self::Value) -> U,
    {
        BoundedVecMap {
            vec: self.vec.iter().map(|option| option.as_ref().map(&mut f)).collect(),
            _ph: PhantomData,
        }
    }

    fn get_by_or_insert<F: FnOnce() -> T>(
        &mut self,
        discrim: Self::Discrim,
        inserter: F,
    ) -> &mut Self::Value {
        let index = discrim.into_usize();
        if self.vec.len() <= index {
            self.vec.resize_with(index + 1, || None);
        }

        let entry = self.vec.get_mut(index).expect("just resized");
        entry.get_or_insert_with(inserter)
    }

    fn get_by_or_insert_array<'t, U, Inserter, Transform, const N: usize>(
        &'t mut self,
        discrims: [Self::Discrim; N],
        mut inserter: Inserter,
        mut transform: Transform,
    ) -> [U; N]
    where
        U: RefToPtr,
        Inserter: FnMut(Self::Discrim) -> Self::Value,
        Transform: FnMut(Self::Discrim, &'t mut Self::Value) -> U,
    {
        let indices = discrims.map(|discrim| {
            let index = discrim.into_usize();
            if self.vec.len() <= index {
                self.vec.resize_with(index + 1, || None);
            }

            let entry = self.vec.get_mut(index).expect("just resized");
            _ = entry.get_or_insert_with(|| inserter(discrim));
            index
        });

        // Safety: self.vec.get_mut() is an injective function.
        unsafe {
            split_mut_array(
                &mut self.vec,
                |slice, index| slice.get_mut(index).expect("resize_with was called above").as_mut(),
                indices,
                |i, t| transform(D::from_usize(i), t),
                |_| unreachable!("get_or_insert_with was called above"),
            )
        }
    }
}

/// Implements the requirements of [`Discrim::FullMap`] with O(1) search complexity,
/// using the `into_usize()` value as the index.
///
/// Optimized for discriminant types with bounded domain known at compile time,
/// e.g. if the discriminant is derived from a enum without fields.
#[derive(Clone)]
pub struct ArrayMap<D: Discrim, T, const N: usize> {
    array: [Option<T>; N],
    _ph:   PhantomData<D>,
}
impl<D: Discrim, T, const N: usize> FromIterator<(D, T)> for ArrayMap<D, T, N> {
    fn from_iter<I: IntoIterator<Item = (D, T)>>(iter: I) -> Self {
        let array: [Option<T>; N] = [(); N].map(|()| None);
        let mut this = Self { array, _ph: PhantomData };
        for (d, s) in iter {
            match this.array.get_mut(d.into_usize()) {
                Some(Some(_)) => panic!("Duplicate value with discriminant {d:?}"),
                Some(option) => *option = Some(s),
                None => panic!(
                    "Discriminants using ArrayMap<N = {N}> must return integers less than {N} in \
                     into_usize()",
                ),
            }
        }
        this
    }
}
impl<D: Discrim, T, const N: usize> Mapped for ArrayMap<D, T, N> {
    type Discrim = D;
    type Key = D;
    type Value = T;

    fn get_discrim(&self, key: D) -> Option<Self::Discrim> { Some(key) }

    fn get_by(&self, key: D) -> Option<&T> {
        self.array.get(key.into_usize()).and_then(Option::as_ref)
    }

    fn get_mut_by(&mut self, key: D) -> Option<&mut T> {
        self.array.get_mut(key.into_usize()).and_then(Option::as_mut)
    }

    fn get_mut_array_by<'t, U, TransformFn, OnMissing, const M: usize>(
        &'t mut self,
        keys: [D; M],
        mut transform: TransformFn,
        mut on_missing: OnMissing,
    ) -> [U; M]
    where
        U: RefToPtr,
        TransformFn: FnMut(&'t mut Self::Value) -> U,
        OnMissing: FnMut(Self::Key) -> !,
    {
        // We collect the keys into [usize] first,
        // because we cannot ensure that `self.vec.get_mut(d.into_usize())` is injective.
        let indices = keys.map(D::into_usize);

        // Safety: `self.array.get_mut` is injective for discrete usize.
        unsafe {
            split_mut_array(
                &mut self.array,
                |slice, index| match slice.get_mut(index) {
                    Some(Some(value)) => Some(value),
                    _ => None,
                },
                indices,
                |_, t| transform(t),
                |i| on_missing(keys[i]),
            )
        }
    }

    type Iter<'t> = impl Iterator<Item = (D, D, &'t T)> + 't where Self: 't;
    fn iter_mapped(&self) -> Self::Iter<'_> {
        self.array[..].iter().enumerate().filter_map(|(discrim, value)| {
            Some((D::from_usize(discrim), D::from_usize(discrim), value.as_ref()?))
        })
    }

    type IterMut<'t> = impl Iterator<Item = (D, &'t mut T)> + 't where Self: 't;
    fn iter_mapped_mut(&mut self) -> Self::IterMut<'_> {
        self.array[..]
            .iter_mut()
            .enumerate()
            .filter_map(|(discrim, value)| Some((D::from_usize(discrim), value.as_mut()?)))
    }

    type IntoIter = impl Iterator<Item = (D, T)>;
    fn into_iter_mapped(self) -> Self::IntoIter {
        self.array
            .into_iter()
            .enumerate()
            .filter_map(|(discrim, value)| Some((D::from_usize(discrim), value?)))
    }
}
impl<D: Discrim, T, const N: usize> FullMap for ArrayMap<D, T, N> {
    type MapRef<U> = ArrayMap<D, U, N>;
    fn map_ref<U, F>(&self, mut f: F) -> Self::MapRef<U>
    where
        F: FnMut(&Self::Value) -> U,
    {
        let mut next_index = 0;
        let array = [(); N].map(|()| {
            let index = next_index;
            next_index += 1;
            self.array[index].as_ref().map(&mut f)
        });
        ArrayMap { array, _ph: PhantomData }
    }

    fn get_by_or_insert<F: FnOnce() -> T>(
        &mut self,
        discrim: Self::Discrim,
        inserter: F,
    ) -> &mut T {
        match self.array.get_mut(discrim.into_usize()) {
            Some(Some(entry)) => entry,
            Some(option) => option.insert(inserter()),
            None => {
                panic!(
                    "Discriminants using ArrayMap<N = {N}> must return integers less than {N} in \
                     into_usize()",
                )
            }
        }
    }

    fn get_by_or_insert_array<'t, U, Inserter, Transform, const M: usize>(
        &'t mut self,
        discrims: [Self::Discrim; M],
        mut inserter: Inserter,
        mut transform: Transform,
    ) -> [U; M]
    where
        U: RefToPtr,
        Inserter: FnMut(Self::Discrim) -> Self::Value,
        Transform: FnMut(Self::Discrim, &'t mut Self::Value) -> U,
    {
        let indices = discrims.map(|discrim| {
            let index = discrim.into_usize();

            let entry = match self.array.get_mut(index) {
                Some(entry) => entry,
                None => {
                    panic!(
                        "Discriminants using ArrayMap<N = {N}> must return integers less than {N} \
                         in into_usize()",
                    )
                }
            };
            _ = entry.get_or_insert_with(|| inserter(discrim));
            index
        });

        // Safety: self.array.get_mut() is an injective function.
        unsafe {
            split_mut_array(
                &mut self.array,
                |slice, index| slice.get_mut(index).expect("checked above").as_mut(),
                indices,
                |i, t| transform(D::from_usize(i), t),
                |_| unreachable!("get_or_insert_with was called above"),
            )
        }
    }
}
