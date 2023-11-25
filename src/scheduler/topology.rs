use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::num::NonZeroUsize;

use super::{
    Node, Order, PartitionIndex, Planner, ResourceAccess, ResourceType, SendSystemIndex,
    UnsendSystemIndex, WakeupState,
};
use crate::system;

/// Stores the topology of the schedule,
/// including the dependency and exclusion relationship.
#[derive(Debug)]
pub(crate) struct Topology {
    /// If `dependents[a].contains(b)`, `b` depends on `a`.
    /// This means `b` is a wakeup candidate when `a` completes.
    dependents: HashMap<Node, Vec<Node>>,

    /// The [`Planner`] reset state every tick.
    initial_planner: Planner,

    /// The list of partitions without dependencies.
    ///
    /// This field is persisted for tracing.
    pub(crate) depless_pars: Vec<PartitionIndex>,

    /// The indexable list of partitions.
    ///
    /// This field is persisted for tracing.
    pub(crate) partitions: Vec<system::partition::Wrapper>,

    /// If `exclusions[a].contains(b)`, `a` and `b` must not execute concurrently.
    /// `exclusions[a].contains(b)` if and only if `exclusions[b].contains(a)`.
    exclusions: HashMap<Node, Vec<Node>>,
}

impl Topology {
    pub(crate) fn init(
        send_systems_count: usize,
        unsend_systems_count: usize,
        partitions: &[&system::partition::Wrapper],
        orders: &[Order],
        resources: &HashMap<ResourceType, HashMap<Node, Vec<ResourceAccess>>>,
        describe_node: impl Fn(Node) -> String,
    ) -> Self {
        let nodes_iter = (0..send_systems_count)
            .map(|index| Node::SendSystem(SendSystemIndex(index)))
            .chain(
                (0..unsend_systems_count).map(|index| Node::UnsendSystem(UnsendSystemIndex(index))),
            )
            .chain((0..partitions.len()).map(|index| Node::Partition(PartitionIndex(index))));

        let dependents = build_dependents_map(nodes_iter.clone(), orders.iter().copied());
        scan_cycles(&dependents, describe_node);
        let (initial_planner, depless_pars) =
            build_initials(nodes_iter.clone(), orders.iter().copied(), &dependents);

        let exclusions = build_exclusions(nodes_iter, resources);

        Self { dependents, initial_planner, depless_pars, partitions: Vec::new(), exclusions }
    }

    pub(crate) fn dependents_of(&self, node: Node) -> &[Node] {
        self.dependents.get(&node).expect("invalid node index")
    }

    pub(crate) fn exclusions_of(&self, node: Node) -> &[Node] {
        self.exclusions.get(&node).expect("invalid node index")
    }

    pub(crate) fn initial_planner(&self) -> &Planner { &self.initial_planner }
}

fn build_dependents_map(
    nodes: impl Iterator<Item = Node>,
    orders: impl Iterator<Item = Order>,
) -> HashMap<Node, Vec<Node>> {
    let mut dependents: HashMap<Node, Vec<Node>> = nodes.map(|node| (node, Vec::new())).collect();
    for order in orders {
        dependents.get_mut(&order.before).expect("invalid node index").push(order.after);
    }
    dependents
}

fn scan_cycles(map: &HashMap<Node, Vec<Node>>, describe_node: impl Fn(Node) -> String) {
    let mut remaining: BTreeSet<Node> = map.keys().copied().collect();
    let mut exited = HashSet::new();
    let mut stack = Vec::new();

    while let Some(&node) = remaining.iter().next() {
        scan_cycles_from(map, node, &mut remaining, &mut exited, &mut stack, &describe_node);
    }
}

