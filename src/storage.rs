//! A storage is the data structure where components of the same type for all entities are stored.

use crate::entity;

mod vec;
pub use vec::VecStorage as Vec;

mod tree;
pub use tree::Tree;

pub(crate) mod simple;
pub(crate) use simple::Simple;
mod isotope;
pub(crate) use isotope::{AnyMap as AnyIsotopeMap, Map as IsotopeMap, MapInner as IsotopeMapInner};

#[cfg(test)]
mod tests;

/// A storage for storing component data.
pub trait Storage: Access + Default + Send + Sync + 'static {
    /// Gets a shared reference to the component for a specific entity if it is present.
    fn get(&self, id: Self::RawEntity) -> Option<&Self::Comp>;

    /// Sets or removes the component for a specific entity,
    /// returning the original value if it was present.
    fn set(&mut self, id: Self::RawEntity, value: Option<Self::Comp>) -> Option<Self::Comp>;

    /// Returns the number of components that exist in this storage.
    fn cardinality(&self) -> usize;

    /// Return value of [`iter`](Self::iter).
    type Iter<'t>: Iterator<Item = (Self::RawEntity, &'t Self::Comp)> + 't;
    /// Returns an immutable iterator over the storage, ordered by entity index order.
    fn iter(&self) -> Self::Iter<'_>;

    /// Return value of [`iter_chunks`](Self::iter_chunks).
    type IterChunks<'t>: Iterator<Item = ChunkRef<'t, Self>> + 't;
    /// Returns an immutable iterator of slices over the storage, ordered by entity index order.
    ///
    /// Each item yielded by the iterator is a tuple of `(index, slice)`,
    /// where `slice` is the slice of components in the chunk,
    /// and `index` is the entity index of `slice[0]`.
    /// `slice` is always nonempty.
    ///
    /// Non-chunked storages should implement this function by returning a chunk for each entity.
    fn iter_chunks(&self) -> Self::IterChunks<'_>;

    /// Return value of [`iter_chunks_mut`](Self::iter_chunks_mut).
    type IterChunksMut<'t>: Iterator<Item = ChunkMut<'t, Self>> + 't;
    /// Returns a mutable iterator of slices over the storage, ordered by entity index order.
    ///
    /// Each item yielded by the iterator is a tuple of `(index, slice)`,
    /// where `slice` is the slice of components in the chunk,
    /// and `index` is the entity index of `slice[0]`.
    /// `slice` is always nonempty.
    ///
    /// Non-chunked storages should implement this function by returning a chunk for each entity.
    fn iter_chunks_mut(&mut self) -> Self::IterChunksMut<'_>;

    /// Return value of [`as_partition`](Self::as_partition).
    type Partition<'u>: Partition<'u, RawEntity = Self::RawEntity, Comp = Self::Comp>
    where
        Self: 'u;
    /// Converts the storage to a [`Partition`] that covers the whole storage (similar to `slice[..]`).
    fn as_partition(&mut self) -> Self::Partition<'_>;
}

/// Borrows a slice of a storage, analogously `&'t mut Storage[..]`.
///
/// This trait does not provide `set` because
/// adding/removing items may cause rebalances in the tree implementation
/// and result in dangling references in other partitions that are not `&mut`-locked.
pub trait Partition<'t>: Access + Send + Sync + Sized + 't {
    /// Return value of [`by_ref`](Self::by_ref).
    type ByRef<'u>: Partition<'u, RawEntity = Self::RawEntity, Comp = Self::Comp>
    where
        Self: 'u;
    /// Re-borrows the partition with reduced lifetime.
    ///
    /// This is useful for calling [`into_iter_mut`](Self::into_iter_mut)
    /// and [`split_at`](Self::split_at),
    /// which take `self` as receiver to preserve the lifetime.
    fn by_ref(&mut self) -> Self::ByRef<'_>;

    /// Splits the partition further into two subpartitions.
    /// `entity` must be `> 0` and `< partition_length`,
    /// i.e. the expected key ranges of both partitions must be nonempty.
    /// (It is allowed to have a nonempty range which does not contain any existing keys)
    fn split_at(mut self, entity: Self::RawEntity) -> (Self, Self) {
        let right = self.split_out(entity);
        (self, right)
    }

