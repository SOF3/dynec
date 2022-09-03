use std::any::{Any, TypeId};
use std::marker::PhantomData;
use std::slice;

/// A `&'t mut [T]` with type elided.
pub struct AnySliceMut<'t> {
    ptr: *mut (),
    len: usize,
    ty:  TypeId,
    _ph: PhantomData<&'t mut (dyn Any)>,
}

impl<'t> AnySliceMut<'t> {
    /// Constructs a new type-elided slice.
    pub fn new<T: 'static>(slice: &'t mut [T]) -> Self {
        let len = slice.len();
        let ptr = slice.as_mut_ptr();

        Self { ptr: ptr as *mut (), len, ty: TypeId::of::<T>(), _ph: PhantomData }
    }

    /// Creates an empty slice.
    pub fn default<T: 'static>() -> Self { Self::new::<T>(&mut []) }

    /// Creates a singleton slice.
    pub fn from<T: 'static>(item: &'t mut T) -> Self { Self::new::<T>(slice::from_mut(item)) }

    /// Creates a singleton slice from an [`Any`] object.
    pub fn from_any(item: &'t mut dyn Any) -> Self {
        let ty = <dyn Any>::type_id(item);
        let ptr = item as *mut dyn Any as *mut ();

        // Safety: this is effectively equivalent to `slice::from_mut()`.

        Self { ptr, len: 1, ty, _ph: PhantomData }
    }

    /// Reify the slice with a concrete type.
    ///
    /// # Panics
    /// Panics if the type differs from the type used for creation.
    pub fn downcast<T: 'static>(self) -> &'t mut [T] {
        assert_eq!(self.ty, TypeId::of::<T>(), "TypeId mismatch");

        if self.ptr.is_null() {
            return &mut [];
        }

        unsafe {
            // Safety:
            // - data are valid because we are just reverting the decomposition step in `new`.
            // - Access is unique since the receiver is `&mut self`
            slice::from_raw_parts_mut(self.ptr as *mut T, self.len)
        }
    }

    /// Acquire ownership of the slice.
    /// It is expected to use this struct with `.reborrow().downcast()`.
    pub fn reborrow<'sub>(&'sub mut self) -> AnySliceMut<'sub>
    where
        'sub: 't,
    {
        Self { ptr: self.ptr, len: self.len, ty: self.ty, _ph: PhantomData }
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use super::AnySliceMut;

    #[test]
    fn test_default() {
        let slice = AnySliceMut::default::<i32>();
        assert_eq!(slice.downcast::<i32>(), &mut []);
    }

    #[test]
    #[should_panic = "TypeId mismatch"]
    fn test_default_type_mismatch() {
        let slice = AnySliceMut::default::<i32>();
        slice.downcast::<u32>();
    }

    #[test]
    fn test_new() {
        let original = &mut [1, 2];
        let slice = AnySliceMut::new(original);
        assert_eq!(slice.downcast::<i32>(), &mut [1, 2]);
    }

    #[test]
    #[should_panic = "TypeId mismatch"]
    fn test_new_type_mismatch() {
        let original: &mut [i32] = &mut [1, 2];
        let slice = AnySliceMut::new(original);
        slice.downcast::<u32>();
    }

    #[test]
    fn test_from() {
        let mut temp = 1_i32;
        let slice = AnySliceMut::from(&mut temp);
        assert_eq!(slice.downcast::<i32>(), &mut [1]);
    }

    #[test]
    #[should_panic = "TypeId mismatch"]
    fn test_from_type_mismatch() {
        let mut temp = 1_i32;
        let slice = AnySliceMut::from(&mut temp);
        slice.downcast::<u32>();
    }

    #[test]
    fn test_from_any() {
        let mut temp = 1_i32;
        let temp_any: &mut dyn Any = &mut temp;
        let slice = AnySliceMut::from_any(temp_any);
        assert_eq!(slice.downcast::<i32>(), &mut [1]);
    }

    #[test]
    #[should_panic = "TypeId mismatch"]
    fn test_from_any_type_mismatch() {
        let mut temp = 1_i32;
        let temp_any: &mut dyn Any = &mut temp;
        let slice = AnySliceMut::from_any(temp_any);
        slice.downcast::<u32>();
    }
}
