//! Miscellaneous traits used for exposing type bounds in the API.

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
