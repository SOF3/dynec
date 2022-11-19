use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{fmt, ops};

/// A raw entity ID.
///
/// Types implementing this trait are only used in storage internals.
///
/// # Safety
/// Violating the [`Eq`] and [`Ord`] invariants leads to undefined behavior.
/// Furthermore, `from_primitive` and `to_primitive` should preserve the equivalence and ordering.
// Such violations normally only lead to *unexpected* but not *undefined* behavior in std,
// but non-transitive implementors of `Raw` could lead to incorrect soundness validation
// in code that iterates over a storage mutably by strictly increasing index.
// The safety constraint here is necessary for the safety constraint in `Storage` to hold.
pub unsafe trait Raw: Sized + Send + Sync + Copy + fmt::Debug + Eq + Ord + 'static {
    /// The atomic variant of this data type.
    type Atomic: Atomic<Self>;

    /// Creates the smallest value of this type in atomic form, used for initialization.
    fn new() -> Self::Atomic;

    /// Equivalent to `self + count`, does not mutate any values
    fn add(self, count: Primitive) -> Self;

    /// Equivalent to `self - other`, does not mutate any values. May panic when `self < other`.
    fn sub(self, other: Self) -> Primitive;

    /// Converts the primitive scalar to the ID.
    /// The primitive scalar is guaranteed to be valid,
    /// returned from a previous [`to_primitive`](Self::to_primitive) call.
    fn from_primitive(i: Primitive) -> Self;

    /// Converts the ID to a primitive scalar.
    ///
    /// The returned primitive scalar is used for indexing in Vec-based storages,
    /// so it should start from a small number.
    fn to_primitive(self) -> Primitive;

    /// Return value of [`range`](Self::range).
    type Range: Iterator<Item = Self>;
    /// Iterates over a range.
    fn range(range: ops::Range<Self>) -> Self::Range;
}

// Safety: NonZeroU32 is semantically identical to `u32`,
// which is a regular primitive satisfying all equivalence and ordering invariants.
unsafe impl Raw for NonZeroU32 {
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

    type Range = impl Iterator<Item = Self>;
    fn range(range: ops::Range<Self>) -> Self::Range {
        (range.start.get()..range.end.get()).map(|v| {
            NonZeroU32::new(v).expect("zero does not appear between two non-zero unsigned integers")
        })
    }
}

/// An atomic variant of [`Raw`].
pub trait Atomic<E: Raw>: Send + Sync + 'static {
    /// Equivalent to `AtomicUsize::fetch_add(self, count, Ordering::SeqCst)`
    fn fetch_add(&self, count: usize) -> E;

    /// Equivalent to `AtomicUsize::load(self, Ordering::SeqCst)`
    fn load(&self) -> E;

    /// Equivalent to `AtomicUsize::get_mut(self)`.
    ///
    /// This is semantically identical to `load`, but should be slightly faster
    /// because it does not require atomic loading.
    fn load_mut(&mut self) -> E;
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
