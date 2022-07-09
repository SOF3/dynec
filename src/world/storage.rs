//! A storage is the data structure where components of the same type for all entities are stored.

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
pub(crate) use isotope::{Factory as IsotopeFactory, Isotope};

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

    /// Returns an immutable iterator over the storage, ordered by entity index order.
    fn iter(&self) -> Box<dyn Iterator<Item = (Self::RawEntity, &Self::Comp)> + '_>;

    /// Returns a mutable iterator over the storage, ordered by entity index order.
    fn iter_mut(&mut self) -> Box<dyn Iterator<Item = (Self::RawEntity, &mut Self::Comp)> + '_>;
}
