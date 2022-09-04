//! A storage is the data structure where components of the same type for all entities are stored.

use core::slice;

use crate::entity;

mod vec;
pub use vec::VecStorage as Vec;

mod tree;
pub use tree::Tree;

pub mod mux;
pub use mux::Mux;

mod simple;
pub(crate) use simple::Simple;
mod isotope;
pub(crate) use isotope::{AnyIsotopeStorage, Factory as IsotopeFactory, Isotope};

/// A [`Mux`] that uses a [`Tree`] and [`Vec`] as the backends.
pub type MapVecMux<E, C> = Mux<E, C, Tree<E, C>, Vec<E, C>>;

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

    /// Returns an immutable iterator over the storage, ordered by entity index order.
    fn iter(&self) -> Box<dyn Iterator<Item = (Self::RawEntity, &Self::Comp)> + '_>;

    /// Returns an immutable iterator of slices over the storage, ordered by entity index order.
    ///
    /// Each item yielded by the iterator is a tuple of `(index, slice)`,
    /// where `slice` is the slice of components in the chunk,
    /// and `index` is the entity index of `slice[0]`.
    /// `slice` is always nonempty.
    #[inline]
    fn iter_chunks(&self) -> Box<dyn Iterator<Item = ChunkRef<'_, Self>> + '_> {
        Box::new(
            self.iter()
                .map(|(entity, item)| ChunkRef { slice: slice::from_ref(item), start: entity }),
        )
    }

    /// Returns a mutable iterator over the storage, ordered by entity index order.
    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = (Self::RawEntity, &mut Self::Comp)> + '_>;

    /// Returns a mutable iterator of slices over the storage, ordered by entity index order.
    ///
    /// Each item yielded by the iterator is a tuple of `(index, slice)`,
    /// where `slice` is the slice of components in the chunk,
    /// and `index` is the entity index of `slice[0]`.
    /// `slice` is always nonempty.
    #[inline]
    fn iter_chunks_mut(&mut self) -> Box<dyn Iterator<Item = ChunkMut<'_, Self>> + '_> {
        Box::new(
            self.iter_mut()
                .map(|(entity, item)| ChunkMut { slice: slice::from_mut(item), start: entity }),
        )
    }
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
