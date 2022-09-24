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

/// A storage for storing component data.
pub trait Storage: Default + Send + Sync + 'static {
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
