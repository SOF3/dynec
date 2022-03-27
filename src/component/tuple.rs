//! This module is used to process multiple components in a compact orm.

/// This trait is implemented for all tuples of component types
///
/// Note that tuples are always assumed to have distinct types.
/// While this cannot be checked at compile time,
/// passing any non-distinct tuple types to the dynec API may cause panics.
pub trait Tuple {}
