//! Opt-in scheduler timing diagnostics.

use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
#[path = "scheduler_diagnostics/tests.rs"]
mod tests;

/// Detailed scheduler measurements collected only with `scheduler-diagnostics`.
///
/// Counters are loaded independently; a live snapshot is not transactionally
/// consistent across Processors.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchedulerDiagnostics {
    /// Aggregate time callers waited to acquire the scheduler mutex.
    pub mutex_wait_ns: u128,
    /// Aggregate time callers held the scheduler mutex.
    pub mutex_hold_ns: u128,
    /// Per-Processor diagnostic totals.
    pub processors: Vec<ProcessorSchedulerDiagnostics>,
}

/// Scheduler work and idle measurements attributed to one Processor.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessorSchedulerDiagnostics {
    /// Processor index in the manager.
    pub processor_index: usize,
    /// Time from worker lock acquisitions and dispatches assigned to this
    /// Processor spent waiting for the scheduler mutex.
    pub mutex_wait_ns: u128,
    /// Time attributed to this Processor spent holding the scheduler mutex.
    pub mutex_hold_ns: u128,
    /// Time spent waiting for the scheduler mutex during dispatch publication.
    pub publish_mutex_wait_ns: u128,
    /// Time spent holding the scheduler mutex during dispatch publication.
    pub publish_mutex_hold_ns: u128,
    /// Time spent waiting for the scheduler mutex in the worker scheduling loop.
    pub worker_mutex_wait_ns: u128,
    /// Time spent holding the scheduler mutex in the worker scheduling loop.
    pub worker_mutex_hold_ns: u128,
    /// Time spent searching queues and transferring tasks.
    pub selection_ns: u128,
    /// Time from beginning a scheduler wait until a task was selected or shutdown.
    pub no_work_ns: u128,
    /// Time spent inside condition-variable waits.
    pub parked_ns: u128,
    /// Number of tasks selected by this Processor.
    pub selected_tasks: u64,
    /// Number of steal attempts made by this Processor.
    pub steal_attempts: u64,
    /// Number of unsuccessful steal attempts made by this Processor.
    pub failed_steal_attempts: u64,
    /// Number of tasks this Processor received through steals.
    pub stolen_in: u64,
    /// Number of tasks stolen from this Processor.
    pub stolen_out: u64,
    /// Time spent executing routine entries and their one-shot context lifecycle.
    pub active_ns: u128,
    /// Number of condition-variable wait returns observed by this Processor.
    pub wake_returns: u64,
}

#[derive(Debug)]
pub(crate) struct SchedulerDiagnosticsStore {
    processors: Vec<ProcessorCounters>,
}

#[repr(align(64))]
#[derive(Debug, Default)]
pub(crate) struct ProcessorCounters {
    mutex_wait_ns: AtomicU64,
    mutex_hold_ns: AtomicU64,
    publish_mutex_wait_ns: AtomicU64,
    publish_mutex_hold_ns: AtomicU64,
    worker_mutex_wait_ns: AtomicU64,
    worker_mutex_hold_ns: AtomicU64,
    selection_ns: AtomicU64,
    no_work_ns: AtomicU64,
    parked_ns: AtomicU64,
    selected_tasks: AtomicU64,
    steal_attempts: AtomicU64,
    failed_steal_attempts: AtomicU64,
    stolen_in: AtomicU64,
    stolen_out: AtomicU64,
    active_ns: AtomicU64,
    wake_returns: AtomicU64,
}

impl SchedulerDiagnosticsStore {
    pub(crate) fn new(processor_count: usize) -> Self {
        Self {
            processors: (0..processor_count)
                .map(|_| ProcessorCounters::default())
                .collect(),
        }
    }

    pub(crate) fn processor(&self, processor_index: usize) -> Result<&ProcessorCounters, String> {
        self.processors
            .get(processor_index)
            .ok_or_else(|| format!("invalid diagnostics Processor index {processor_index}"))
    }

