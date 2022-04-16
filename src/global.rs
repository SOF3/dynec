use crate::entity;

/// A global state that can be requested by all systems.
pub trait Global: entity::Referrer + Sized + 'static {}
