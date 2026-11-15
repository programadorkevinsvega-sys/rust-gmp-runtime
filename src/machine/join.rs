use std::error::Error;
use std::fmt;
use std::sync::mpsc::Receiver;

use crate::routine::{RoutineId, RoutineRunError};

use super::MachineId;

/// Result of waiting for a dispatched routine.
#[derive(Debug)]
pub enum RoutineJoinError {
    /// The routine's execution returned a runtime error or panic payload.
    Execution(RoutineRunError),
    /// The selected worker encountered an internal dispatch failure.
    WorkerFailed {
        /// Worker that attempted this routine.
        machine_id: MachineId,
        /// Routine associated with the failed dispatch.
        routine_id: RoutineId,
        /// Failure details.
        message: String,
    },
    /// The worker exited before publishing the routine outcome.
    CompletionLost {
        /// Worker assigned to the routine.
        machine_id: MachineId,
        /// Routine whose result was not delivered.
        routine_id: RoutineId,
    },
}

impl fmt::Display for RoutineJoinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execution(error) => write!(formatter, "{error}"),
            Self::WorkerFailed {
                machine_id,
                routine_id,
                message,
            } => write!(
                formatter,
                "Machine {} failed while dispatching routine {}: {message}",
                machine_id.index(),
                routine_id.get()
            ),
            Self::CompletionLost {
                machine_id,
                routine_id,
            } => write!(
                formatter,
                "Machine {} exited before routine {} completed",
                machine_id.index(),
                routine_id.get()
            ),
        }
    }
}

impl Error for RoutineJoinError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Execution(error) => Some(error),
            Self::WorkerFailed { .. } | Self::CompletionLost { .. } => None,
        }
    }
}

/// Wait handle for one routine dispatched to a Machine worker.
#[derive(Debug)]
pub struct RoutineJoinHandle {
    machine_id: MachineId,
    routine_id: RoutineId,
    completion: Receiver<Result<(), RoutineJoinError>>,
}

impl RoutineJoinHandle {
    pub(super) fn new(
        machine_id: MachineId,
        routine_id: RoutineId,
        completion: Receiver<Result<(), RoutineJoinError>>,
    ) -> Self {
        Self {
            machine_id,
            routine_id,
            completion,
        }
    }

    /// Returns the identity of the dispatched routine.
    pub fn routine_id(&self) -> RoutineId {
        self.routine_id
    }

    /// Blocks until the routine completes or reports its execution failure.
    ///
    /// # Errors
    ///
    /// Returns the routine's panic/runtime outcome, a worker failure, or an
    /// explicit completion-loss error if the worker exits unexpectedly.
    pub fn join(self) -> Result<(), RoutineJoinError> {
        match self.completion.recv() {
            Ok(result) => result,
            Err(_) => Err(RoutineJoinError::CompletionLost {
                machine_id: self.machine_id,
                routine_id: self.routine_id,
            }),
        }
    }
}
