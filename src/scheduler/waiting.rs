#[cfg(feature = "scheduler-diagnostics")]
use std::time::Instant;

use crate::machine::MachineError;
use crate::machine::worker::dispatch::DispatchTask;
#[cfg(feature = "scheduler-diagnostics")]
use crate::metrics::{ProcessorSchedulerDiagnostics, SchedulerDiagnosticsStore};

use super::Scheduler;
use super::selection::select_task;

impl Scheduler {
    pub(crate) fn wait_next(
        &self,
        processor_index: usize,
    ) -> Result<Option<DispatchTask>, MachineError> {
        #[cfg(feature = "scheduler-diagnostics")]
        let diagnostic_counters = self
            .diagnostics
            .processor(processor_index)
            .map_err(MachineError::SchedulerQueue)?;
        #[cfg(feature = "scheduler-diagnostics")]
        let mut diagnostic_delta = ProcessorSchedulerDiagnostics {
            processor_index,
            ..ProcessorSchedulerDiagnostics::default()
        };
        #[cfg(feature = "scheduler-diagnostics")]
        let no_work_started = Instant::now();
        #[cfg(feature = "scheduler-diagnostics")]
        let lock_started = Instant::now();
        let mut state = self.lock().map_err(MachineError::SchedulerQueue)?;
        #[cfg(feature = "scheduler-diagnostics")]
        let mut lock_acquired = Instant::now();
        if processor_index >= state.local.len() {
            return Err(MachineError::SchedulerQueue(format!(
                "processor index {processor_index} exceeds scheduler queue count {}",
                state.local.len()
            )));
        }
        #[cfg(feature = "scheduler-diagnostics")]
        {
            diagnostic_delta.mutex_wait_ns = lock_acquired.duration_since(lock_started).as_nanos();
        }
        #[cfg(feature = "scheduler-diagnostics")]
        let mut stolen_out = None;
        loop {
            #[cfg(feature = "scheduler-diagnostics")]
            let selection_started = Instant::now();
            #[cfg(feature = "scheduler-diagnostics")]
            let prior_steal_attempts = state.steal_attempts;
            #[cfg(feature = "scheduler-diagnostics")]
            let prior_failed_steal_attempts = state.failed_steal_attempts;
            #[cfg(feature = "scheduler-diagnostics")]
            let prior_stolen_tasks = state.stolen_tasks;
            #[cfg(feature = "scheduler-diagnostics")]
            let selected = select_task(&mut state, processor_index, &mut stolen_out)?;
            #[cfg(not(feature = "scheduler-diagnostics"))]
            let selected = select_task(&mut state, processor_index)?;
            if let Some(task) = selected {
                #[cfg(feature = "scheduler-diagnostics")]
                {
                    diagnostic_delta.selection_ns = diagnostic_delta
                        .selection_ns
                        .saturating_add(selection_started.elapsed().as_nanos());
                    diagnostic_delta.mutex_hold_ns = diagnostic_delta
                        .mutex_hold_ns
                        .saturating_add(Instant::now().duration_since(lock_acquired).as_nanos());
                    diagnostic_delta.no_work_ns =
                        Instant::now().duration_since(no_work_started).as_nanos();
                    diagnostic_delta.selected_tasks = 1;
                    diagnostic_delta.steal_attempts = diagnostic_delta
                        .steal_attempts
                        .saturating_add(state.steal_attempts.saturating_sub(prior_steal_attempts));
                    diagnostic_delta.failed_steal_attempts =
                        diagnostic_delta.failed_steal_attempts.saturating_add(
                            state
                                .failed_steal_attempts
                                .saturating_sub(prior_failed_steal_attempts),
                        );
                    diagnostic_delta.stolen_in = diagnostic_delta
                        .stolen_in
                        .saturating_add(state.stolen_tasks.saturating_sub(prior_stolen_tasks));
                }
                state.selected_tasks = state.selected_tasks.saturating_add(1);
                state.tasks_by_processor[processor_index] =
                    state.tasks_by_processor[processor_index].saturating_add(1);
                #[cfg(feature = "scheduler-diagnostics")]
                {
                    drop(state);
                    SchedulerDiagnosticsStore::record_wait(diagnostic_counters, &diagnostic_delta);
                    if let Some((victim, transferred)) = stolen_out {
                        let victim_counters = self
                            .diagnostics
                            .processor(victim)
                            .map_err(MachineError::SchedulerQueue)?;
                        SchedulerDiagnosticsStore::record_stolen_out(victim_counters, transferred);
                    }
                }
                return Ok(Some(task));
            }
            #[cfg(feature = "scheduler-diagnostics")]
            {
                diagnostic_delta.selection_ns = diagnostic_delta
                    .selection_ns
                    .saturating_add(selection_started.elapsed().as_nanos());
                diagnostic_delta.steal_attempts = diagnostic_delta
                    .steal_attempts
                    .saturating_add(state.steal_attempts.saturating_sub(prior_steal_attempts));
                diagnostic_delta.failed_steal_attempts =
                    diagnostic_delta.failed_steal_attempts.saturating_add(
                        state
                            .failed_steal_attempts
                            .saturating_sub(prior_failed_steal_attempts),
                    );
            }
            if state.closed {
                #[cfg(feature = "scheduler-diagnostics")]
                {
                    diagnostic_delta.mutex_hold_ns = diagnostic_delta
                        .mutex_hold_ns
                        .saturating_add(Instant::now().duration_since(lock_acquired).as_nanos());
                    diagnostic_delta.no_work_ns =
                        Instant::now().duration_since(no_work_started).as_nanos();
                    drop(state);
                    SchedulerDiagnosticsStore::record_wait(diagnostic_counters, &diagnostic_delta);
                }
                return Ok(None);
            }
            #[cfg(feature = "scheduler-diagnostics")]
            {
                diagnostic_delta.mutex_hold_ns = diagnostic_delta
                    .mutex_hold_ns
                    .saturating_add(Instant::now().duration_since(lock_acquired).as_nanos());
            }
            state.park_events = state.park_events.saturating_add(1);
            state.waiting_workers = state.waiting_workers.saturating_add(1);
            #[cfg(feature = "scheduler-diagnostics")]
            let parked_started = Instant::now();
            state = match self.available.wait(state) {
                Ok(state) => state,
                Err(error) => {
                    let mut state = error.into_inner();
                    state.waiting_workers = state.waiting_workers.saturating_sub(1);
                    return Err(MachineError::SchedulerQueue(
                        "scheduler queue lock poisoned while parking".to_owned(),
                    ));
                }
            };
            #[cfg(feature = "scheduler-diagnostics")]
            {
                let resumed = Instant::now();
                diagnostic_delta.parked_ns = diagnostic_delta
                    .parked_ns
                    .saturating_add(resumed.duration_since(parked_started).as_nanos());
                diagnostic_delta.wake_returns = diagnostic_delta.wake_returns.saturating_add(1);
                lock_acquired = resumed;
            }
            state.waiting_workers = state.waiting_workers.saturating_sub(1);
            state.pending_notifications = state.pending_notifications.saturating_sub(1);
            state.wake_returns = state.wake_returns.saturating_add(1);
        }
    }
}
