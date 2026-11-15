#[cfg(feature = "scheduler-benchmark")]
use crate::machine::worker::dispatch::DispatchTask;
#[cfg(feature = "scheduler-benchmark")]
use crate::queue::LOCAL_QUEUE_CAPACITY;

use super::Scheduler;
#[cfg(feature = "scheduler-benchmark")]
use super::StealEvent;
#[cfg(feature = "scheduler-benchmark")]
use super::work_stealing::push_local;

impl Scheduler {
    #[cfg(feature = "scheduler-benchmark")]
    pub(crate) fn benchmark_publish_local(
        &self,
        processor_index: usize,
        task: DispatchTask,
    ) -> Result<(), (DispatchTask, String)> {
        let mut state = match self.lock() {
            Ok(state) => state,
            Err(message) => return Err((task, message)),
        };
        if processor_index >= state.local.len() {
            return Err((task, format!("invalid local queue index {processor_index}")));
        }
        if state.local[processor_index].remaining_capacity() == 0 {
            return Err((task, "benchmark local queue is at capacity".to_owned()));
        }
        push_local(&mut state, processor_index, task);
        state.published_tasks = state.published_tasks.saturating_add(1);
        Ok(())
    }

    #[cfg(feature = "scheduler-benchmark")]
    pub(crate) fn benchmark_configure_stealing(
        &self,
        steal_batch: usize,
        trace_capacity: usize,
    ) -> Result<(), String> {
        if !(1..=LOCAL_QUEUE_CAPACITY).contains(&steal_batch) {
            return Err(format!(
                "benchmark steal batch must be between 1 and {LOCAL_QUEUE_CAPACITY}"
            ));
        }
        let mut state = self.lock()?;
        state
            .benchmark_steal_events
            .try_reserve(trace_capacity)
            .map_err(|error| format!("cannot reserve steal trace: {error}"))?;
        state.steal_batch = steal_batch;
        state.benchmark_trace_enabled = trace_capacity > 0;
        Ok(())
    }

    #[cfg(feature = "scheduler-benchmark")]
    pub(crate) fn benchmark_steal_events(&self) -> Result<Vec<StealEvent>, String> {
        let state = self.lock()?;
        Ok(state.benchmark_steal_events.clone())
    }

    #[cfg(feature = "scheduler-benchmark")]
    pub(crate) fn benchmark_set_stealing_enabled(&self, enabled: bool) -> Result<(), String> {
        let mut state = self.lock()?;
        state.stealing_enabled = enabled;
        Ok(())
    }
}
