use crate::machine::MachineError;
use crate::machine::worker::dispatch::DispatchTask;

#[cfg(not(feature = "scheduler-benchmark"))]
use super::STEAL_BATCH;
use super::SchedulerState;
#[cfg(feature = "scheduler-benchmark")]
use super::StealEvent;
use super::TRANSFER_BATCH;

impl SchedulerState {
    pub(super) fn local_capacity(&self, processor_index: usize) -> usize {
        self.local[processor_index].remaining_capacity()
    }
}

pub(super) fn refill_local_from_global(
    state: &mut SchedulerState,
    processor_index: usize,
) -> Result<(), MachineError> {
    let count = TRANSFER_BATCH
        .min(state.local_capacity(processor_index))
        .min(state.global.len());
    for _ in 0..count {
        let task = state.global.pop_front().ok_or_else(|| {
            MachineError::SchedulerQueue("global queue changed during transfer".to_owned())
        })?;
        push_local(state, processor_index, task);
    }
    Ok(())
}

pub(super) fn refill_local_from_injection(state: &mut SchedulerState, processor_index: usize) {
    let count = TRANSFER_BATCH
        .min(state.local_capacity(processor_index))
        .min(state.injection.len());
    for _ in 0..count {
        if let Some(task) = state.injection.pop_front() {
            push_local(state, processor_index, task);
        }
    }
}

pub(super) fn steal_work(
    state: &mut SchedulerState,
    thief: usize,
    #[cfg(feature = "scheduler-diagnostics")] stolen_out: &mut Option<(usize, u64)>,
) -> bool {
    state.steal_attempts = state.steal_attempts.saturating_add(1);
    if state.local[thief].remaining_capacity() == 0 {
        state.failed_steal_attempts = state.failed_steal_attempts.saturating_add(1);
        return false;
    }
    let processor_count = state.local.len();
    #[cfg(feature = "scheduler-benchmark")]
    let steal_batch = state.steal_batch;
    #[cfg(not(feature = "scheduler-benchmark"))]
    let steal_batch = STEAL_BATCH;
    for offset in 1..processor_count {
        let victim = (thief + offset) % processor_count;
        let victim_len = state.local[victim].len();
        if victim_len == 0 {
            continue;
        }
        let count = victim_len
            .div_ceil(2)
            .min(steal_batch)
            .min(state.local[thief].remaining_capacity());
        for _ in 0..count {
            if let Some(task) = state.local[victim].pop_back() {
                #[cfg(feature = "scheduler-benchmark")]
                if state.benchmark_trace_enabled {
                    state.benchmark_steal_events.push(StealEvent::Transferred {
                        routine_id: task.benchmark_routine_id().get(),
                        from_processor: victim,
                        to_processor: thief,
                    });
                }
                push_local(state, thief, task);
                state.stolen_tasks = state.stolen_tasks.saturating_add(1);
            }
        }
        #[cfg(feature = "scheduler-diagnostics")]
        if count > 0 {
            *stolen_out = Some((victim, count as u64));
        }
        return count > 0;
    }
    state.failed_steal_attempts = state.failed_steal_attempts.saturating_add(1);
    false
}

pub(super) fn push_local(state: &mut SchedulerState, processor_index: usize, task: DispatchTask) {
    state.local[processor_index].push_reserved(task);
    state.peak_local_queue_lengths[processor_index] =
        state.peak_local_queue_lengths[processor_index].max(state.local[processor_index].len());
}
