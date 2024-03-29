//! Specifies the requirements for a system.

use std::any::{self, Any, TypeId};
use std::collections::HashSet;

use crate::entity::{ealloc, referrer};
use crate::util::DbgTypeId;
use crate::{comp, storage, system, world, Archetype, Global};

/// Describes an instance of system.
pub struct Spec {
    /// The debug name of the system.
    pub debug_name:              String,
    /// The partition dependencies related to the system.
    pub dependencies:            Vec<Dependency>,
    /// The global states requested by the system.
    pub global_requests:         Vec<GlobalRequest>,
    /// The simple components requested by the system.
    pub simple_requests:         Vec<SimpleRequest>,
    /// The isotope components requested by the system.
    pub isotope_requests:        Vec<IsotopeRequest>,
    /// The archetypes of which entities may be created.
    pub entity_creator_requests: Vec<EntityCreatorRequest>,
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
    pub(crate) ty:          DbgTypeId,
    /// The referrer vtable of the type.
    pub(crate) vtable:      referrer::SingleVtable,
    /// A closure that calls [`Global::initial`].
    pub(crate) initial:     GlobalInitial,
    /// Whether mutable access is requested.
    pub(crate) mutable:     bool,
    /// The list of strongly referenced archetypes that must be initialized.
    pub(crate) strong_refs: HashSet<DbgTypeId>,
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
        let mut visitor = referrer::VisitTypeArg::new();
        G::visit_type(&mut visitor);

        Self {
            ty: DbgTypeId::of::<G>(),
            vtable: referrer::SingleVtable::of::<G>(),
            initial: GlobalInitial::Sync(|| Box::new(G::initial())),
            mutable,
            strong_refs: visitor.found_archs,
        }
    }

    /// Creates a new thread-unsafe global state request with types known at compile time.
    pub fn new_unsync<G: Global>(mutable: bool) -> Self {
        let mut visitor = referrer::VisitTypeArg::new();
        G::visit_type(&mut visitor);

        Self {
            ty: DbgTypeId::of::<G>(),
            vtable: referrer::SingleVtable::of::<G>(),
            initial: GlobalInitial::Unsync(|| Box::new(G::initial())),
            mutable,
            strong_refs: visitor.found_archs,
        }
    }

    /// Asserts that strong references of `A` used in a system
    /// are not strictly required to be initialized.
    pub fn maybe_uninit<A: Archetype>(mut self) -> Self {
        let present = self.strong_refs.remove(&TypeId::of::<A>());
        if !present {
            panic!("No strong references to `{}` detected in `{}`", any::type_name::<A>(), self.ty);
        }
        self
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
    /// The list of strongly referenced archetypes that must be initialized.
    pub(crate) strong_refs:     HashSet<DbgTypeId>,
}

impl SimpleRequest {
    /// Creates a new simple component request with types known at compile time.
    pub fn new<A: Archetype, C: comp::Simple<A>>(mutable: bool) -> Self {
        let mut visitor = referrer::VisitTypeArg::new();
        C::visit_type(&mut visitor);

        Self {
            arch: ArchetypeDescriptor::of::<A>(),
            comp: DbgTypeId::of::<C>(),
            mutable,
            storage_builder: storage::simple::builder::<A, C> as fn() -> Box<dyn Any>,
            strong_refs: visitor.found_archs,
        }
    }

    /// Asserts that strong references of `A` used in a system
    /// are not strictly required to be initialized.
    pub fn maybe_uninit<A: Archetype>(mut self) -> Self {
        let present = self.strong_refs.remove(&TypeId::of::<A>());
        if !present {
            panic!(
                "No strong references to `{}` detected in `{}`",
                any::type_name::<A>(),
                self.comp
            );
        }
        self
    }
}

/// Indicates that the system requires an isotope component read/write.
pub struct IsotopeRequest {
    /// The archetype requested.
    pub(crate) arch:        ArchetypeDescriptor,
    /// The archetype of the isotope component.
    pub(crate) comp:        DbgTypeId,
    /// Builder for the IsotopeFactory. Downcasts to `Box<Arc<dyn storage::AnyIsotopeMap<A>>>`.
    pub(crate) map_builder: fn() -> Box<dyn Any>,
    /// If `Some`, only the isotope components of the given discriminants are accessible.
    ///
    /// This will not lead to creation of the discriminant storages.
    pub(crate) discrim:     Option<Vec<usize>>,
    /// Whether mutable access is requested.
    pub(crate) mutable:     bool,
    /// The list of strongly referenced archetypes that must be initialized.
    pub(crate) strong_refs: HashSet<DbgTypeId>,
}

impl IsotopeRequest {
    /// Creates a new isotope component request with types known at compile time.
    pub fn new<A: Archetype, C: comp::Isotope<A>>(
        discrim: Option<Vec<usize>>,
        mutable: bool,
    ) -> Self {
        let mut visitor = referrer::VisitTypeArg::new();
        C::visit_type(&mut visitor);

        Self {
            arch: ArchetypeDescriptor::of::<A>(),
            comp: DbgTypeId::of::<C>(),
            discrim,
            mutable,
            map_builder: || Box::new(storage::IsotopeMap::<A, C>::new_any()),
            strong_refs: visitor.found_archs,
        }
    }

    /// Asserts that strong references of `A` used in a system
    /// are not strictly required to be initialized.
    pub fn maybe_uninit<A: Archetype>(mut self) -> Self {
        let present = self.strong_refs.remove(&TypeId::of::<A>());
        if !present {
            panic!(
                "No strong references to `{}` detected in `{}`",
                any::type_name::<A>(),
                self.comp
            );
        }
        self
    }
}

/// Indicates that the system may create entities for a particular archetype.
pub struct EntityCreatorRequest {
    /// The archetype requested.
    pub(crate) arch:         DbgTypeId,
    /// Partition dependency is disabled.
    pub(crate) no_partition: bool,
}

impl EntityCreatorRequest {
    /// Creates a new entity creator request with type known at compile time.
    pub fn new<A: Archetype>() -> Self { Self { arch: DbgTypeId::of::<A>(), no_partition: false } }

    /// Do not add [`EntityCreationPartition`](system::EntityCreationPartition) dependency for this system.
    pub fn no_partition(self) -> Self { Self { no_partition: true, ..self } }
}
