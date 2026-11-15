//! Mutex-protected G-M-P work publication, selection, and bounded stealing.

use std::fmt;
use std::sync::{Condvar, Mutex, MutexGuard};

use crate::machine::MachineError;
#[cfg(feature = "scheduler-diagnostics")]
use crate::metrics::SchedulerDiagnosticsStore;
use crate::queue::{GlobalRunQueue, InjectionQueue, LocalRunQueue};

const TRANSFER_BATCH: usize = 16;
const STEAL_BATCH: usize = 16;

pub(crate) struct Scheduler {
    state: Mutex<SchedulerState>,
    available: Condvar,
    #[cfg(feature = "scheduler-diagnostics")]
    diagnostics: SchedulerDiagnosticsStore,
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Scheduler").finish_non_exhaustive()
    }
}

struct SchedulerState {
    local: Vec<LocalRunQueue>,
    global: GlobalRunQueue,
    injection: InjectionQueue,
    closed: bool,
    waiting_workers: usize,
    pending_notifications: usize,
    park_events: u64,
    wake_notifications: u64,
    wake_returns: u64,
    published_tasks: u64,
    selected_tasks: u64,
    completed_tasks: u64,
    tasks_by_processor: Vec<u64>,
    peak_local_queue_lengths: Vec<usize>,
    peak_injection_queue_len: usize,
    peak_global_queue_len: usize,
    stolen_tasks: u64,
    steal_attempts: u64,
    failed_steal_attempts: u64,
    #[cfg(feature = "scheduler-benchmark")]
    steal_batch: usize,
    #[cfg(feature = "scheduler-benchmark")]
    stealing_enabled: bool,
    #[cfg(feature = "scheduler-benchmark")]
    benchmark_steal_events: Vec<StealEvent>,
    #[cfg(feature = "scheduler-benchmark")]
    benchmark_trace_enabled: bool,
    rejected_publications: u64,
    worker_state_transitions: u64,
}

#[cfg(feature = "scheduler-benchmark")]
#[derive(Clone, Debug)]
pub(crate) enum StealEvent {
    Transferred {
        routine_id: u64,
        from_processor: usize,
        to_processor: usize,
    },
    Selected {
        routine_id: u64,
        processor_index: usize,
    },
}

#[path = "accounting.rs"]
mod accounting;
#[path = "benchmark.rs"]
mod benchmark;
#[path = "publication.rs"]
mod publication;
#[path = "selection.rs"]
mod selection;
#[path = "waiting.rs"]
mod waiting;
#[path = "stealing.rs"]
mod work_stealing;

impl Scheduler {
    pub(crate) fn new(processor_count: usize) -> Result<Self, MachineError> {
        if processor_count == 0 {
            return Err(MachineError::EmptyProcessorPool);
        }
        let mut local = Vec::new();
        local
            .try_reserve_exact(processor_count)
            .map_err(|error| MachineError::SchedulerQueue(error.to_string()))?;
        local.extend((0..processor_count).map(|_| LocalRunQueue::new()));
        let mut tasks_by_processor = Vec::new();
        tasks_by_processor
            .try_reserve_exact(processor_count)
            .map_err(|error| MachineError::SchedulerQueue(error.to_string()))?;
        tasks_by_processor.resize(processor_count, 0);
        let mut peak_local_queue_lengths = Vec::new();
        peak_local_queue_lengths
            .try_reserve_exact(processor_count)
            .map_err(|error| MachineError::SchedulerQueue(error.to_string()))?;
        peak_local_queue_lengths.resize(processor_count, 0);
        Ok(Self {
            state: Mutex::new(SchedulerState {
                local,
                global: GlobalRunQueue::default(),
                injection: InjectionQueue::new(),
                closed: false,
                waiting_workers: 0,
                pending_notifications: 0,
                park_events: 0,
                wake_notifications: 0,
                wake_returns: 0,
                published_tasks: 0,
                selected_tasks: 0,
                completed_tasks: 0,
                tasks_by_processor,
                peak_local_queue_lengths,
                peak_injection_queue_len: 0,
                peak_global_queue_len: 0,
                stolen_tasks: 0,
                steal_attempts: 0,
                failed_steal_attempts: 0,
                #[cfg(feature = "scheduler-benchmark")]
                steal_batch: STEAL_BATCH,
                #[cfg(feature = "scheduler-benchmark")]
                stealing_enabled: true,
                #[cfg(feature = "scheduler-benchmark")]
                benchmark_steal_events: Vec::new(),
                #[cfg(feature = "scheduler-benchmark")]
                benchmark_trace_enabled: false,
                rejected_publications: 0,
                worker_state_transitions: 0,
            }),
            available: Condvar::new(),
            #[cfg(feature = "scheduler-diagnostics")]
            diagnostics: SchedulerDiagnosticsStore::new(processor_count),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, SchedulerState>, String> {
        self.state
            .lock()
            .map_err(|error| format!("scheduler queue lock poisoned: {error}"))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
