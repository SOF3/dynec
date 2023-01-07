//! Miscellaneous types used in the API.

#[cfg(debug_assertions)]
use std::any;
use std::any::TypeId;
use std::borrow::Borrow;
use std::num::NonZeroU32;
use std::{cmp, fmt, hash, ops};

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

// Safety: NonZeroU32 is semantically identical to `u32`,
// which is a regular primitive satisfying all equivalence and ordering invariants.
unsafe impl UnsafeEqOrd for NonZeroU32 {}

// Safety: `usize` is a regular primitive satisfying all equivalence and ordering invariants.
unsafe impl UnsafeEqOrd for usize {}
