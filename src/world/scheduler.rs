use std::fmt;
use std::num::NonZeroUsize;

use crate::util::DbgTypeId;
use crate::world;

mod builder;
pub(crate) use builder::Builder;

mod executor;
use executor::Executor;

mod planner;
use parking_lot::Mutex;
use planner::Planner;

mod state;
use state::{SyncState, UnsyncState};

mod topology;
use topology::Topology;

#[cfg(test)]
mod tests;

pub(crate) struct Scheduler {
    topology:     Topology,
    planner:      Mutex<Planner>,
    sync_state:   SyncState,
    unsync_state: UnsyncState,
    executor:     Executor,
}

impl Scheduler {
    pub(in crate::world) fn execute(
        &mut self,
        tracer: &impl world::Tracer,
        components: &world::Components,
        sync_globals: &world::SyncGlobals,
        unsync_globals: &mut world::UnsyncGlobals,
    ) {
        self.executor.execute_full_cycle(
            tracer,
            &self.topology,
            &mut self.planner,
            SendArgs { state: &self.sync_state, components, globals: sync_globals },
            UnsendArgs { state: &mut self.unsync_state, globals: unsync_globals },
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum WakeupState {
    /// The node is runnable after being awaken by `count` other nodes.
    Blocked { count: NonZeroUsize },
    /// The node is in the planner queue.
    Pending,
    /// The node is scheduled on one of the threads.
    Started,
    /// The node has already completed.
    Completed,
}

impl WakeupState {
    fn increment(&mut self) {
        match self {
            Self::Blocked { count } => {
                *count = NonZeroUsize::new(count.get() + 1).expect("integer overflow")
            }
            state @ Self::Pending => {
                *state = Self::Blocked { count: NonZeroUsize::new(1).expect("1 != 0") }
            }
            Self::Started | Self::Completed => panic!("Cannot increment started node"),
        }
    }
}

/// Identifies a topological node in the schedule of a cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Node {
    /// The execution span of a thread-safe system.
    SendSystem(SendSystemIndex),
    /// The execution span of a thread-unsafe system.
    UnsendSystem(UnsendSystemIndex),
    /// The instant between systems before and after a partition.
    Partition(PartitionIndex),
}

/// Uniquely identifies a [`system::Sendable`](crate::system::Sendable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SendSystemIndex(usize);

/// Uniquely identifies a [`system::Unsendable`](crate::system::Unsendable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnsendSystemIndex(usize);

/// Uniquely identifies a [`system::Partition`](crate::system::Partition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartitionIndex(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ResourceType {
    Global(DbgTypeId),
    Simple { arch: DbgTypeId, comp: DbgTypeId },
    Isotope { arch: DbgTypeId, comp: DbgTypeId },
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Global(ty) => writeln!(f, "global state {ty}"),
            Self::Simple { arch, comp } => writeln!(f, "simple component {arch}/{comp}"),
            Self::Isotope { arch, comp } => writeln!(f, "isotope component {arch}/{comp}"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResourceAccess {
    mutable: bool,
    discrim: Option<Vec<usize>>,
}

impl ResourceAccess {
    pub(crate) fn new(mutable: bool) -> Self { Self { mutable, discrim: None } }

    pub(crate) fn with_discrim(mutable: bool, discrim: Option<Vec<usize>>) -> Self {
        Self { mutable, discrim }
    }

    fn check_conflicts_with(&self, other: &Self) -> Result<(), String> {
        if !self.mutable && !other.mutable {
            return Ok(());
        }

        // ensure that `this` requests unique access to generate correct error message
        let (this, that, that_mut) = match self.mutable {
            true => (&self.discrim, &other.discrim, other.mutable),
            false => (&other.discrim, &self.discrim, self.mutable),
        };
        let that_mut_str = match that_mut {
            true => "unique",
            false => "shared",
        };

        match (this, that) {
            (Some(this), Some(that)) => {
                if let Some(discrim) = this.iter().find(|&discrim| that.contains(discrim)) {
                    Err(format!(
                        "unique access to discriminant {discrim:?} requested but {that_mut_str} \
                         access is requested again in the same system; multiple isotope component \
                         requests on the same type are only allowed when they do not overlapped"
                    ))
                } else {
                    Ok(())
                }
            }
            (Some(this), None) => Err(format!(
                "unique access to discriminants {this:?} requested but {that_mut_str} access is \
                 requested again for the same isotope component in the same system; multiple \
                 isotope component requests on the same type are only allowed when they do not \
                 overlapped"
            )),
            (None, Some(that)) => Err(format!(
                "unique access to all discriminants is requested but {that_mut_str} access is \
                 requested again for the same isotope component in the same system; multiple \
                 isotope component requests on the same type are only allowed when they do not \
                 overlapped"
            )),
            (None, None) => Err(format!(
                "unique access is requested but {that_mut_str} access is requested again in the \
                 same system"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Order {
    before: Node,
    after:  Node,
}

#[derive(Clone, Copy)]
struct SendArgs<'t> {
    state:      &'t SyncState,
    components: &'t world::Components,
    globals:    &'t world::SyncGlobals,
}

struct UnsendArgs<'t> {
    state:   &'t mut UnsyncState,
    globals: &'t mut world::UnsyncGlobals,
}