    /// Splits the partition further into two subpartitions,
    /// replacing `self` with the left partition.
    ///
    /// `entity` must be `> 0` and `< partition_length`,
    /// i.e. the expected key ranges of both partitions must be nonempty.
    /// (It is allowed to have a nonempty range which does not contain any existing keys)
    fn split_out(&mut self, entity: Self::RawEntity) -> Self;

    /// Return value of [`into_iter_mut`](Self::into_iter_mut).
    type IntoIterMut: Iterator<Item = (Self::RawEntity, &'t mut Self::Comp)>;
    /// Same as [`iter_mut`](Access::iter_mut), but moves the partition object into the iterator.
    fn into_iter_mut(self) -> Self::IntoIterMut;

    /// Same as [`get_mut`](Access::get_mut), but returns a reference with lifetime `'t`.
    fn into_mut(self, entity: Self::RawEntity) -> Option<&'t mut Self::Comp>;

    /// Same as [`get_many_mut`](Access::get_many_mut), but returns a reference with lifetime `'t`.
    fn into_many_mut<const N: usize>(
        self,
        entities: [Self::RawEntity; N],
    ) -> Option<[&'t mut Self::Comp; N]>;
}

/// Mutable access functions for a storage, generalizing [`Storage`] and [`Partition`].
pub trait Access {
    /// The type of entity ID used for identification.
    type RawEntity: entity::Raw;
    /// The component type stored.
    type Comp: Send + Sync + 'static;

    /// Gets a mutable reference to the component for a specific entity if it is present.
    fn get_mut(&mut self, entity: Self::RawEntity) -> Option<&mut Self::Comp>;

    /// Gets mutable references to the components for specific entities if they are present.
    ///
    /// Returns `None` if any entity is uninitialized
    /// or if any entity appeared in `entities` more than once.
    fn get_many_mut<const N: usize>(
        &mut self,
        entities: [Self::RawEntity; N],
    ) -> Option<[&mut Self::Comp; N]>;

    /// Return value of [`iter_mut`](Self::iter_mut).
    type IterMut<'u>: Iterator<Item = (Self::RawEntity, &'u mut Self::Comp)> + 'u
    where
        Self: 'u;
    /// Returns a mutable iterator over the storage, ordered by entity index order.
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
}

/// Provides chunked access capabilities,
/// i.e. the storage can always return a slice for contiguous present components.
pub trait Chunked: Storage + AccessChunked {
    /// Gets a shared reference to a slice of components.
    ///
    /// Returns `None` if any of the components in the range is missing.
    ///
    /// Panics if `start > end`.
    fn get_chunk(&self, start: Self::RawEntity, end: Self::RawEntity) -> Option<&[Self::Comp]>;

    /// Return value of [`as_partition_chunk`](Self::as_partition_chunk).
    type PartitionChunked<'u>: PartitionChunked<'u, RawEntity = Self::RawEntity, Comp = Self::Comp>;
    /// Converts the storage to a [`PartitionChunked`] that covers the whole storage (similar to `slice[..]`).
    fn as_partition_chunk(&mut self) -> Self::PartitionChunked<'_>;
}

/// Mutable chunk access functions for a storage, generalizing [`Chunked`] and [`PartitionChunked`].
pub trait AccessChunked: Access {
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

/// Borrows a slice of a chunked storage, analogously `&'t mut Chunked[..]`.
///
/// This trait does not provide `set` because
/// adding/removing items may cause rebalances in the tree implementation
/// and result in dangling references in other partitions that are not `&mut`-locked.
pub trait PartitionChunked<'t>: Partition<'t> + AccessChunked {
    /// Gets a mutable reference to a slice of components,
    /// preserving the lifetime `'t` of this partition object.
    fn into_chunk_mut(
        self,
        start: Self::RawEntity,
        end: Self::RawEntity,
    ) -> Option<&'t mut [Self::Comp]>;

    /// Return value of [`into_iter_chunks_mut`](Self::into_iter_chunks_mut).
    type IntoIterChunksMut: Iterator<Item = (Self::RawEntity, &'t mut [Self::Comp])>;
    /// Returns a mutable iterator over the storage, ordered by entity index order.
    fn into_iter_chunks_mut(self) -> Self::IntoIterChunksMut;
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
