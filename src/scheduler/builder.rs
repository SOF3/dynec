use std::collections::{hash_map, HashMap};
use std::fmt;

use indexmap::IndexSet;
use parking_lot::Mutex;

use super::{
    Executor, Node, Order, PartitionIndex, ResourceAccess, ResourceType, Scheduler,
    SendSystemIndex, SyncState, Topology, UnsendSystemIndex, UnsyncState,
};
use crate::system::{self, spec};

pub(crate) struct Builder {
    pub(crate) concurrency: usize,
    send_systems:           Vec<(String, Box<dyn system::Sendable>)>,
    unsend_systems:         Vec<(String, Box<dyn system::Unsendable>)>,
    partitions:             IndexSet<system::partition::Wrapper>,
    resources:              HashMap<ResourceType, HashMap<Node, Vec<ResourceAccess>>>,
    orders:                 Vec<Order>,
}

impl Builder {
    pub(crate) fn new(concurrency: usize) -> Self {
        Self {
            concurrency,
            send_systems: Vec::new(),
            unsend_systems: Vec::new(),
            partitions: IndexSet::new(),
            resources: HashMap::new(),
            orders: Vec::new(),
        }
    }

    pub(crate) fn push_send_system(
        &mut self,
        sys: Box<dyn system::Sendable>,
    ) -> (Node, system::Spec) {
        let spec = sys.get_spec();
        let index = SendSystemIndex(self.send_systems.len());
        self.send_systems.push((spec.debug_name.clone(), sys));
        (Node::SendSystem(index), spec)
    }

    pub(crate) fn push_unsend_system(
        &mut self,
        sys: Box<dyn system::Unsendable>,
    ) -> (Node, system::Spec) {
        let spec = sys.get_spec();
        let index = UnsendSystemIndex(self.unsend_systems.len());
        self.unsend_systems.push((spec.debug_name.clone(), sys));
        (Node::UnsendSystem(index), spec)
    }

    pub(crate) fn push_partition(
        &mut self,
        par: system::partition::Wrapper,
    ) -> (Node, &system::partition::Wrapper) {
        let (index, _is_new) = self.partitions.insert_full(par);
        (
            Node::Partition(PartitionIndex(index)),
            self.partitions.get_index(index).expect("value returned by insert_full"),
        )
    }

    pub(crate) fn use_resource(&mut self, node: Node, ty: ResourceType, access: ResourceAccess) {
        match self.resources.entry(ty).or_default().entry(node) {
            hash_map::Entry::Vacant(entry) => {
                entry.insert(vec![access]);
            }
            hash_map::Entry::Occupied(mut entry) => {
                for other in entry.get() {
                    if let Err(err) = access.check_conflicts_with(other) {
                        panic!(
                            "Cannot schedule {} due to conflicts in {} access: {}",
                            self.display_node(node),
                            ty,
                            err
                        );
                    }
                }
                entry.get_mut().push(access);
            }
        }
    }

    pub(crate) fn add_dependencies(&mut self, deps: Vec<spec::Dependency>, system_node: Node) {
        for dep in deps {
            match dep {
                spec::Dependency::Before(partition) => {
                    let (partition_node, _) =
                        self.push_partition(system::partition::Wrapper(partition));
                    self.add_dependency(system_node, partition_node);
                }
                spec::Dependency::After(partition) => {
                    let (partition_node, _) =
                        self.push_partition(system::partition::Wrapper(partition));
                    self.add_dependency(partition_node, system_node);
                }
            }
        }
    }

    fn display_node(&self, node: Node) -> impl fmt::Display + '_ {
        struct Ret<'t>(&'t Builder, Node);

        impl<'t> fmt::Display for Ret<'t> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.1 {
                    Node::SendSystem(index) => {
                        let (debug_name, _) =
                            self.0.send_systems.get(index.0).expect("invalid node index");
                        write!(f, "thread-safe system #{} ({})", index.0, debug_name)
                    }
                    Node::UnsendSystem(index) => {
                        let (debug_name, _) =
                            self.0.unsend_systems.get(index.0).expect("invalid node index");
                        write!(f, "thread-unsafe system #{} ({})", index.0, debug_name)
                    }
                    Node::Partition(index) => {
                        let par = self.0.partitions.get_index(index.0).expect("invalid node index");
                        write!(f, "partition #{} ({:?})", index.0, &par)
                    }
                }
            }
        }

        Ret(self, node)
    }

    pub(crate) fn add_dependency(&mut self, before: Node, after: Node) {
        self.orders.push(Order { before, after });
    }

    pub(crate) fn build(self) -> Scheduler {
        let partitions: Vec<_> = self.partitions.iter().collect();
        let mut topology = Topology::init(
            self.send_systems.len(),
            self.unsend_systems.len(),
            &partitions,
            &self.orders,
            &self.resources,
            |node| self.display_node(node).to_string(),
        );
        // late-initialized because display_node needs to read this field
        topology.partitions = self.partitions.into_iter().collect();

        let planner = Mutex::new(topology.initial_planner().clone());
        let executor = Executor::new(self.concurrency);

        Scheduler {
            topology,
            planner,
            executor,
            sync_state: SyncState {
                send_systems: self
                    .send_systems
                    .into_iter()
                    .map(|(ty, sys)| (ty, Mutex::new(sys)))
                    .collect(),
            },
            unsync_state: UnsyncState { unsend_systems: self.unsend_systems },
        }
    }
}
