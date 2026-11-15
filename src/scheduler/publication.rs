use std::sync::Condvar;
#[cfg(feature = "scheduler-diagnostics")]
use std::time::Instant;

use super::{Scheduler, SchedulerState};
use crate::machine::MachineError;
use crate::machine::worker::dispatch::DispatchTask;
#[cfg(feature = "scheduler-diagnostics")]
use crate::metrics::{ProcessorCounters, SchedulerDiagnosticsStore};

impl Scheduler {
    pub(crate) fn publish(
        &self,
        task: DispatchTask,
        _processor_index: usize,
    ) -> Result<(), (DispatchTask, String)> {
        #[cfg(feature = "scheduler-diagnostics")]
        let diagnostic_counters: &ProcessorCounters =
            match self.diagnostics.processor(_processor_index) {
                Ok(counters) => counters,
                Err(message) => return Err((task, message)),
            };
        #[cfg(feature = "scheduler-diagnostics")]
        let lock_started = Instant::now();
        let mut state = match self.lock() {
            Ok(state) => state,
            Err(message) => return Err((task, message)),
        };
        #[cfg(feature = "scheduler-diagnostics")]
        let lock_acquired = Instant::now();
        let result = if state.closed {
            state.rejected_publications = state.rejected_publications.saturating_add(1);
            Err((task, "scheduler is shutting down".to_owned()))
        } else if !state.injection.has_capacity() {
            if let Err(message) = state.global.reserve_one() {
                state.rejected_publications = state.rejected_publications.saturating_add(1);
                Err((task, message))
            } else {
                publish_reserved(&mut state, task, &self.available);
                Ok(())
            }
        } else {
            publish_reserved(&mut state, task, &self.available);
            Ok(())
        };
        #[cfg(feature = "scheduler-diagnostics")]
        let mutex_hold_ns = Instant::now().duration_since(lock_acquired).as_nanos();
        drop(state);
        #[cfg(feature = "scheduler-diagnostics")]
        SchedulerDiagnosticsStore::record_publication(
            diagnostic_counters,
            lock_acquired.duration_since(lock_started).as_nanos(),
            mutex_hold_ns,
        );
        result
    }

    pub(crate) fn close(&self) -> Result<(), MachineError> {
        let mut state = self.lock().map_err(MachineError::SchedulerQueue)?;
        state.closed = true;
        self.available.notify_all();
        Ok(())
    }
}

fn publish_reserved(state: &mut SchedulerState, task: DispatchTask, available: &Condvar) {
    if state.injection.has_capacity() {
        state.injection.push_reserved(task);
        state.peak_injection_queue_len = state.peak_injection_queue_len.max(state.injection.len());
    } else {
        state.global.push_reserved(task);
        state.peak_global_queue_len = state.peak_global_queue_len.max(state.global.len());
    }
    state.published_tasks = state.published_tasks.saturating_add(1);
    if state.waiting_workers > state.pending_notifications {
        state.pending_notifications = state.pending_notifications.saturating_add(1);
        state.wake_notifications = state.wake_notifications.saturating_add(1);
        available.notify_one();
    }
}
