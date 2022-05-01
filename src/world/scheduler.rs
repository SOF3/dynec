use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;
use std::thread;

use crossbeam::channel;
use indexmap::IndexSet;
use parking_lot::{Condvar, Mutex, MutexGuard};

use crate::system;
use crate::util::DbgTypeId;

#[derive(Default)]
pub(crate) struct Builder {
    /// Systems that can be scheduled to other threads.
    pub(crate) send_systems:   Vec<Box<dyn system::Spec + Send>>,
    /// Systems that must be scheduled to the main thread.
    pub(crate) unsend_systems: Vec<Box<dyn system::Spec>>,

    /// Global states that can be concurrently accessed by systems on other threads.
    pub(crate) send_globals:   HashMap<DbgTypeId, Option<Box<dyn Any + Send>>>,
    /// Global states that must be accessed on the main thread.
    pub(crate) unsend_globals: HashMap<DbgTypeId, Option<Box<dyn Any>>>,

    pub(crate) partitions: IndexSet<system::PartitionWrapper>,

    /// Indexes systems that access a component.
    pub(crate) components: HashMap<DbgTypeId, Vec<(TaskId, ComponentAccess)>>,
    /// Indexes systems that access a global.
    pub(crate) globals:    HashMap<DbgTypeId, Vec<(TaskId, bool)>>,

    /// If `dependencies[a].contains(b)`, `b` runs before `a`
    pub(crate) dependencies: HashMap<TaskId, Vec<TaskId>>,
    /// If `dependents[a].contains(b)`, `a` runs before `b`
    pub(crate) dependents:   HashMap<TaskId, Vec<TaskId>>,
}

pub(crate) struct Scheduler {
    graph:      Graph,
    sync_state: SyncState,
    state:      Mutex<State>,
}

impl Builder {
    pub(crate) fn build(self) -> Scheduler {
        let send_tasks =
            (0..self.send_systems.len()).map(|index| TaskId { class: TaskClass::Send, index });
        let unsend_tasks =
            (0..self.unsend_systems.len()).map(|index| TaskId { class: TaskClass::Unsend, index });
        let partition_tasks =
            (0..self.partitions.len()).map(|index| TaskId { class: TaskClass::Partition, index });

        let mut dependents: HashMap<_, _> = [TASK_SOURCE, TASK_SINK]
            .into_iter()
            .chain(send_tasks.clone())
            .chain(unsend_tasks.clone())
            .chain(partition_tasks.clone())
            .map(|task| (task, Vec::new()))
            .collect();

        for task in send_tasks.clone().chain(unsend_tasks.clone()) {
            // all systems depend on TASK_SOURCE
            dependents.get_mut(&task).expect("just inserted").push(TASK_SOURCE);
            // TASK_SINK depends on all systems
            dependents.get_mut(&TASK_SINK).expect("just inserted").push(task);
        }

        for (task, task_dependents) in &self.dependents {
            dependents.get_mut(task).expect("unknown task").extend(task_dependents);
        }

        let mut exclusions: HashMap<_, _> = [TASK_SOURCE, TASK_SINK]
            .into_iter()
            .chain(send_tasks.clone())
            .chain(unsend_tasks.clone())
            .chain(partition_tasks.clone())
            .map(|task| (task, Vec::new()))
            .collect();

        for (comp_ty, tasks) in &self.components {
            for (offset, &(task1, ref access1)) in tasks.iter().enumerate() {
                for &(task2, ref access2) in &tasks[(offset + 1)..] {
                    if access1.conflicts_with(access2) {
                        exclusions.get_mut(&task1).expect("unknown task").push(task2);
                        exclusions.get_mut(&task2).expect("unknown task").push(task1);
                    }
                }
            }
        }

        let mut blocker_count_cache: HashMap<_, _> = [TASK_SOURCE, TASK_SINK]
            .into_iter()
            .chain(send_tasks)
            .chain(unsend_tasks)
            .chain(partition_tasks)
            .map(|task| (task, 0))
            .collect();
        for (dep, vec) in &self.dependents {
            for task_dependent in vec {
                *blocker_count_cache.get_mut(task_dependent).expect("unknown task") += 1;
            }
        }

        let graph = Graph {
            dependents,
            exclusions,
            blocker_count_cache,
            send_systems: self.send_systems.into_iter().map(Mutex::new).collect(),
        };

        let sync_state = SyncState { condvar: Condvar::new(), completed: AtomicBool::new(false) };

        let state = State {
            blocker_count:         graph.blocker_count_cache.clone(),
            send_runnable_queue:   VecDeque::new(),
            unsend_runnable_queue: VecDeque::new(),
        };

        Scheduler { graph, sync_state, state: Mutex::new(state) }
    }
}