fn scan_cycles_from(
    map: &HashMap<Node, Vec<Node>>,
    node: Node,
    remaining: &mut BTreeSet<Node>,
    exited: &mut HashSet<Node>,
    stack: &mut Vec<Node>,
    describe_node: &impl Fn(Node) -> String,
) {
    if exited.contains(&node) {
        return; // already scanned
    }

    if !remaining.remove(&node) {
        use fmt::Write;

        let mut panic_message = String::new();

        for &ancestor in stack.iter().skip_while(|&&ancestor| ancestor != node) {
            write!(panic_message, "{} -> ", describe_node(ancestor))
                .expect("String write is infallible");
        }

        write!(panic_message, "{}", describe_node(node)).expect("String write is infallible");

        panic!("Scheduled systems have a cyclic dependency: {}", panic_message);
    }

    stack.push(node);

    for &dependent in map.get(&node).expect("invalid node index") {
        scan_cycles_from(map, dependent, remaining, exited, stack, describe_node);
    }

    let popped = stack.pop();
    debug_assert_eq!(Some(node), popped);

    let new_exit = exited.insert(node);
    assert!(new_exit, "exited is inserted recursively but no cycles were detected");
}

fn build_initials(
    nodes: impl Iterator<Item = Node> + Clone,
    orders: impl Iterator<Item = Order>,
    dependents: &HashMap<Node, Vec<Node>>,
) -> (Planner, Vec<PartitionIndex>) {
    let mut dependency_counts: HashMap<Node, usize> = nodes.clone().map(|node| (node, 0)).collect();
    for order in orders {
        *dependency_counts.get_mut(&order.after).expect("invalid node index") += 1;
    }

    // trim dependencyless partitions
    let mut depless_pars: Vec<PartitionIndex> = dependency_counts
        .iter()
        .filter_map(|tuple| match tuple {
            (&Node::Partition(index), &0) => Some(index),
            _ => None,
        })
        .collect();

    while let Some(par) = depless_pars.pop() {
        for &dependent in dependents.get(&Node::Partition(par)).expect("invalid node index") {
            let count = dependency_counts.get_mut(&dependent).expect("invalid node index");
            *count = count
                .checked_sub(1)
                .expect("dependent of partition should not have zero dependency count");

            if let Node::Partition(index) = dependent {
                depless_pars.push(index);
            }
        }
    }

    // nominate dependencyless systems into the runnable pool
    let send_runnable: BTreeSet<SendSystemIndex> = dependency_counts
        .iter()
        .filter_map(|entry| match entry {
            (&Node::SendSystem(index), 0) => Some(index),
            _ => None,
        })
        .collect();
    let unsend_runnable: BTreeSet<UnsendSystemIndex> = dependency_counts
        .iter()
        .filter_map(|entry| match entry {
            (&Node::UnsendSystem(index), 0) => Some(index),
            _ => None,
        })
        .collect();

    let wakeup_state: HashMap<Node, WakeupState> = dependency_counts
        .into_iter()
        .map(|entry| match entry {
            (node, count @ 1..) => (
                node,
                WakeupState::Blocked { count: NonZeroUsize::new(count).expect("count @ 1..") },
            ),
            (node @ (Node::SendSystem(_) | Node::UnsendSystem(_)), 0) => {
                (node, WakeupState::Pending)
            }
            (node @ Node::Partition(_), 0) => (node, WakeupState::Completed),
        })
        .collect();

    let remaining_systems =
        nodes.filter(|node| matches!(node, Node::SendSystem(_) | Node::UnsendSystem(_))).count();

    (Planner { wakeup_state, send_runnable, unsend_runnable, remaining_systems }, depless_pars)
}

fn build_exclusions(
    nodes: impl Iterator<Item = Node>,
    resources: &HashMap<ResourceType, HashMap<Node, Vec<ResourceAccess>>>,
) -> HashMap<Node, Vec<Node>> {
    let mut exclusions: HashMap<Node, HashSet<Node>> =
        nodes.map(|node| (node, HashSet::new())).collect();

    for nodes in resources.values() {
        for (&node1, accesses1) in nodes {
            for (&node2, accesses2) in nodes {
                if node1 == node2 {
                    continue;
                }

                if accesses1.iter().any(|access1| {
                    accesses2.iter().any(|access2| access1.check_conflicts_with(access2).is_err())
                }) {
                    exclusions.get_mut(&node1).expect("invalid node index").insert(node2);
                }
            }
        }
    }

    exclusions.into_iter().map(|(node, set)| (node, set.into_iter().collect())).collect()
}
