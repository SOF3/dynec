//! Specifies the requirements for a system.

use std::any::Any;

use crate::entity::ealloc;
use crate::util::DbgTypeId;
use crate::world::{self, storage};
use crate::{comp, system, Archetype, Global};

/// Describes an instance of system.
pub struct Spec {
    /// The debug name of the system.
    pub debug_name:       String,
    /// The partition dependencies related to the system.
    pub dependencies:     Vec<Dependency>,
    /// The global states requested by the system.
    pub global_requests:  Vec<GlobalRequest>,
    /// The simple components requested by the system.
    pub simple_requests:  Vec<SimpleRequest>,
    /// The isotope components requested by the system.
    pub isotope_requests: Vec<IsotopeRequest>,
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
    pub ty:      DbgTypeId,
    /// A closure that calls [`Global::initial`].
    pub initial: GlobalInitial,
    /// Whether mutable access is requested.
    pub mutable: bool,
}

/// Specifies the initializer for a global type.
#[derive(Clone, Copy)]
pub enum GlobalInitial {
    /// Used for thread-safe globals.
    Sync(fn() -> Box<dyn Any + Send + Sync>),
    /// Used for thread-unsafe globals.
    Unsync(fn() -> Box<dyn Any>),
}

impl GlobalRequest {
    /// Creates a new thread-safe global state request with types known at compile time.
    pub fn new_sync<G: Global + Send + Sync>(mutable: bool) -> Self {
        Self {
            ty: DbgTypeId::of::<G>(),
            initial: GlobalInitial::Sync(|| Box::new(G::initial())),
            mutable,
        }
    }

    /// Creates a new thread-unsafe global state request with types known at compile time.
    pub fn new_unsync<G: Global>(mutable: bool) -> Self {
        Self {
            ty: DbgTypeId::of::<G>(),
            initial: GlobalInitial::Unsync(|| Box::new(G::initial())),
            mutable,
        }
    }

    /// Returns whether the global is thread-safe.
    pub fn sync(&self) -> bool { matches!(&self.initial, GlobalInitial::Sync(..)) }
}

#[derive(Clone, Copy)]
pub(crate) struct ArchetypeDescriptor {
    pub(crate) id:      DbgTypeId,
    pub(crate) builder: fn() -> (ealloc::AnyBuilder, Box<dyn world::typed::AnyBuilder>),
}

impl ArchetypeDescriptor {
    fn of<A: Archetype>() -> Self {
        Self {
            id:      DbgTypeId::of::<A>(),
            builder: || (ealloc::builder::<A>(), Box::new(world::typed::builder::<A>())),
        }
    }
}

/// Indicates that the system requires a simple component read/write.
pub struct SimpleRequest {
    /// The archetype requested.
    pub(crate) arch:            ArchetypeDescriptor,
    /// The type of the simple component.
    pub(crate) comp:            DbgTypeId,
    /// Builder for the storage. Must be `Box<storage::SharedSimple<A>>`.
    pub(crate) storage_builder: fn() -> Box<dyn Any>,
    /// Whether mutable access is requested.
    pub(crate) mutable:         bool,
}

impl SimpleRequest {
    /// Creates a new simple component request with types known at compile time.
    pub fn new<A: Archetype, C: comp::Simple<A>>(mutable: bool) -> Self {
        Self {
            arch: ArchetypeDescriptor::of::<A>(),
            comp: DbgTypeId::of::<C>(),
            mutable,
            storage_builder: || Box::new(storage::Simple::<A>::new::<C>()),
        }
    }
}

/// Indicates that the system requires an isotope component read/write.
pub struct IsotopeRequest {
    /// The archetype requested.
    pub(crate) arch:            ArchetypeDescriptor,
    /// The archetype of the isotope component.
    pub(crate) comp:            DbgTypeId,
    /// Builder for the IsotopeFactory. Must be `Box<Box<dyn storage::AnyIsotopeFactory<A>>>`.
    pub(crate) factory_builder: fn() -> Box<dyn Any>,
    /// If `Some`, only the isotope components of the given discriminants are accessible.
    ///
    /// This will not lead to creation of the discriminant storages.
    pub(crate) discrim:         Option<Vec<usize>>,
    /// Whether mutable access is requested.
    pub(crate) mutable:         bool,
}

impl IsotopeRequest {
    /// Creates a new isotope component request with types known at compile time.
    pub fn new<A: Archetype, C: comp::Isotope<A>>(
        discrim: Option<Vec<usize>>,
        mutable: bool,
    ) -> Self {
        Self {
            arch: ArchetypeDescriptor::of::<A>(),
            comp: DbgTypeId::of::<C>(),
            discrim,
            mutable,
            factory_builder: || Box::new(storage::IsotopeFactory::<A>::new::<C>()),
        }
    }
}
