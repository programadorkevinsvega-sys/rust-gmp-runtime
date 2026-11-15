/// Measurements from selecting work preloaded onto one local Processor queue.
#[derive(Clone, Debug)]
pub struct StealSample {
    /// Elapsed time for synchronized idle Processors to drain the local queue.
    pub elapsed_ns: u128,
    /// Number of individual tasks transferred by successful steals.
    pub stolen_tasks: u64,
    /// Number of attempts made to steal from a local queue.
    pub steal_attempts: u64,
    /// Number of steal attempts that found no transferable local work.
    pub failed_steal_attempts: u64,
    /// Number of tasks selected by each Processor.
    pub tasks_by_processor: Vec<u64>,
    /// Time spent draining work by each thief, summed across refill rounds.
    pub thief_drain_ns: Vec<u128>,
    /// Time spent waiting at experiment barriers by each thief; diagnostic only.
    pub thief_barrier_wait_ns: Vec<u128>,
    /// Per-routine transfer history; populated only in the untimed trace pass.
    pub task_traces: Vec<TaskTransferTrace>,
}

/// Transfer history and final selection Processor for one benchmark routine.
#[derive(Clone, Debug)]
pub struct TaskTransferTrace {
    /// Process-unique routine identifier.
    pub routine_id: u64,
    /// Processor whose local queue initially held the routine.
    pub initial_processor: usize,
    /// Ordered `(source P, destination P)` transfers.
    pub transfers: Vec<(usize, usize)>,
    /// Processor that selected the task, if selected.
    pub selected_by: Option<usize>,
}