pub(crate) struct ThreadPool {
    workers: Vec<Worker>,
}

impl ThreadPool {
    fn new(size: usize) -> Self { Self { workers: Vec::with_capacity(size) } }

    fn run(&mut self, scheduler: &Arc<Scheduler>) {
        while self.workers.len() < self.workers.capacity() {
            self.workers.push(Worker::spawn(Arc::clone(scheduler)));
        }

        for worker in &self.workers {
            match worker.wakeup.try_send(WorkerWakeup::StartCycle) {
                Ok(()) => (),
                Err(channel::TrySendError::Full(_)) => {
                    log::warn!("worker thread is more than one cycle off");
                }
                Err(channel::TrySendError::Disconnected(_)) => panic!("worker thread panic"),
            }
        }
    }
}

enum WorkerWakeup {
    StartCycle,
    Shutdown,
}

struct Worker {
    wakeup: channel::Sender<WorkerWakeup>,
    jh:     Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn spawn(scheduler: Arc<Scheduler>) -> Self {
        let (wakeup_tx, wakeup_rx) = channel::bounded(1);
        let jh = thread::spawn(move || {
            for wakeup in wakeup_rx {
                match wakeup {
                    WorkerWakeup::StartCycle => {
                        let state = &scheduler.state;
                        let mut state = state.lock();
                        if let Some(task) = State::steal_task(
                            &mut state,
                            false,
                            &scheduler.graph,
                            &scheduler.sync_state,
                        ) {
                            assert_eq!(task.class, TaskClass::Send);
                            let system = scheduler
                                .graph
                                .send_systems
                                .get(task.index)
                                .expect("stole unknown task");
                            let mut system = system
                                .try_lock()
                                .expect("stolen system is locked by another system");
                            system.run();
                        }
                    }
                    WorkerWakeup::Shutdown => break,
                }
            }
        });

        Self { wakeup: wakeup_tx, jh: Some(jh) }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.wakeup.send(WorkerWakeup::Shutdown).expect("worker thread panic");
        self.jh
            .take()
            .expect("JoinHandle is only dropped in the Drop impl")
            .join()
            .expect("worker thread panic");
    }
}

pub(crate) struct Graph {
    pub(crate) dependents:          HashMap<TaskId, Vec<TaskId>>,
    pub(crate) exclusions:          HashMap<TaskId, Vec<TaskId>>,
    pub(crate) blocker_count_cache: HashMap<TaskId, usize>,
    pub(crate) send_systems:        Vec<Mutex<Box<dyn system::Spec + Send>>>,
}

impl Graph {
    /// The tasks that depend on the given task.
    fn dependents(&self, task: TaskId) -> impl Iterator<Item = TaskId> + '_ {
        self.dependents.get(&task).expect("unknown task").iter().copied()
    }

    /// The tasks that cannot run simultaneously with the given task.
    fn exclusions(&self, task: TaskId) -> impl Iterator<Item = TaskId> + '_ {
        self.exclusions.get(&task).expect("unknown task").iter().copied()
    }
}

/// The synchronized states used for ITC.
pub(crate) struct SyncState {
    condvar:   Condvar,
    completed: AtomicBool,
}

impl SyncState {
    fn reset(&mut self) { *self.completed.get_mut() = false; }
}

/// The mutable states used for scheduling.
pub(crate) struct State {
    blocker_count:         HashMap<TaskId, usize>,
    /// The queue of thread-safe tasks that are marked for runnability check.
    /// Note that some tasks might not be runnable because of another task in the queue.
    send_runnable_queue:   VecDeque<usize>,
    /// The queue of main-thread-only tasks that are marked for runnability check.
    /// Note that some tasks might not be runnable because of another task in the queue.
    unsend_runnable_queue: VecDeque<usize>,
}

