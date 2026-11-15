use crate::machine::MachineError;
use crate::machine::worker::dispatch::DispatchTask;
use crate::metrics::SchedulerStats;
#[cfg(feature = "scheduler-diagnostics")]
use crate::metrics::{SchedulerDiagnostics, SchedulerDiagnosticsStore};
use crate::queue::LocalRunQueue;

use super::Scheduler;

impl Scheduler {
    pub(crate) fn take_pending_for_shutdown(&self) -> Result<Option<DispatchTask>, MachineError> {
        let mut state = self.lock().map_err(MachineError::SchedulerQueue)?;
        for queue in &mut state.local {
            if let Some(task) = queue.pop_front() {
                return Ok(Some(task));
            }
        }
        Ok(state
            .global
            .pop_front()
            .or_else(|| state.injection.pop_front()))
    }

    pub(crate) fn stats(&self) -> Result<SchedulerStats, MachineError> {
        let state = self.lock().map_err(MachineError::SchedulerQueue)?;
        Ok(SchedulerStats {
            injection_queue_len: state.injection.len(),
            global_queue_len: state.global.len(),
            local_queue_lengths: state.local.iter().map(LocalRunQueue::len).collect(),
            peak_injection_queue_len: state.peak_injection_queue_len,
            peak_global_queue_len: state.peak_global_queue_len,
            peak_local_queue_lengths: state.peak_local_queue_lengths.clone(),
            parked_workers: state.waiting_workers,
            park_events: state.park_events,
            wake_notifications: state.wake_notifications,
            wake_returns: state.wake_returns,
            published_tasks: state.published_tasks,
            selected_tasks: state.selected_tasks,
            completed_tasks: state.completed_tasks,
            worker_state_transitions: state.worker_state_transitions,
            tasks_by_processor: state.tasks_by_processor.clone(),
            stolen_tasks: state.stolen_tasks,
            steal_attempts: state.steal_attempts,
            failed_steal_attempts: state.failed_steal_attempts,
            rejected_publications: state.rejected_publications,
        })
    }

    #[cfg(feature = "scheduler-diagnostics")]
    pub(crate) fn diagnostics(&self) -> SchedulerDiagnostics {
        self.diagnostics.snapshot()
    }

    #[cfg(feature = "scheduler-diagnostics")]
    pub(crate) fn record_active(
        &self,
        processor_index: usize,
        duration_ns: u128,
    ) -> Result<(), MachineError> {
        let counters = self
            .diagnostics
            .processor(processor_index)
            .map_err(MachineError::SchedulerQueue)?;
        SchedulerDiagnosticsStore::record_active(counters, duration_ns);
        Ok(())
    }

    pub(crate) fn record_completed(&self) -> Result<(), MachineError> {
        let mut state = self.lock().map_err(MachineError::SchedulerQueue)?;
        state.completed_tasks = state.completed_tasks.saturating_add(1);
        Ok(())
    }

    pub(crate) fn record_worker_state_transition(&self) -> Result<(), MachineError> {
        let mut state = self.lock().map_err(MachineError::SchedulerQueue)?;
        state.worker_state_transitions = state.worker_state_transitions.saturating_add(1);
        Ok(())
    }
}
