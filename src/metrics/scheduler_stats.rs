//! Immutable scheduler diagnostics.

/// Point-in-time queue, worker parking, and task activity counters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerStats {
    /// Number of tasks waiting in the injection queue.
    pub injection_queue_len: usize,
    /// Number of overflow tasks waiting in the global run queue.
    pub global_queue_len: usize,
    /// Number of tasks waiting in each Processor's local queue, by index.
    pub local_queue_lengths: Vec<usize>,
    /// Highest observed occupancy of the injection queue since startup.
    pub peak_injection_queue_len: usize,
    /// Highest observed occupancy of the global run queue since startup.
    pub peak_global_queue_len: usize,
    /// Highest observed occupancy of each local queue since startup.
    pub peak_local_queue_lengths: Vec<usize>,
    /// Number of workers currently waiting for work.
    pub parked_workers: usize,
    /// Total number of times workers entered a condition-variable wait.
    pub park_events: u64,
    /// Number of condition-variable notifications sent to workers.
    pub wake_notifications: u64,
    /// Number of returns from condition-variable waits, including spurious returns.
    pub wake_returns: u64,
    /// Total number of tasks accepted for publication.
    pub published_tasks: u64,
    /// Total number of tasks selected by a worker.
    pub selected_tasks: u64,
    /// Total number of routine executions that returned to the scheduler.
    pub completed_tasks: u64,
    /// Number of worker state transitions observed since startup.
    pub worker_state_transitions: u64,
    /// Number of tasks selected by each Processor, by index.
    pub tasks_by_processor: Vec<u64>,
    /// Total number of tasks moved from one local queue to another by stealing.
    pub stolen_tasks: u64,
    /// Number of attempts to steal after the current Processor found no shared work.
    pub steal_attempts: u64,
    /// Number of steal attempts that found no transferable local work.
    pub failed_steal_attempts: u64,
    /// Number of publications rejected because the scheduler was closed or capacity could not be reserved.
    pub rejected_publications: u64,
}
