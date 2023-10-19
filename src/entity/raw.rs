use std::sync::atomic;
use std::{fmt, ops};

use crate::util::UnsafeEqOrd;

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
pub trait Raw: Sized + Send + Sync + Copy + fmt::Debug + UnsafeEqOrd + 'static {
    /// The atomic variant of this data type.
    type Atomic: Atomic<Self>;

    /// Creates the smallest value of this type in atomic form, used for initialization.
    fn new() -> Self::Atomic;

    /// Equivalent to `self + count`, does not mutate any values
    fn add(self, count: Primitive) -> Self;

    /// Equivalent to `self - other`, does not mutate any values. May panic when `self < other`.
    fn sub(self, other: Self) -> Primitive;

    /// Returns the approximated midpoint between two numbers.
    ///
    /// It does not need to be strictly accurate
    /// as it is only an approximation used in optimization algorithms.
    fn approx_midpoint(self, other: Self) -> Self;

    /// Converts the primitive scalar to the ID.
    ///
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

macro_rules! impl_raw {
    ($base:ty, $atomic:ty, $primitive:ty) => {
        impl Raw for $base {
            type Atomic = $atomic;

            fn new() -> Self::Atomic { <$atomic>::new(1) }

            fn add(self, count: usize) -> Self {
                let count: $primitive = count.try_into().expect("count is too large");
                <$base>::new(self.get() + count).expect("integer overflow")
            }

            fn sub(self, other: Self) -> usize {
                (self.get() - other.get()).try_into().expect("usize should be sufficiently large")
            }

            fn approx_midpoint(self, other: Self) -> Self {
                <$base>::new((self.get() + other.get()) / 2)
                    .expect("get() >= 1, get() + get() >= 2, half >= 1")
            }

            fn from_primitive(i: Primitive) -> Self {
                i.try_into().ok().and_then(Self::new).expect("Invalid usize")
            }

            fn to_primitive(self) -> Primitive { self.get().try_into().expect("Too many entities") }

            type Range = impl Iterator<Item = Self>;
            fn range(range: ops::Range<Self>) -> Self::Range {
                (range.start.get()..range.end.get()).map(|v| {
                    // Safety: v >= range.start.get(), which is guaranteed to be non-zero.
                    // Unsafe is necessary here because this function is called during chunk iteration,
                    // and this branch may break vectorization.
                    unsafe { <$base>::new_unchecked(v) }
                })
            }
        }

        impl Atomic<$base> for $atomic {
            fn fetch_add(&self, count: usize) -> $base {
                let original = <$atomic>::fetch_add(
                    self,
                    count.try_into().expect("count is too large"),
                    atomic::Ordering::SeqCst,
                );
                <$base>::new(original).expect("integer overflow")
            }

            fn load(&self) -> $base {
                let original = <$atomic>::load(self, atomic::Ordering::SeqCst);
                <$base>::new(original).expect("invalid state")
            }

            fn load_mut(&mut self) -> $base {
                let original = *<$atomic>::get_mut(self);
                <$base>::new(original).expect("invalid state")
            }
        }
    };
}

impl_raw!(std::num::NonZeroU16, std::sync::atomic::AtomicU16, u16);
impl_raw!(std::num::NonZeroU32, std::sync::atomic::AtomicU32, u32);
impl_raw!(std::num::NonZeroU64, std::sync::atomic::AtomicU64, u64);

/// The primitive scalar type.
pub type Primitive = usize;
