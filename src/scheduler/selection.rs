use crate::machine::MachineError;
use crate::machine::worker::dispatch::DispatchTask;

#[cfg(feature = "scheduler-benchmark")]
use super::StealEvent;
use super::work_stealing::{refill_local_from_global, refill_local_from_injection, steal_work};
use super::{Scheduler, SchedulerState};

impl Scheduler {
    #[cfg(any(test, feature = "scheduler-benchmark"))]
    pub(crate) fn take_next(
        &self,
        processor_index: usize,
    ) -> Result<Option<DispatchTask>, MachineError> {
        let mut state = self.lock().map_err(MachineError::SchedulerQueue)?;
        #[cfg(feature = "scheduler-diagnostics")]
        let mut stolen_out = None;
        #[cfg(feature = "scheduler-diagnostics")]
        let task = select_task(&mut state, processor_index, &mut stolen_out)?;
        #[cfg(not(feature = "scheduler-diagnostics"))]
        let task = select_task(&mut state, processor_index)?;
        if task.is_some() {
            state.selected_tasks = state.selected_tasks.saturating_add(1);
            state.tasks_by_processor[processor_index] =
                state.tasks_by_processor[processor_index].saturating_add(1);
        }
        Ok(task)
    }
}

pub(super) fn select_task(
    state: &mut SchedulerState,
    processor_index: usize,
    #[cfg(feature = "scheduler-diagnostics")] stolen_out: &mut Option<(usize, u64)>,
) -> Result<Option<DispatchTask>, MachineError> {
    let processor_count = state.local.len();
    if processor_index >= processor_count {
        return Err(MachineError::SchedulerQueue(format!(
            "processor index {processor_index} exceeds scheduler queue count {processor_count}"
        )));
    }
    if let Some(task) = state.local[processor_index].pop_front() {
        record_selection(state, processor_index, &task);
        return Ok(Some(task));
    }

    if !state.global.is_empty() {
        refill_local_from_global(state, processor_index)?;
        if let Some(task) = state.local[processor_index].pop_front() {
            record_selection(state, processor_index, &task);
            return Ok(Some(task));
        }
    }
    if !state.injection.is_empty() {
        refill_local_from_injection(state, processor_index);
        if let Some(task) = state.local[processor_index].pop_front() {
            record_selection(state, processor_index, &task);
            return Ok(Some(task));
        }
    }
    #[cfg(all(feature = "scheduler-diagnostics", feature = "scheduler-benchmark"))]
    let stole = if state.stealing_enabled {
        steal_work(state, processor_index, stolen_out)
    } else {
        false
    };
    #[cfg(all(
        feature = "scheduler-diagnostics",
        not(feature = "scheduler-benchmark")
    ))]
    let stole = steal_work(state, processor_index, stolen_out);
    #[cfg(all(
        not(feature = "scheduler-diagnostics"),
        feature = "scheduler-benchmark"
    ))]
    let stole = if state.stealing_enabled {
        steal_work(state, processor_index)
    } else {
        false
    };
    #[cfg(all(
        not(feature = "scheduler-diagnostics"),
        not(feature = "scheduler-benchmark")
    ))]
    let stole = steal_work(state, processor_index);
    if stole {
        if let Some(task) = state.local[processor_index].pop_front() {
            record_selection(state, processor_index, &task);
            return Ok(Some(task));
        }
    }

    let task = state
        .global
        .pop_front()
        .or_else(|| state.injection.pop_front());
    if let Some(task) = &task {
        record_selection(state, processor_index, task);
    }
    Ok(task)
}

#[cfg(feature = "scheduler-benchmark")]
fn record_selection(state: &mut SchedulerState, processor_index: usize, task: &DispatchTask) {
    if !state.benchmark_trace_enabled {
        return;
    }
    state.benchmark_steal_events.push(StealEvent::Selected {
        routine_id: task.benchmark_routine_id().get(),
        processor_index,
    });
}

#[cfg(not(feature = "scheduler-benchmark"))]
fn record_selection(_state: &mut SchedulerState, _processor_index: usize, _task: &DispatchTask) {}
