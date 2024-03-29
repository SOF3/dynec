//! Miscellaneous types used in the API.

#[cfg(debug_assertions)]
use std::any;
use std::any::TypeId;
use std::borrow::Borrow;
use std::{cmp, fmt, hash, mem, num, ops};

/// A generic mutable/immutable reference type.
pub trait Ref {
    /// The owned type.
    type Target: ?Sized;

    /// Whether the reference is mutable.
    const MUTABLE: bool;

    /// Converts the reference to a shared reference.
    fn as_ref(&self) -> &Self::Target;
}

impl<T: ?Sized> Ref for &T {
    type Target = T;

    const MUTABLE: bool = false;

    fn as_ref(&self) -> &T { self }
}

impl<T: ?Sized> Ref for &mut T {
    type Target = T;

    const MUTABLE: bool = true;

    fn as_ref(&self) -> &T { self }
}

/// Wraps a double-deref type so that `*self` is equivalent to `**self.0`
#[derive(Clone, Copy)]
pub struct DoubleDeref<T>(pub T);

impl<T> ops::Deref for DoubleDeref<T>
where
    T: ops::Deref,
    T::Target: ops::Deref,
{
    type Target = <T::Target as ops::Deref>::Target;
    fn deref(&self) -> &Self::Target { &self.0 }
}

/// A container that implements [`ops::Deref`]/[`ops::DerefMut`]
/// without any special logic.
pub struct OwnedDeref<T>(pub T);

impl<T> ops::Deref for OwnedDeref<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}
impl<T> ops::DerefMut for OwnedDeref<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.0 }
}

/// A TypeId that may include type name for debugging.
#[derive(Debug, Clone, Copy)]
pub struct DbgTypeId {
    /// The actual [`TypeId`].
    pub id: TypeId,
    #[cfg(debug_assertions)]
    name:   &'static str,
}

impl DbgTypeId {
    /// Creates a new [`DbgTypeId`], similar to [`TypeId::of`].
    pub fn of<T: 'static>() -> Self {
        Self {
            id:                            TypeId::of::<T>(),
            #[cfg(debug_assertions)]
            name:                          any::type_name::<T>(),
        }
    }
}

impl fmt::Display for DbgTypeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(debug_assertions)]
        {
            write!(f, "{}", self.name)
        }

        #[cfg(not(debug_assertions))]
        {
            write!(f, "{:?}", self.id)
        }
    }
}

impl PartialEq for DbgTypeId {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}

impl PartialEq<TypeId> for DbgTypeId {
    fn eq(&self, other: &TypeId) -> bool { self.id == *other }
}

impl Eq for DbgTypeId {}

impl PartialOrd for DbgTypeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

impl Ord for DbgTypeId {
    fn cmp(&self, other: &Self) -> cmp::Ordering { self.id.cmp(&other.id) }
}

impl hash::Hash for DbgTypeId {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        <TypeId as hash::Hash>::hash(&self.id, state);
    }
}

impl Borrow<TypeId> for DbgTypeId {
    fn borrow(&self) -> &TypeId { &self.id }
}

/// Same as [`Eq`] and [`Ord`], but with a stronger guarantee.
///
/// # Safety
/// Undefined behavior may occur if the invariants of `Eq` and `Ord` are not fully satisfied.
pub unsafe trait UnsafeEqOrd: Eq + Ord {}

// Safety: NonZeroU16 is semantically identical to `u16`,
// which is a regular primitive satisfying all equivalence and ordering invariants.
unsafe impl UnsafeEqOrd for num::NonZeroU16 {}

// Safety: NonZeroU32 is semantically identical to `u32`,
// which is a regular primitive satisfying all equivalence and ordering invariants.
unsafe impl UnsafeEqOrd for num::NonZeroU32 {}

// Safety: NonZeroU64 is semantically identical to `u64`,
// which is a regular primitive satisfying all equivalence and ordering invariants.
unsafe impl UnsafeEqOrd for num::NonZeroU64 {}

// Safety: `usize` is a regular primitive satisfying all equivalence and ordering invariants.
unsafe impl UnsafeEqOrd for usize {}

/// Transforms a value behind a mutable reference with a function that moves it.
///
/// The placeholder value will be left at the position of `ref_` if the transform function panics.
pub(crate) fn transform_mut<T, R>(
    ref_: &mut T,
    placeholder: T,
    transform: impl FnOnce(T) -> (T, R),
) -> R {
    let old = mem::replace(ref_, placeholder);
    let (new, ret) = transform(old);
    *ref_ = new;
    ret
}

pub(crate) fn is_all_distinct_quadtime<T: PartialEq>(slice: &[T]) -> bool {
    for (i, item) in slice.iter().enumerate() {
        if !slice[(i + 1)..].iter().all(|other| item == other) {
            return false;
        }
    }
    true
}
