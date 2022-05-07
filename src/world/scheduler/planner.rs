use std::collections::{BTreeSet, HashMap};
use std::num::NonZeroUsize;

use parking_lot::Condvar;

use super::{Node, SendSystemIndex, Topology, UnsendSystemIndex, WakeupState};
use crate::util;

/// Stores the tick-local state for schedule availability.
#[derive(Clone)]
pub(in crate::world::scheduler) struct Planner {
    /// Stores the number of nodes blocking each node from getting scheduled.
    /// Started nodes are not removed from the map and remain as 0.
    /// Non-started nodes with count 0 may get incremented if an exclusion starts.
    pub(in crate::world::scheduler) wakeup_state: HashMap<Node, WakeupState>,

    /// The queue of [`Node::SendSystem`] nodes that may be runnable.
    /// Due to exclusion, nodes in the queue may no longer be runnable.
    /// `wakeup_count` must always be re-checked.
    pub(in crate::world::scheduler) send_runnable: BTreeSet<SendSystemIndex>,

    /// The queue of [`Node::UnsendSystem`] nodes that may be runnable.
    /// Due to exclusion, nodes in the queue may no longer be runnable.
    /// `wakeup_count` must always be re-checked.
    pub(in crate::world::scheduler) unsend_runnable: BTreeSet<UnsendSystemIndex>,

    /// Whether the planner is complete.
    pub(in crate::world::scheduler) is_complete: bool,
}

impl Planner {
    /// Steal a task from the pending pool if any is available
    fn steal<I: Eq + Ord + Copy>(
        &mut self,
        topology: &Topology,
        pool: fn(&mut Self) -> &mut BTreeSet<I>,
        to_node: fn(I) -> Node,
    ) -> StealResult<I> {
        if self.is_complete {
            return StealResult::CycleComplete;
        }

        let index = match util::btreeset_remove_first(pool(self)) {
            Some(index) => index,
            None => return StealResult::Pending,
        };
        let node = to_node(index);

        // mark node as started
        {
            let state = self.wakeup_state.get_mut(&node).expect("invalid node index");
            match state {
                WakeupState::Pending => *state = WakeupState::Started,
                _ => panic!(
                    "node {state:?} is in runnable queue but state is {node:?} instead of Pending"
                ),
            }
        }

        // starting a node has no effect on its dependencies and dependents

        // increment the block counter of exclusive nodes
        for &excl in topology.exclusions_of(node) {
            let state = self.wakeup_state.get_mut(&excl).expect("invalid node index");
            match state {
                WakeupState::Pending => {
                    *state = WakeupState::Blocked { count: NonZeroUsize::new(1).expect("1 != 0") };
                    match excl {
                        Node::SendSystem(index) => {
                            self.send_runnable
                                .take(&index)
                                .expect("Pending node should be in runnable pool");
                        }
                        Node::UnsendSystem(index) => {
                            self.unsend_runnable
                                .take(&index)
                                .expect("Pending node should be in runnable pool");
                        }
                        Node::Partition(_) => {
                            panic!("partitions are not exclusive with other nodes")
                        }
                    }
                }
                WakeupState::Blocked { count } => {
                    *count = NonZeroUsize::new(count.get() + 1).expect("integer overflow");
                }
                WakeupState::Started => {
                    panic!("Started node {excl:?} should not be in the runnable pool")
                }
                WakeupState::Completed => {} // We don't care about completed nodes
            }
        }

        StealResult::Ready(index)
    }

    pub(in crate::world::scheduler) fn steal_send(
        &mut self,
        topology: &Topology,
    ) -> StealResult<SendSystemIndex> {
        self.steal(topology, |this| &mut this.send_runnable, Node::SendSystem)
    }

    pub(in crate::world::scheduler) fn steal_unsend(
        &mut self,
        topology: &Topology,
    ) -> StealResult<UnsendSystemIndex> {
        self.steal(topology, |this| &mut this.unsend_runnable, Node::UnsendSystem)
    }

    /// Mark a node as completed.
    pub(in crate::world::scheduler) fn complete(
        &mut self,
        node: Node,
        topology: &Topology,
        condvar: &Condvar,
    ) {
        {
            let state = self.wakeup_state.get_mut(&node).expect("invalid node index");
            match state {
                WakeupState::Started => *state = WakeupState::Completed,
                _ => panic!("cannot mark a {state:?} node as completed"),
            }
        }

        self.remove_one_block(topology, topology.dependents_of(node).iter().copied());

        condvar.notify_all();
    }

    fn remove_one_block(&mut self, topology: &Topology, queue: impl Iterator<Item = Node>) {
        let mut queue: Vec<Node> = queue.collect();
        while let Some(node) = queue.pop() {
            self.remove_one_block_no_recursion(node, topology, &mut queue);
        }
    }

    /// Removes one blocker count from a node wakeup state
    fn remove_one_block_no_recursion(
        &mut self,
        node: Node,
        topology: &Topology,
        queue: &mut Vec<Node>,
    ) {
        let state = self.wakeup_state.get_mut(&node).expect("invalid node index");
        match state {
            WakeupState::Blocked { count } if count.get() > 1 => {
                *count = NonZeroUsize::new(count.get() - 1).expect("count - 1 > 1 - 1 = 0")
            }
            WakeupState::Blocked { count } if count.get() == 1 => {
                *state = WakeupState::Pending;
                match node {
                    Node::SendSystem(index) => {
                        let new = self.send_runnable.insert(index);
                        if !new {
                            panic!("Blocked node {node:?} is already in runnable pool")
                        }
                    }
                    Node::UnsendSystem(index) => {
                        let new = self.unsend_runnable.insert(index);
                        if !new {
                            panic!("Blocked node {node:?} is already in runnable pool")
                        }
                    }
                    Node::Partition(index) => {
                        *state = WakeupState::Completed;
                        queue.extend(topology.dependents_of(node).iter().copied())
                    }
                }
            }
            _ => panic!("Node {node:?} is in state {state:?} which should not have blockers"),
        }
    }
}

pub(in crate::world::scheduler) enum StealResult<I> {
    Ready(I),
    Pending,
    CycleComplete,
}
