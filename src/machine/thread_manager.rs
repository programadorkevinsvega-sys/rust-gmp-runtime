use std::panic::UnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::processor::ProcessorPool;
use crate::routine::Routine;

use super::error::MachineError;
use super::identity::MachineId;
use super::join::RoutineJoinHandle;
use super::representation::MachineSnapshot;
use super::worker::Worker;

/// Owns worker threads and their Processor associations.
#[derive(Debug)]
pub struct MachineManager {
    processors: Arc<ProcessorPool>,
    workers: Vec<Worker>,
    next_worker: AtomicUsize,
}

impl MachineManager {
    /// Starts one parked native worker for each Processor in the pool.
    ///
    /// Startup is transactional: if spawning or binding any worker fails,
    /// already-started workers are stopped and joined before returning.
    ///
    /// # Errors
    ///
    /// Returns an error on thread creation, Processor binding, worker startup,
    /// or rollback failure.
    pub fn start(processors: Arc<ProcessorPool>) -> Result<Self, MachineError> {
        if processors.is_empty() {
            return Err(MachineError::EmptyProcessorPool);
        }
        let mut manager = Self {
            processors,
            workers: Vec::new(),
            next_worker: AtomicUsize::new(0),
        };
        let processor_ids = manager
            .processors
            .iter()
            .map(|processor| processor.id())
            .collect::<Vec<_>>();
        for processor_id in processor_ids {
            let id = MachineId::new(manager.workers.len());
            let worker = match Worker::spawn(id, processor_id, Arc::clone(&manager.processors)) {
                Ok(worker) => worker,
                Err(error) => return Err(manager.rollback(error)),
            };
            manager.workers.push(worker);
            let startup = match manager.workers.last() {
                Some(worker) => worker.wait_started(),
                None => Err(MachineError::EmptyProcessorPool),
            };
            if let Err(error) = startup {
                return Err(manager.rollback(error));
            }
        }
        Ok(manager)
    }

    /// Publishes and dispatches a routine to one managed worker.
    ///
    /// The routine executes once and returns to the worker scheduler stack.
    /// This does not provide yield or context resumption.
    ///
    /// # Errors
    ///
    /// Returns an error if the routine cannot be published, there are no
    /// workers, or the selected worker no longer accepts commands.
    pub fn dispatch<F>(&self, routine: Routine, entry: F) -> Result<RoutineJoinHandle, MachineError>
    where
        F: FnOnce() + Send + UnwindSafe + 'static,
    {
        if self.workers.is_empty() {
            return Err(MachineError::EmptyProcessorPool);
        }
        routine.publish()?;
        let worker_index = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let worker = &self.workers[worker_index];
        match worker.dispatch(routine, entry) {
            Ok(handle) => Ok(handle),
            Err((error, routine)) => match routine.discard() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(MachineError::DispatchRejected {
                    machine_id: worker.id,
                    routine_id: routine.id(),
                    message: format!("{error}; routine rollback failed: {cleanup}"),
                }),
            },
        }
    }

    /// Returns snapshots of all managed workers in stable Machine ID order.
    ///
    /// # Errors
    ///
    /// Returns an error if a worker control lock is poisoned.
    pub fn machines(&self) -> Result<Vec<MachineSnapshot>, MachineError> {
        self.workers.iter().map(Worker::snapshot).collect()
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
        for worker in &self.workers {
            if let Err(error) = worker.request_stop() {
                record_first(&mut first_error, error);
            }
        }
        for worker in &mut self.workers {
            if let Err(error) = worker.join() {
                record_first(&mut first_error, error);
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
