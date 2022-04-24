//! Specifies the requirements for a system.

use std::any::TypeId;

use crate::world::storage;
use crate::{component, system, Archetype};

/// Describes an instance of system.
///
/// There may be multiple instances of the same implementor type.
/// This is meaningful as they may have different states.
pub trait Spec {
    /// The debug name of the system.
    fn debug_name(&self) -> String;

    /// Executes the given function on each dependency.
    fn for_each_dependency(&self, f: &mut dyn FnMut(Dependency));

    /// Executes the given function on each global state request.
    fn for_each_global_request(&self, f: &mut dyn FnMut(GlobalRequest));

    /// Executes the given function on each simple component read/write request.
    fn for_each_simple_request(&self, f: &mut dyn FnMut(SimpleRequest));

    /// Executes the given function on each isotope component read/write request.
    fn for_each_isotope_request(&self, f: &mut dyn FnMut(IsotopeRequest));

    /// Runs the system.
    fn run(&mut self);
}

/// Indicates the dependency of a system.
pub enum Dependency {
    /// The system must execute before the given partition.
    Before(Box<dyn system::Partition>),
    /// The system must execute after the given partition.
    After(Box<dyn system::Partition>),
}

impl Dependency {
    /// The system must execute before the given partition.
    pub fn before(p: impl system::Partition) -> Self { Self::Before(Box::new(p)) }

    /// The system must execute after the given partition.
    pub fn after(p: impl system::Partition) -> Self { Self::After(Box::new(p)) }
}

/// Indicates that the system requires a global state.
pub struct GlobalRequest {
    /// The type of the global state.
    pub global:  TypeId,
    /// Whether mutable access is requested.
    pub mutable: bool,
    /// Whether the resource requires thread safety.
    pub sync:    bool,
}

/// Indicates that the system requires a simple component read/write.
pub struct SimpleRequest {
    /// The archetype requested.
    pub(crate) archetype:       TypeId,
    /// The type of the simple component.
    pub(crate) component:       TypeId,
    /// Whether mutable access is requested.
    pub(crate) mutable:         bool,
    pub(crate) storage_builder: fn() -> storage::Shared,
}

impl SimpleRequest {
    pub fn new<A: Archetype, C: component::Simple<A>>(mutable: bool) -> Self {
        Self {
            archetype: TypeId::of::<A>(),
            component: TypeId::of::<C>(),
            mutable,
            storage_builder: || storage::shared_simple::<A, C>(),
        }
    }
}

/// Indicates that the system requires an isotope component read/write.
pub struct IsotopeRequest {
    /// The archetype requested.
    pub archetype: TypeId,
    /// The archetype of the isotope component.
    pub component: TypeId,
    /// If `Some`, only the isotope components of the given discriminants are accessible.
    ///
    /// This will not lead to creation of the discriminant storages.
    pub discrim:   Option<Vec<usize>>,
    /// Whether mutable access is requested.
    pub mutable:   bool,
}
