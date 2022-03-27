/// A generic mutable/immutable reference type
pub trait Ref {
    type Target: ?Sized;

    const MUTABLE: bool;

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
