//! Access component storages in the world.

mod single;
pub use single::AccessSingle;

mod isotope;
pub use isotope::AccessIsotope;
pub(crate) use isotope::{PartialStorageMap, StorageMap, StorageMapMut};

mod iter;
pub use iter::{IntoZip, Try, Zip, ZipChunked};