    pub(crate) fn record_publication(
        counters: &ProcessorCounters,
        mutex_wait_ns: u128,
        mutex_hold_ns: u128,
    ) {
        add(&counters.mutex_wait_ns, nanos(mutex_wait_ns));
        add(&counters.mutex_hold_ns, nanos(mutex_hold_ns));
        add(&counters.publish_mutex_wait_ns, nanos(mutex_wait_ns));
        add(&counters.publish_mutex_hold_ns, nanos(mutex_hold_ns));
    }

    pub(crate) fn record_wait(counters: &ProcessorCounters, delta: &ProcessorSchedulerDiagnostics) {
        add(&counters.mutex_wait_ns, nanos(delta.mutex_wait_ns));
        add(&counters.mutex_hold_ns, nanos(delta.mutex_hold_ns));
        add(&counters.worker_mutex_wait_ns, nanos(delta.mutex_wait_ns));
        add(&counters.worker_mutex_hold_ns, nanos(delta.mutex_hold_ns));
        add(&counters.selection_ns, nanos(delta.selection_ns));
        add(&counters.no_work_ns, nanos(delta.no_work_ns));
        add(&counters.parked_ns, nanos(delta.parked_ns));
        add(&counters.selected_tasks, delta.selected_tasks);
        add(&counters.steal_attempts, delta.steal_attempts);
        add(&counters.failed_steal_attempts, delta.failed_steal_attempts);
        add(&counters.stolen_in, delta.stolen_in);
        add(&counters.wake_returns, delta.wake_returns);
    }

    pub(crate) fn record_stolen_out(counters: &ProcessorCounters, count: u64) {
        add(&counters.stolen_out, count);
    }

    pub(crate) fn record_active(counters: &ProcessorCounters, duration_ns: u128) {
        add(&counters.active_ns, nanos(duration_ns));
    }

    pub(crate) fn snapshot(&self) -> SchedulerDiagnostics {
        let processors = self
            .processors
            .iter()
            .enumerate()
            .map(
                |(processor_index, counters)| ProcessorSchedulerDiagnostics {
                    processor_index,
                    mutex_wait_ns: read(&counters.mutex_wait_ns),
                    mutex_hold_ns: read(&counters.mutex_hold_ns),
                    publish_mutex_wait_ns: read(&counters.publish_mutex_wait_ns),
                    publish_mutex_hold_ns: read(&counters.publish_mutex_hold_ns),
                    worker_mutex_wait_ns: read(&counters.worker_mutex_wait_ns),
                    worker_mutex_hold_ns: read(&counters.worker_mutex_hold_ns),
                    selection_ns: read(&counters.selection_ns),
                    no_work_ns: read(&counters.no_work_ns),
                    parked_ns: read(&counters.parked_ns),
                    selected_tasks: counters.selected_tasks.load(Ordering::Relaxed),
                    steal_attempts: counters.steal_attempts.load(Ordering::Relaxed),
                    failed_steal_attempts: counters.failed_steal_attempts.load(Ordering::Relaxed),
                    stolen_in: counters.stolen_in.load(Ordering::Relaxed),
                    stolen_out: counters.stolen_out.load(Ordering::Relaxed),
                    active_ns: read(&counters.active_ns),
                    wake_returns: counters.wake_returns.load(Ordering::Relaxed),
                },
            )
            .collect::<Vec<_>>();
        let mutex_wait_ns = processors.iter().map(|item| item.mutex_wait_ns).sum();
        let mutex_hold_ns = processors.iter().map(|item| item.mutex_hold_ns).sum();
        SchedulerDiagnostics {
            mutex_wait_ns,
            mutex_hold_ns,
            processors,
        }
    }
}

fn nanos(value: u128) -> u64 {
    value.min(u128::from(u64::MAX)) as u64
}

fn read(value: &AtomicU64) -> u128 {
    u128::from(value.load(Ordering::Relaxed))
}

fn add(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_add(value);
        match target.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}
