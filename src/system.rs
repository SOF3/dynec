//! Systems are actions performed every frame to manipulate entities and components.
//!
//! A system can declare mutable or immutable access
//! to archetype-specific components and global states.
//! dynec will schedule systems such that systems have unique access to mutable resources they
//! request.
//! Furthermore, systems can request relative execution order through [`Partition`]s.
//! Systems that use thread-unsafe resources (systems that are not [`Send`])
//! are always executed on the main thread.

use crate::entity::{ealloc, referrer};
use crate::world;
use crate::world::offline;

mod accessor;
pub(crate) use accessor::BorrowedOwned;
pub use accessor::{
    IsotopeMutMap, ReadIsotope, ReadSimple, RefOrDefault, WriteIsotope, WriteSimple,
};

pub mod partition;
pub use partition::{EntityCreationPartition, Partition};

mod entity;
pub use entity::{EntityCreator, EntityDeleter};
#[doc(hidden)]
pub use entity::{EntityCreatorImpl, EntityDeleterImpl};

pub mod spec;
#[doc(inline)]
pub use spec::Spec;

pub trait Descriptor {
    /// Describes this instance of system.
    ///
    /// The method is only called when the system was initially scheduled,
    /// but it should return a consistent value.
    fn get_spec(&self) -> Spec;

    /// Delegates to [`Referrer::visit_type`](referrer::Referrer::visit_type)
    /// for the states of this system.
    fn visit_type(&self, arg: &mut referrer::VisitTypeArg);

    /// Executes the mutable visitor
    fn visit_mut(&mut self) -> referrer::AsObject<'_>;
}

/// A system requests some resources, stores some states of its own and is runnable with the
/// requested resources.
///
/// There may be multiple instances of the same implementor type.
/// This is meaningful as they may have different states.
pub trait Sendable: Send + Descriptor {
    /// Runs the system.
    fn run(
        &mut self,
        globals: &world::SyncGlobals,
        components: &world::Components,
        ealloc_shard_map: &mut ealloc::ShardMap,
        offline_shard: &mut offline::BufferShard,
    );

    /// Returns self upcast to [`Descriptor`] as a trait object.
    fn as_descriptor_mut(&mut self) -> &mut dyn Descriptor;
}

/// A variant of [`Sendable`] that runs on the main thread only,
/// but allows storing [`Send`] states
/// and accessing non-<code>[Send] + [Sync]</code> global states.
pub trait Unsendable: Descriptor {
    /// Runs the system.
    fn run(
        &mut self,
        sync_globals: &world::SyncGlobals,
        unsync_globals: &mut world::UnsyncGlobals,
        components: &world::Components,
        ealloc_shard_map: &mut ealloc::ShardMap,
        offline_shard: &mut offline::BufferShard,
    );

    /// Returns self upcast to [`Descriptor`] as a trait object.
    fn as_descriptor_mut(&mut self) -> &mut dyn Descriptor;
}
