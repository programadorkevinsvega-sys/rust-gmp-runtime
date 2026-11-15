use std::panic::UnwindSafe;
use std::sync::atomic::Ordering;

use crate::routine::{Routine, RoutineId, RoutineMetadata};

use super::MachineManager;
use crate::machine::{MachineError, RoutineJoinHandle};

impl MachineManager {
    /// Publishes a routine with a round-robin origin and notifies a waiting worker.
    ///
    /// The scheduler may select any worker for execution after publication.
    /// The routine executes once and returns to the worker scheduler stack;
    /// this does not provide yield or context resumption.
    ///
    /// # Errors
    ///
    /// Returns an error if the routine cannot be published, there are no
    /// workers, or scheduler admission is closed or cannot reserve queue space.
    pub fn dispatch<F>(&self, routine: Routine, entry: F) -> Result<RoutineJoinHandle, MachineError>
    where
        F: FnOnce() + Send + UnwindSafe + 'static,
    {
        if self.workers.is_empty() {
            return Err(MachineError::EmptyProcessorPool);
        }
        routine.reserve_for_dispatch()?;
        let worker_index = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let worker = &self.workers[worker_index];
        let (task, handle) =
            super::super::worker::dispatch::DispatchTask::new(worker.id, routine, entry);
        match self.scheduler.publish(task, worker.processor_id.index()) {
            Ok(()) => Ok(handle),
            Err((task, message)) => {
                let rollback_error = task.reject(worker.id, message.clone()).err();
                let message = match rollback_error {
                    Some(error) => format!("{message}; {error}"),
                    None => message,
                };
                Err(MachineError::DispatchRejected {
                    machine_id: worker.id,
                    routine_id: handle.routine_id(),
                    message,
                })
            }
        }
    }

    /// Creates and dispatches a routine on its selected worker.
    ///
    /// With [`Self::start_with_processor_caches`], both stack acquisition and
    /// release use that worker's Processor cache. With
    /// [`Self::start_with_stack_manager`], allocation uses the manager's global
    /// stack path. The routine executes exactly once and can be joined through
    /// the returned handle.
    ///
    /// # Errors
    ///
    /// Returns an error if no stack manager or worker is configured, or if
    /// scheduler admission is closed or cannot reserve queue space.
    pub fn dispatch_new<F>(
        &self,
        metadata: RoutineMetadata,
        entry: F,
    ) -> Result<RoutineJoinHandle, MachineError>
    where
        F: FnOnce() + Send + UnwindSafe + 'static,
    {
        if self.workers.is_empty() {
            return Err(MachineError::EmptyProcessorPool);
        }
        if self.stack_manager.is_none() {
            return Err(MachineError::StackManagerNotConfigured);
        }
        let routine_id = RoutineId::allocate().map_err(crate::routine::RoutineError::from)?;
        let worker_index = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let worker = &self.workers[worker_index];
        let (task, handle) = super::super::worker::dispatch::DispatchTask::new_on_worker(
            worker.id,
            routine_id,
            metadata,
            entry,
            self.processor_caches,
        );
        match self.scheduler.publish(task, worker.processor_id.index()) {
            Ok(()) => Ok(handle),
            Err((task, message)) => {
                let rollback_error = task.reject(worker.id, message.clone()).err();
                let message = match rollback_error {
                    Some(error) => format!("{message}; {error}"),
                    None => message,
                };
                Err(MachineError::DispatchRejected {
                    machine_id: worker.id,
                    routine_id: handle.routine_id(),
                    message,
                })
            }
        }
    }
}
