//! A storage is the data structure where components of the same type for all entities are stored.

use crate::entity;

mod vec;
pub use vec::VecStorage as Vec;

mod tree;
pub use tree::Tree;

mod simple;
pub(crate) use simple::Simple;
mod isotope;
pub(crate) use isotope::{AnyMap as AnyIsotopeMap, Map as IsotopeMap, MapInner as IsotopeMapInner};

#[cfg(test)]
mod tests;

/// A storage for storing component data.
///
/// # Safety
/// Implementors of this trait must ensure that
/// [`get`](Self::get) and [`get_mut`](Self::get_mut) are consistent and [injective][injective].
/// In other words, for any `a: Self::RawEntity`,
/// `get(a)` and `get_mut(a)` return the same value (only differing by mutability),
/// and for any `b: Self::RawEntity` where `a != b`, `get(a)` must not alias `get(b)`.
///
/// This implies the implementation is not safe if
/// [`Eq`] and [`Ord`] are incorrectly implemented for `Self::RawEntity`,
/// which is why [`entity::Raw`] is an unsafe trait
/// that strictly requires complete equivalence and ordering.
/// (Imagine if `RawEntity` is [`f64`], and `a` and `b` are both [`f64::NAN`];
/// then `a != b` but `get_mut(a)` would still alias `get_mut(b)`)
///
/// [injective]: https://en.wikipedia.org/wiki/Injective_function
pub unsafe trait Storage: Default + Send + Sync + 'static {
    /// The type of entity ID used for identification.
    type RawEntity: entity::Raw;
    /// The component type stored.
    type Comp;

    /// Gets a shared reference to the component for a specific entity if it is present.
    fn get(&self, id: Self::RawEntity) -> Option<&Self::Comp>;

    /// Gets a mutable reference to the component for a specific entity if it is present.
    fn get_mut(&mut self, id: Self::RawEntity) -> Option<&mut Self::Comp>;

    /// Sets or removes the component for a specific entity,
    /// returning the original value if it was present.
    fn set(&mut self, id: Self::RawEntity, value: Option<Self::Comp>) -> Option<Self::Comp>;

    /// Returns the number of components that exist in this storage.
    fn cardinality(&self) -> usize;

    /// Return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = (Self::RawEntity, &'t Self::Comp)> + 't;
    /// Returns an immutable iterator over the storage, ordered by entity index order.
    fn iter(&self) -> Self::Iter<'_>;

    /// Return value of [`iter_chunk`](Self::iter_chunks).
    type IterChunks<'t>: Iterator<Item = ChunkRef<'t, Self>> + 't;
    /// Returns an immutable iterator of slices over the storage, ordered by entity index order.
    ///
    /// Each item yielded by the iterator is a tuple of `(index, slice)`,
    /// where `slice` is the slice of components in the chunk,
    /// and `index` is the entity index of `slice[0]`.
    /// `slice` is always nonempty.
    fn iter_chunks(&self) -> Self::IterChunks<'_>;

    /// Return value of [`iter_mut`](Self::iter_mut).
    type IterMut<'t>: Iterator<Item = (Self::RawEntity, &'t mut Self::Comp)> + 't;
    /// Returns a mutable iterator over the storage, ordered by entity index order.
    fn iter_mut(&mut self) -> Self::IterMut<'_>;

    /// Return value of [`iter_chunk_mut`](Self::iter_chunks_mut).
    type IterChunksMut<'t>: Iterator<Item = ChunkMut<'t, Self>> + 't;
    /// Returns a mutable iterator of slices over the storage, ordered by entity index order.
    ///
    /// Each item yielded by the iterator is a tuple of `(index, slice)`,
    /// where `slice` is the slice of components in the chunk,
    /// and `index` is the entity index of `slice[0]`.
    /// `slice` is always nonempty.
    fn iter_chunks_mut(&mut self) -> Self::IterChunksMut<'_>;
}

/// Provides chunked access capabilities,
/// i.e. the storage can always return a slice for contiguous present components.
///
/// # Safety
/// Implementors of this trait must ensure that
/// [`get_chunk`](Self::get_chunk) and [`get_chunk_mut`](Self::get_chunk_mut) are consistent,
/// and non-overlapping ranges map to non-overlapping slices.
/// In other words, for any `a, b: Self::RawEntity` where `a < b`,
/// `get_chunk(a, b)` and `get_chunk_mut(a, b)` return the same slice
/// (only differing by mutability),
/// and for any `c, d: Self::RawEntity` where `b <= c` `c < d`,
/// `get_chunk(a, b)` must not alias `get_chunk(c, d)`.
///
/// [injective]: https://en.wikipedia.org/wiki/Injective_function
pub unsafe trait Chunked: Storage {
    /// Gets a shared reference to a slice of components.
    ///
    /// Returns `None` if any of the components in the range is missing.
    ///
    /// Panics if `start > end`.
    fn get_chunk(&self, start: Self::RawEntity, end: Self::RawEntity) -> Option<&[Self::Comp]>;

    /// Gets a mutable reference to a slice of components.
    ///
    /// Returns `None` if any of the components in the range is missing.
    ///
    /// Panics if `start > end`.
    fn get_chunk_mut(
        &mut self,
        start: Self::RawEntity,
        end: Self::RawEntity,
    ) -> Option<&mut [Self::Comp]>;
}

/// The iterator item of [`Storage::iter_chunks`].
pub struct ChunkRef<'t, S: Storage> {
    /// The slice of components in the chunk.
    pub slice: &'t [S::Comp],
    /// The entity index of `slice[0]`.
    pub start: S::RawEntity,
}

/// The iterator item of [`Storage::iter_chunks_mut`].
pub struct ChunkMut<'t, S: Storage> {
    /// The slice of components in the chunk.
    pub slice: &'t mut [S::Comp],
    /// The entity index of `slice[0]`.
    pub start: S::RawEntity,
}
