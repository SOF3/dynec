//! Exposes testing, profiling and tracing capabilities.

use crate::{system, world};

/// A tracer used for recording the events throughout an execution cycle.
///
/// Can be used for profiling and testing.
pub trait Tracer: Sync {
    /// A cycle starts.
    fn start_cycle(&self) {}

    /// A cycle ends.
    fn end_cycle(&self) {}

    /// A thread tries to steal a task, but all tasks have started.
    fn steal_return_complete(&self, thread: Thread) {}

    /// A thread tries to steal a task, but no tasks are in the runnable pool.
    fn steal_return_pending(&self, thread: Thread) {}

    /// A node is marked as runnable because all blockers have been removed.
    fn mark_runnable(&self, node: world::ScheduleNode) {}

    /// A node is unmarked as runnable because an exclusive node has been stolen.
    fn unmark_runnable(&self, node: world::ScheduleNode) {}

    /// A thread-safe system starts running.
    fn start_run_sendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Sendable,
    ) {
    }

    /// A thread-safe system stops running.
    fn end_run_sendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Sendable,
    ) {
    }

    /// A thread-unsafe system starts running.
    fn start_run_unsendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Unsendable,
    ) {
    }

    /// A thread-unsafe system stops running.
    fn end_run_unsendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Unsendable,
    ) {
    }

    /// A partition completes.
    fn partition(&self, node: world::ScheduleNode, partition: &dyn system::Partition) {}
}

/// An empty tracer.
pub struct Noop;

impl Tracer for Noop {}

/// A tracer that logs all events.
pub struct Log(
    /// The log level to log events with.
    pub log::Level,
);

impl Tracer for Log {
    fn start_cycle(&self) {
        log::log!(self.0, "start_cycle()");
    }

    fn end_cycle(&self) {
        log::log!(self.0, "end_cycle()");
    }

    fn steal_return_complete(&self, thread: Thread) {
        log::log!(self.0, "steal_return_complete(thread = {thread:?})");
    }

    fn steal_return_pending(&self, thread: Thread) {
        log::log!(self.0, "steal_return_pending(thread = {thread:?})");
    }

    fn mark_runnable(&self, node: world::ScheduleNode) {
        log::log!(self.0, "mark_runnable(node = {node:?})");
    }

    fn unmark_runnable(&self, node: world::ScheduleNode) {
        log::log!(self.0, "unmark_runnable(node = {node:?})");
    }

    fn start_run_sendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Sendable,
    ) {
        log::log!(
            self.0,
            "start_run_sendable(thread = {thread:?}, node = {node:?}, debug_name = {debug_name:?})"
        )
    }

    fn end_run_sendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Sendable,
    ) {
        log::log!(
            self.0,
            "end_run_sendable(thread = {thread:?}, node = {node:?}, debug_name = {debug_name:?})"
        )
    }

    fn start_run_unsendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Unsendable,
    ) {
        log::log!(
            self.0,
            "start_run_unsendable(thread = {thread:?}, node = {node:?}, debug_name = \
             {debug_name:?})"
        )
    }

    fn end_run_unsendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str,
        system: &mut dyn system::Unsendable,
    ) {
        log::log!(
            self.0,
            "end_run_unsendable(thread = {thread:?}, node = {node:?}, debug_name = {debug_name:?})"
        )
    }

    fn partition(&self, node: world::ScheduleNode, partition: &dyn system::Partition) {
        log::log!(self.0, "partition(node = {node:?})")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Thread {
    Main,
    Worker(usize),
}
