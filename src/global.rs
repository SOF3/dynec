use std::any;

use crate::entity;

/// A global state that can be requested by all systems.
///
/// A global state may be a transitive owner of entity references.
/// Thus, all global states must implement [`entity::Referrer`].
pub trait Global: entity::Referrer + Sized + 'static {
    /// This method is called during [`world::Builder::build`](crate::world::Builder::build)
    /// if some system requests this type but this type was not provided separately.
    ///
    /// The default implementation panics.
    /// Users are expected to override this method if a default value is intended.
    fn initial() -> Self {
        panic!(
            "Global type {} does not have an initial impl and was not provided manually",
            any::type_name::<Self>()
        )
    }
}
