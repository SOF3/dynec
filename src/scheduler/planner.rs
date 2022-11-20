use std::collections::{BTreeSet, HashMap};
use std::num::NonZeroUsize;

use parking_lot::Condvar;

use super::executor::DeadlockCounter;
use super::{Node, SendSystemIndex, Topology, UnsendSystemIndex, WakeupState};
use crate::tracer::{self, Tracer};

/// Stores the tick-local state for schedule availability.
#[derive(Debug, Clone)]
pub(crate) struct Planner {
    /// Stores the number of nodes blocking each node from getting scheduled.
    /// Started nodes are not removed from the map and remain as 0.
    /// Non-started nodes with count 0 may get incremented if an exclusion starts.
    pub(crate) wakeup_state: HashMap<Node, WakeupState>,

    /// The queue of [`Node::SendSystem`] nodes that may be runnable.
    /// Due to exclusion, nodes in the queue may no longer be runnable.
    /// `wakeup_count` must always be re-checked.
    pub(crate) send_runnable: BTreeSet<SendSystemIndex>,

    /// The queue of [`Node::UnsendSystem`] nodes that may be runnable.
    /// Due to exclusion, nodes in the queue may no longer be runnable.
    /// `wakeup_count` must always be re-checked.
    pub(crate) unsend_runnable: BTreeSet<UnsendSystemIndex>,

    /// Number of remaining systems to run.
    pub(crate) remaining_systems: usize,
}

impl Planner {
    /// Steal a task from the pending pool if any is available
    fn steal<I: Eq + Ord + Copy>(
        &mut self,
        tracer: &impl Tracer,
        thread: tracer::Thread,
        topology: &Topology,
        pool: fn(&mut Self) -> &mut BTreeSet<I>,
        to_node: fn(I) -> Node,
    ) -> StealResult<I> {
        if self.remaining_systems == 0 {
            tracer.steal_return_complete(thread);
            return StealResult::CycleComplete;
        }

        let index = match pool(self).pop_first() {
            Some(index) => index,
            None => {
                tracer.steal_return_pending(thread);
                return StealResult::Pending;
            }
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
                    tracer.unmark_runnable(excl);
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

    pub(crate) fn steal_send(
        &mut self,
        tracer: &impl Tracer,
        thread: tracer::Thread,
        topology: &Topology,
    ) -> StealResult<SendSystemIndex> {
        self.steal(tracer, thread, topology, |this| &mut this.send_runnable, Node::SendSystem)
    }

    pub(crate) fn steal_unsend(
        &mut self,
        tracer: &impl Tracer,
        thread: tracer::Thread,
        topology: &Topology,
    ) -> StealResult<UnsendSystemIndex> {
        self.steal(tracer, thread, topology, |this| &mut this.unsend_runnable, Node::UnsendSystem)
    }

    /// Mark a node as completed.
    ///
    /// This method is only called for system nodes.
    /// Partition nodes are completed in-place.
    pub(crate) fn complete(
        &mut self,
        tracer: &impl Tracer,
        node: Node,
        topology: &Topology,
        condvar: &Condvar,
        deadlock_counter: &DeadlockCounter,
    ) {
        {
            let state = self.wakeup_state.get_mut(&node).expect("invalid node index");
            match state {
                WakeupState::Started => *state = WakeupState::Completed,
                _ => panic!("cannot mark a {state:?} node as completed"),
            }
        }

        self.remove_one_block(tracer, topology, topology.dependents_of(node).iter().copied());
        self.remove_one_block(tracer, topology, topology.exclusions_of(node).iter().copied());

        self.remaining_systems -= 1;

        tracer.complete_system(node, self.remaining_systems);

        let wakeups = condvar.notify_all();
        deadlock_counter.end_wait(wakeups);
    }

    /// Removes one blocker from each node in the queue iterator.
    fn remove_one_block(
        &mut self,
        tracer: &impl Tracer,
        topology: &Topology,
        queue: impl Iterator<Item = Node>,
    ) {
        let mut queue: Vec<Node> = queue.collect();
        while let Some(node) = queue.pop() {
            self.remove_one_block_no_recursion(tracer, node, topology, &mut queue);
        }
    }

    /// Removes one blocker count from a node wakeup state
    fn remove_one_block_no_recursion(
        &mut self,
        tracer: &impl Tracer,
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
                        tracer.mark_runnable(node);
                    }
                    Node::UnsendSystem(index) => {
                        let new = self.unsend_runnable.insert(index);
                        if !new {
                            panic!("Blocked node {node:?} is already in runnable pool")
                        }
                        tracer.mark_runnable(node);
                    }
                    Node::Partition(index) => {
                        *state = WakeupState::Completed;
                        tracer.partition(
                            node,
                            &*topology.partitions.get(index.0).expect("invalid node index").0,
                        );
                        queue.extend(topology.dependents_of(node).iter().copied())
                    }
                }
            }
            WakeupState::Completed => {} // no exclusion for completed nodes
            state => panic!("Node {node:?} is in state {state:?} which should not have blockers"),
        }
    }
}

#[derive(Debug)]
pub(crate) enum StealResult<I> {
    Ready(I),
    Pending,
    CycleComplete,
}
