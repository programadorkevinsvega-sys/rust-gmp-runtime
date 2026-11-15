use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use crate::processor::ProcessorPool;
use crate::scheduler::Scheduler;
use crate::stack::StackManager;

use super::error::MachineError;
use super::representation::MachineSnapshot;
use super::worker::Worker;

#[path = "thread_manager/dispatch.rs"]
mod dispatch;
#[path = "thread_manager/start.rs"]
mod start;

/// Owns worker threads and their Processor associations.
#[derive(Debug)]
pub struct MachineManager {
    processors: Arc<ProcessorPool>,
    scheduler: Arc<Scheduler>,
    workers: Vec<Worker>,
    next_worker: AtomicUsize,
    stack_manager: Option<Arc<StackManager>>,
    processor_caches: bool,
}

impl MachineManager {
    /// Returns snapshots of all managed workers in stable Machine ID order.
    ///
    /// # Errors
    ///
    /// Returns an error if a worker control lock is poisoned.
    pub fn machines(&self) -> Result<Vec<MachineSnapshot>, MachineError> {
        self.workers.iter().map(Worker::snapshot).collect()
    }

    /// Returns a point-in-time scheduler diagnostic snapshot.
    ///
    /// Queue lengths are instantaneous; activity counters are cumulative since
    /// this manager was started.
    ///
    /// # Errors
    ///
    /// Returns an error if the scheduler state lock is poisoned.
    pub fn scheduler_stats(&self) -> Result<crate::metrics::SchedulerStats, MachineError> {
        self.scheduler.stats()
    }

    /// Returns opt-in, per-Processor scheduler diagnostic counters.
    ///
    /// This API is available only with the `scheduler-diagnostics` feature.
    /// Measurements are accumulated in cache-padded, per-Processor counters
    /// after releasing the scheduler queue lock. Timing calls still perturb the
    /// instrumented run, so use it for diagnosis, not performance comparisons.
    /// Fields are loaded independently; a live snapshot is not transactional.
    #[cfg(feature = "scheduler-diagnostics")]
    pub fn scheduler_diagnostics(&self) -> crate::metrics::SchedulerDiagnostics {
        self.scheduler.diagnostics()
    }

    /// Enables or disables work stealing for controlled scheduler benchmarks.
    ///
    /// This method is available only with `scheduler-benchmark`; production
    /// scheduler policy is unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if the scheduler state lock is poisoned.
    #[cfg(feature = "scheduler-benchmark")]
    #[doc(hidden)]
    pub fn benchmark_set_stealing_enabled(&self, enabled: bool) -> Result<(), MachineError> {
        self.scheduler
            .benchmark_set_stealing_enabled(enabled)
            .map_err(MachineError::SchedulerQueue)
    }

    /// Requests worker shutdown and joins every native thread.
    ///
    /// Repeated calls are safe. All workers are joined even if an earlier
    /// worker reports an error.
    ///
    /// # Errors
    ///
    /// Returns the first stop, panic, or worker failure after attempting all
    /// joins.
    pub fn shutdown(&mut self) -> Result<(), MachineError> {
        let mut first_error = None;
        if let Err(error) = self.scheduler.close() {
            record_first(&mut first_error, error);
        }
        for worker in &self.workers {
            match worker.request_stop() {
                Ok(true) => {
                    if let Err(error) = self.scheduler.record_worker_state_transition() {
                        record_first(&mut first_error, error);
                    }
                }
                Ok(false) => {}
                Err(error) => record_first(&mut first_error, error),
            }
        }
        for worker in &mut self.workers {
            if let Err(error) = worker.join() {
                record_first(&mut first_error, error);
            }
        }
        loop {
            match self.scheduler.take_pending_for_shutdown() {
                Ok(Some(task)) => {
                    if let Err(error) = task.reject_after_shutdown() {
                        record_first(&mut first_error, error);
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    record_first(&mut first_error, error);
                    break;
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn rollback(&mut self, startup: MachineError) -> MachineError {
        match self.shutdown() {
            Ok(()) => startup,
            Err(cleanup) => MachineError::StartupCleanup {
                startup: startup.to_string(),
                cleanup: cleanup.to_string(),
            },
        }
    }
}

impl Drop for MachineManager {
    fn drop(&mut self) {
        if self.workers.iter().any(Worker::is_joinable) {
            if let Err(error) = self.shutdown() {
                eprintln!("error: MachineManager shutdown during drop failed: {error}");
            }
        }
    }
}

fn record_first(first: &mut Option<MachineError>, error: MachineError) {
    if first.is_none() {
        *first = Some(error);
    }
}
