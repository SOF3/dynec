use std::fmt;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};

/// A raw entity ID.
///
/// Types implementing this trait are only used in storage internals.
pub trait Raw: Sized + Send + Sync + Copy + fmt::Debug + Eq + Ord + 'static {
    /// The atomic variant of this data type.
    type Atomic: Atomic<Self>;

    /// Creates the smallest value of this type in atomic form, used for initialization.
    fn new() -> Self::Atomic;

    /// Equivalent to `self + count`, does not mutate any values
    fn add(self, count: Primitive) -> Self;

    /// Equivalent to `self - other`, does not mutate any values. May panic when `self < other`.
    fn sub(self, other: Self) -> Primitive;

    /// Converts the primitive scalar to the ID.
    /// The scalar is guaranteed to be valid, returned from a previous `to_scalar()` call.
    fn from_primitive(i: Primitive) -> Self;

    /// Converts the ID to a scalar.
    ///
    /// The returned scalar is used for indexing in Vec-based storages, so it should start from a
    /// small number.
    fn to_primitive(self) -> Primitive;
}

impl Raw for NonZeroU32 {
    type Atomic = AtomicU32;

    fn new() -> Self::Atomic { AtomicU32::new(1) }

    fn add(self, count: usize) -> Self {
        let count: u32 = count.try_into().expect("count is too large");
        NonZeroU32::new(self.get() + count).expect("integer overflow")
    }

    fn sub(self, other: Self) -> usize {
        (self.get() - other.get()).try_into().expect("usize >= u32")
    }

    fn from_primitive(i: Primitive) -> Self {
        i.try_into().ok().and_then(Self::new).expect("Invalid usize")
    }

    fn to_primitive(self) -> Primitive { self.get().try_into().expect("Too many entities") }
}

/// An atomic variant of [`Raw`].
pub trait Atomic<R: Raw>: Send + Sync + 'static {
    /// Equivalent to `AtomicUsize::fetch_add(self, count, Ordering::SeqCst)`
    fn fetch_add(&self, count: usize) -> R;

    /// Equivalent to `AtomicUsize::load(self, Ordering::SeqCst)`
    fn load(&self) -> R;

    /// Equivalent to `AtomicUsize::get_mut(self)`.
    ///
    /// This is semantically identical to `load`, but should be slightly faster
    /// because it does not require atomic loading.
    fn load_mut(&mut self) -> R;
}

impl Atomic<NonZeroU32> for AtomicU32 {
    fn fetch_add(&self, count: usize) -> NonZeroU32 {
        let original = AtomicU32::fetch_add(
            self,
            count.try_into().expect("count is too large"),
            Ordering::SeqCst,
        );
        NonZeroU32::new(original).expect("integer overflow")
    }

    fn load(&self) -> NonZeroU32 {
        let original = AtomicU32::load(self, Ordering::SeqCst);
        NonZeroU32::new(original).expect("invalid state")
    }

    fn load_mut(&mut self) -> NonZeroU32 {
        let original = *AtomicU32::get_mut(self);
        NonZeroU32::new(original).expect("invalid state")
    }
}

/// The primitive scalar type.
pub type Primitive = usize;
