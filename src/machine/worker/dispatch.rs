use std::panic::UnwindSafe;
use std::sync::mpsc::{self, Sender};

use crate::routine::{Routine, RoutineId, RoutineMetadata};

use super::super::identity::MachineId;
use super::super::join::RoutineJoinError;

#[path = "dispatch/deferred.rs"]
mod deferred;

pub(crate) struct DispatchTask {
    machine_id: MachineId,
    routine: Option<Routine>,
    routine_id: RoutineId,
    pending_routine: Option<RoutineMetadata>,
    return_stack_to_cache: bool,
    entry: Option<Box<dyn FnOnce() + Send + UnwindSafe>>,
    completion: Option<Sender<Result<(), RoutineJoinError>>>,
}

impl DispatchTask {
    pub(crate) fn new<F>(
        machine_id: MachineId,
        routine: Routine,
        entry: F,
    ) -> (Self, crate::machine::RoutineJoinHandle)
    where
        F: FnOnce() + Send + UnwindSafe + 'static,
    {
        let (completion, receiver) = mpsc::channel();
        let routine_id = routine.id();
        (
            Self {
                machine_id,
                routine: Some(routine),
                routine_id,
                pending_routine: None,
                return_stack_to_cache: false,
                entry: Some(Box::new(entry)),
                completion: Some(completion),
            },
            crate::machine::RoutineJoinHandle::new(machine_id, routine_id, receiver),
        )
    }

    pub(crate) fn take_entry(&mut self) -> Option<Box<dyn FnOnce() + Send + UnwindSafe>> {
        self.entry.take()
    }

    #[cfg(feature = "scheduler-benchmark")]
    pub(crate) fn benchmark_routine_id(&self) -> crate::routine::RoutineId {
        self.routine_id
    }

    pub(crate) fn routine(&self) -> Option<&Routine> {
        self.routine.as_ref()
    }

    pub(crate) fn routine_id(&self) -> RoutineId {
        self.routine_id
    }

    pub(crate) fn complete(mut self, outcome: Result<(), RoutineJoinError>) {
        if let Some(routine) = self.routine.take() {
            drop(routine);
        }
        self.send_completion(outcome);
    }

    pub(crate) fn fail(mut self, machine_id: MachineId, message: String) {
        self.fail_if_pending(machine_id, message);
    }

    pub(crate) fn reject(mut self, machine_id: MachineId, message: String) -> Result<(), String> {
        let (message, rollback_error) = match self.routine.as_ref() {
            Some(routine) => match routine.abandon_dispatch() {
                Ok(true) => (message, None),
                Ok(false) => {
                    let error = "dispatch reservation was already consumed".to_owned();
                    (format!("{message}; {error}"), Some(error))
                }
                Err(error) => {
                    let detail = error.to_string();
                    (
                        format!("{message}; routine rollback failed: {detail}"),
                        Some(detail),
                    )
                }
            },
            None if self.pending_routine.is_some() => (message, None),
            None => {
                let error = "dispatch task no longer owns its routine".to_owned();
                (format!("{message}; {error}"), Some(error))
            }
        };
        if let Some(routine) = self.routine.take() {
            drop(routine);
        }
        self.send_completion(Err(RoutineJoinError::WorkerFailed {
            machine_id,
            routine_id: self.routine_id,
            message,
        }));
        match rollback_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn reject_after_shutdown(self) -> Result<(), super::super::error::MachineError> {
        let machine_id = self.machine_id;
        let routine_id = self.routine_id;
        let message = "no worker remained to execute accepted work during shutdown".to_owned();
        self.reject(machine_id, message.clone())
            .map_err(
                |rollback| super::super::error::MachineError::DispatchRejected {
                    machine_id,
                    routine_id,
                    message: format!("{message}; {rollback}"),
                },
            )
    }

    fn send_completion(&mut self, outcome: Result<(), RoutineJoinError>) {
        if let Some(completion) = self.completion.take() {
            if let Err(error) = completion.send(outcome) {
                drop(error.0);
            }
        }
    }

    fn fail_if_pending(&mut self, machine_id: MachineId, message: String) {
        if self.completion.is_none() {
            return;
        }
        let message = match self.routine.as_ref() {
            Some(routine) => match routine.abandon_dispatch() {
                Ok(true) | Ok(false) => message,
                Err(error) => format!("{message}; routine rollback failed: {error}"),
            },
            None => message,
        };
        if let Some(routine) = self.routine.take() {
            drop(routine);
        }
        self.send_completion(Err(RoutineJoinError::WorkerFailed {
            machine_id,
            routine_id: self.routine_id,
            message,
        }));
    }
}

impl Drop for DispatchTask {
    fn drop(&mut self) {
        self.fail_if_pending(
            self.machine_id,
            "worker exited before dispatch completed".into(),
        );
    }
}

#[cfg(test)]
#[path = "dispatch/tests.rs"]
mod tests;
