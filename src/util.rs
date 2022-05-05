//! Miscellaneous traits used for exposing type bounds in the API.

use core::fmt;
#[cfg(debug_assertions)]
use std::any;
use std::any::TypeId;
use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::{cmp, hash};

/// A generic mutable/immutable reference type.
pub trait Ref {
    /// The owned type.
    type Target: ?Sized;

    /// Whether the reference is mutable.
    const MUTABLE: bool;

    /// Converts the reference to a shared reference.
    fn as_ref(&self) -> &Self::Target;
}

impl<'t, T: ?Sized> Ref for &'t T {
    type Target = T;

    const MUTABLE: bool = false;

    fn as_ref(&self) -> &T { self }
}

impl<'t, T: ?Sized> Ref for &'t mut T {
    type Target = T;

    const MUTABLE: bool = true;

    fn as_ref(&self) -> &T { self }
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
            writeln!(f, "`{}`", self.name)
        }

        #[cfg(not(debug_assertions))]
        {
            writeln!(f, "{:?}", self.id)
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

#[inline]
pub(crate) fn btreeset_remove_first<T: Eq + Ord + Copy>(set: &mut BTreeSet<T>) -> Option<T> {
    #[cfg(feature = "map-first-last")]
    {
        set.pop_first()
    }

    #[cfg(not(feature = "map-first-last"))]
    {
        let item = *set.iter().next()?;
        Some(set.take(&item).expect("equality is not reflexive"))
    }
}
