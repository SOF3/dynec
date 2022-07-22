//! Exposes testing, profiling and tracing capabilities.

use std::fmt;

use crate::{system, world};

/// Defines the [`Tracer`] trait and implements the [`Log`] and [`Aggregate`] types.
///
/// All tracer method parameters must be either [`Copy`] or a mutable reference
/// (or immutable reference, which is [`Copy`]).
///
/// Use the `{@LOG_WITH = transformer}` syntax to transform an argument for log printing,
/// where `transformer` is an invokable that accepts the argument
/// and returns any [`fmt::Debug`] type.
macro_rules! define_tracer {
    (
        $(
            $(#[$meta:meta])*
            fn $name:ident(
                &self
                $(,$logged_ident:ident: $logged_ty:ty $({@LOG_WITH = $log_with:expr})?)*
                $(; @NOLOG $($nolog_ident:ident: $nolog_ty:ty),*)?
                $(,)?
            );
        )*
    ) => {
        /// A tracer used for recording the events throughout an execution cycle.
        ///
        /// Can be used for profiling and testing.
        pub trait Tracer: Sync {
            $(
                $(#[$meta])*
                #[allow(unused_variables)]
                fn $name(&self, $($logged_ident: $logged_ty,)* $($($nolog_ident: $nolog_ty,)*)?) {}
            )*
        }

        impl Tracer for Log {
            $(
                #[allow(unused_variables)]
                fn $name(&self, $($logged_ident: $logged_ty,)* $($($nolog_ident: $nolog_ty,)*)?) {
                    log::log!(self.0, concat!(stringify!($name), "(", $(
                        stringify!($logged_ident),
                        " = {",
                        stringify!($logged_ident),
                        ":?}, ",
                    )* ")"), $(
                        $logged_ident = define_tracer!(@LOG_EXPR $logged_ident $(@LOG_WITH = $log_with)?),
                    )*);
                }
            )*
        }

        impl_tuple_accumulate! {
            @TYPES (
                T1, T2, T3, T4, T5, T6, T7, T8,
                T9, T10, T11, T12, T13, T14, T15, T16,
                T17, T18, T19, T20, T21, T22, T23, T24,
                T25, T26, T27, T28, T29, T30, T31, T32,
            );
            $(
                @VARS (
                    t1, t2, t3, t4, t5, t6, t7, t8,
                    t9, t10, t11, t12, t13, t14, t15, t16,
                    t17, t18, t19, t20, t21, t22, t23, t24,
                    t25, t26, t27, t28, t29, t30, t31, t32,
                );
                @METHOD {fn $name(&self, $($logged_ident: $logged_ty,)* $($($nolog_ident: $nolog_ty,)*)?);}
            )*
        }
    };

    (@LOG_EXPR $ident:ident) => { $ident };
    (@LOG_EXPR $ident:ident @LOG_WITH = $closure:expr) => { ($closure)($ident) }
}

macro_rules! impl_tuple {
    (
        @TYPES ($($ty:ident),* $(,)?);
        $(
            @VARS ($($vars:ident),* $(,)?);
            @METHOD {fn $name:ident(&self, $($arg_ident:ident: $arg_ty:ty,)*);}
        )*
    ) => {
        impl<$($ty: Tracer),*> Tracer for Aggregate<($($ty,)*)> {
            $(
                fn $name(&self, $($arg_ident: $arg_ty),*) {
                    #[allow(unused_mut, unused_variables)]
                    let mut args = ($($arg_ident,)*);

                    #[allow(dead_code)]
                    fn call_with_args(tracer: &impl Tracer, ($($arg_ident,)*): &mut ($($arg_ty,)*)) {
                        tracer.$name($(*$arg_ident,)*);
                    }

                    let Aggregate(($($vars,)*)) = self;
                    $(
                        call_with_args($vars, &mut args);
                    )*
                }
            )*
        }
    };
}

macro_rules! impl_tuple_accumulate {
    (@TYPES (); $(@VARS (); @METHOD {$($body:tt)*})*) => {
        impl_tuple! {
            @TYPES ();
            $(
                @VARS ();
                @METHOD {$($body)*}
            )*
        }
    };
    (
        @TYPES ($first_ty:ident $(, $rest_ty:ident)* $(,)?);
        $(
            @VARS ($first_var:ident $(, $rest_var:ident)* $(,)?);
            @METHOD {$($body:tt)*}
        )*
    ) => {
        impl_tuple! {
            @TYPES ($first_ty $(, $rest_ty)* );
            $(
                @VARS ($first_var $(, $rest_var)*);
                @METHOD {$($body)*}
            )*
        }

        impl_tuple_accumulate! {
            @TYPES ($($rest_ty),*);
            $(
                @VARS ($($rest_var),*);
                @METHOD {$($body)*}
            )*
        }
    };
}

define_tracer! {
    /// A cycle starts.
    fn start_cycle(&self);

    /// A cycle ends.
    fn end_cycle(&self);

    /// A thread tries to steal a task, but all tasks have started.
    fn steal_return_complete(&self, thread: Thread);

    /// A thread tries to steal a task, but no tasks are in the runnable pool.
    fn steal_return_pending(&self, thread: Thread);

    /// A node is marked as runnable because all blockers have been removed.
    fn mark_runnable(&self, node: world::ScheduleNode);

    /// A node is unmarked as runnable because an exclusive node has been stolen.
    fn unmark_runnable(&self, node: world::ScheduleNode);

    /// A thread-safe system starts running.
    fn start_run_sendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str;
        @NOLOG
        system: &mut dyn system::Sendable,
    );

    /// A thread-safe system stops running.
    fn end_run_sendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str;
        @NOLOG
        system: &mut dyn system::Sendable,
    );

    /// A thread-unsafe system starts running.
    fn start_run_unsendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str;
        @NOLOG
        system: &mut dyn system::Unsendable,
    );

    /// A thread-unsafe system stops running.
    fn end_run_unsendable(
        &self,
        thread: Thread,
        node: world::ScheduleNode,
        debug_name: &str;
        @NOLOG
        system: &mut dyn system::Unsendable,
    );

    /// A partition completes.
    fn partition(&self, node: world::ScheduleNode, partition: &dyn system::Partition {@LOG_WITH = RefPartitionWrapper});
}

struct RefPartitionWrapper<'t>(&'t dyn system::Partition);

impl<'t> fmt::Debug for RefPartitionWrapper<'t> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.0.describe(f) }
}

/// An empty tracer.
pub struct Noop;

impl Tracer for Noop {}

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