impl State {
    fn reset(&mut self, graph: &Graph) {
        assert!(
            self.send_runnable_queue.is_empty(),
            "reset should only be called when all queues are clear"
        );
        assert!(
            self.unsend_runnable_queue.is_empty(),
            "reset should only be called when all queues are clear"
        );
        self.blocker_count.clone_from(&graph.blocker_count_cache);
    }

    /// Marks a task as completed, initiating downstream tasks.
    fn finish(&mut self, task: TaskId, graph: &Graph, sync_state: &SyncState) {
        self.finish_inner(task, graph, sync_state);
        sync_state.condvar.notify_all();
    }

    /// count must only decrease in this loop.
    /// `push_task` may recurse back to `finish` with partitions,
    /// but this should not cause any logical issues
    /// `push_task` itself does not start any resource-acquiring tasks.
    fn finish_inner(&mut self, task: TaskId, graph: &Graph, sync_state: &SyncState) {
        for dependent in graph.dependents(task).chain(graph.exclusions(task)) {
            let count = self.blocker_count.get_mut(&dependent).expect("unknown task");
            *count -= 1;
            if *count == 0 {
                self.push_task(dependent, graph, sync_state);
            }
        }
    }

    /// Pushes a task to the runnable queue.
    fn push_task(&mut self, task: TaskId, graph: &Graph, sync_state: &SyncState) {
        match task.class {
            TaskClass::Endpoint => {
                assert!(task.index == 1, "Should not push source task for running");
                sync_state.completed.store(true, atomic::Ordering::SeqCst);
            }
            TaskClass::Partition => {
                self.finish(task, graph, sync_state);
            }
            TaskClass::Send => {
                self.send_runnable_queue.push_back(task.index);
            }
            TaskClass::Unsend => {
                self.unsend_runnable_queue.push_back(task.index);
            }
        };
    }

    /// Steals a task and updates the runnability constraints.
    ///
    /// Blocks the thread until a task is available or the iteration is done.
    /// Returns `None` if the cycle finished.
    fn steal_task(
        this: &mut MutexGuard<'_, Self>,
        main_thread: bool,
        graph: &Graph,
        sync_state: &SyncState,
    ) -> Option<TaskId> {
        loop {
            if sync_state.completed.load(atomic::Ordering::SeqCst) {
                return None;
            }

            if main_thread {
                if let Some(index) = this.unsend_runnable_queue.pop_front() {
                    let task = TaskId { class: TaskClass::Unsend, index };
                    if this.start_task(task, graph) {
                        return Some(task);
                    }
                }
            }

            if let Some(index) = this.send_runnable_queue.pop_front() {
                let task = TaskId { class: TaskClass::Send, index };
                if this.start_task(task, graph) {
                    return Some(task);
                }
            }

            sync_state.condvar.wait(this);
        }
    }

    /// Starts a task if it is runnable and sets up its resource acquisition.
    /// Returns whether it is runnable.
    fn start_task(&mut self, task: TaskId, graph: &Graph) -> bool {
        if *self.blocker_count.get(&task).expect("unknown task") == 0 {
            for exclusion in graph.exclusions(task) {
                let count = self.blocker_count.get_mut(&exclusion).expect("unknown task");
                *count += 1;
            }

            true
        } else {
            // No need to push back to the queue, because it is currently not runnable.
            // It will get pushed to the queue again when the blocker finishes.
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TaskId {
    pub(crate) class: TaskClass,
    pub(crate) index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum TaskClass {
    Endpoint,
    Send,
    Unsend,
    Partition,
}

pub(crate) struct ComponentAccess {
    pub(crate) exclusive: bool,
    pub(crate) discrim:   Option<Vec<usize>>,
}

impl ComponentAccess {
    fn conflicts_with(&self, other: &Self) -> bool {
        let intersects = match (&self.discrim, &other.discrim) {
            (Some(this), Some(that)) => this.iter().any(|discrim| that.contains(discrim)),
            _ => true,
        };

        intersects && (self.exclusive || other.exclusive)
    }
}

pub(crate) const TASK_SOURCE: TaskId = TaskId { class: TaskClass::Endpoint, index: 0 };
pub(crate) const TASK_SINK: TaskId = TaskId { class: TaskClass::Endpoint, index: 1 };
