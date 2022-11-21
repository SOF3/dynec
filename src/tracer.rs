//! Exposes testing, profiling and tracing capabilities.

use std::{fmt, time};

use crate::util::DbgTypeId;
use crate::{scheduler, system};

/// A handler that receives scheduling-related events in dynec.
///
/// New implementations should use [`#[tracer]`](crate::tracer)
/// to auto implement required types and methods for future compatibility.
#[dynec_codegen::tracer_def(
    max_tuple_len = 32,
    import = crate::util::DbgTypeId,
    import = crate::scheduler,
    import = crate::system,
)]
pub trait Tracer: Sync {
    /// Context from [`start_cycle`](Self::start_cycle) to [`end_cycle`](Self::end_cycle).
    #[dynec(log_time)]
    type CycleContext;
    /// A cycle starts.
    #[dynec(log_return_now)]
    fn start_cycle(&self) -> Self::CycleContext;
    /// A cycle ends.
    fn end_cycle(&self, #[dynec(log_with = ElapsedFmt)] arg: Self::CycleContext);

    /// Context from [`start_prepare_ealloc_shards`](Self::start_prepare_ealloc_shards)
    /// to [`end_prepare_ealloc_shards`](Self::end_prepare_ealloc_shards).
    #[dynec(log_time)]
    type PrepareEallocShardsContext;
    /// The executor starts preparing ealloc shards for each worker thread.
    #[dynec(log_return_now)]
    fn start_prepare_ealloc_shards(&self) -> Self::PrepareEallocShardsContext;
    /// The executor has partitioned ealloc into different worker threads.
    fn end_prepare_ealloc_shards(
        &self,
        #[dynec(log_with = ElapsedFmt)] arg: Self::PrepareEallocShardsContext,
    );

    /// Context from [`start_flush_ealloc`](Self::start_flush_ealloc)
    /// to [`end_flush_ealloc`](Self::end_flush_ealloc).
    #[dynec(log_time)]
    type FlushEallocContext;
    /// The executor starts preparing ealloc shards for each worker thread.
    #[dynec(log_return_now)]
    fn start_flush_ealloc(&self, archetype: DbgTypeId) -> Self::FlushEallocContext;
    /// The executor has partitioned ealloc into different worker threads.
    fn end_flush_ealloc(
        &self,
        #[dynec(log_with = ElapsedFmt)] arg: Self::FlushEallocContext,
        archetype: DbgTypeId,
    );

    /// A thread tries to steal a task, but all tasks have started.
    fn steal_return_complete(&self, thread: Thread);

    /// A thread tries to steal a task, but no tasks are in the runnable pool.
    fn steal_return_pending(&self, thread: Thread);

    /// A node is marked as runnable because all blockers have been removed.
    fn mark_runnable(&self, node: scheduler::Node);

    /// A node is unmarked as runnable because an exclusive node has been stolen.
    fn unmark_runnable(&self, node: scheduler::Node);

    /// A system has completed. Also passes the number of remaining nodes.
    fn complete_system(&self, node: scheduler::Node, remaining: usize);

    /// Context from [`start_run_sendable`](Self::start_run_sendable)
    /// to [`end_run_sendable`](Self::end_run_sendable).
    #[dynec(log_time)]
    type RunSendableContext;
    /// A thread-safe system starts running.
    #[dynec(log_return_now)]
    fn start_run_sendable(
        &self,
        thread: Thread,
        node: scheduler::Node,
        debug_name: &str,
        #[dynec(log_skip)] system: &mut dyn system::Sendable,
    ) -> Self::RunSendableContext;

    /// A thread-safe system stops running.
    fn end_run_sendable(
        &self,
        #[dynec(log_with = ElapsedFmt)] context: Self::RunSendableContext,
        thread: Thread,
        node: scheduler::Node,
        debug_name: &str,
        #[dynec(log_skip)] system: &mut dyn system::Sendable,
    );

    /// Context from [`start_run_unsendable`](Self::start_run_unsendable)
    /// to [`end_run_unsendable`](Self::end_run_unsendable).
    #[dynec(log_time)]
    type RunUnsendableContext;
    /// A thread-unsafe system starts running.
    #[dynec(log_return_now)]
    fn start_run_unsendable(
        &self,
        thread: Thread,
        node: scheduler::Node,
        debug_name: &str,
        #[dynec(log_skip)] system: &mut dyn system::Unsendable,
    ) -> Self::RunUnsendableContext;

    /// A thread-unsafe system stops running.
    fn end_run_unsendable(
        &self,
        #[dynec(log_with = ElapsedFmt)] context: Self::RunUnsendableContext,
        thread: Thread,
        node: scheduler::Node,
        debug_name: &str,
        #[dynec(log_skip)] system: &mut dyn system::Unsendable,
    );

    /// A partition completes.
    fn partition(
        &self,
        node: scheduler::Node,
        #[dynec(log_with = PartitionFmt)] partition: &dyn system::Partition,
    );
}

struct PartitionFmt<'t>(&'t dyn system::Partition);

impl<'t> fmt::Display for PartitionFmt<'t> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.0.describe(f) }
}

struct ElapsedFmt(time::Instant);

impl fmt::Display for ElapsedFmt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{:?}", self.0.elapsed()) }
}

/// An empty tracer.
pub struct Noop;

/// Groups multiple tracers into a tuple and dispatches each call to them in serial.
pub struct Aggregate<T>(
    /// A tuple of child tracers to execute in serial.
    pub T,
);

/// A tracer that logs all events.
pub struct Log(
    /// The log level to log events with.
    pub log::Level,
);

/// The thread ID for a system executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Thread {
    /// The main thread, typically used for executing thread-unsafe systems.
    Main,
    /// A worker thread. The index is in the range `0..concurrency`.
    Worker(usize),
}
