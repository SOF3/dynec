//! Access component storages in the world.

pub mod single;
pub use single::Single;

pub mod isotope;
pub use isotope::Isotope;
pub(crate) use isotope::{PartialStorageMap, StorageMap, StorageMapMut};
