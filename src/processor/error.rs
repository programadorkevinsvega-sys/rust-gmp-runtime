use std::error::Error;
use std::fmt;

use crate::routine::{RoutineError, RoutineId, RoutineState};

/// Failure while configuring or using a Processor.
#[derive(Debug)]
pub enum ProcessorError {
    /// A Processor pool cannot contain zero Processors.
    EmptyPool,
    /// The operating system could not report the available parallelism.
    ParallelismUnavailable(std::io::Error),
    /// The Processor is already associated with a thread.
    AlreadyOwned,
    /// The current thread does not own this Processor.
    NotOwner,
    /// A routine was already present in this local queue.
    DuplicateRoutine(RoutineId),
    /// Only published routines can enter a Processor-local queue.
    RoutineNotRunnable {
        /// Identifier of the rejected routine.
        routine_id: RoutineId,
        /// State observed during queue admission.
        state: RoutineState,
    },
    /// Queue storage and its duplicate index no longer agree.
    QueueInvariantViolation(RoutineId),
    /// The Processor state/queue lock was poisoned.
    LockPoisoned(String),
    /// A routine lifecycle query failed.
    Routine(RoutineError),
}

impl fmt::Display for ProcessorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPool => formatter.write_str("Processor pool must not be empty"),
            Self::ParallelismUnavailable(error) => {
                write!(
                    formatter,
                    "could not determine available parallelism: {error}"
                )
            }
            Self::AlreadyOwned => formatter.write_str("Processor is already bound"),
            Self::NotOwner => formatter.write_str("current thread does not own the Processor"),
            Self::DuplicateRoutine(id) => {
                write!(formatter, "routine {} is already queued", id.get())
            }
            Self::RoutineNotRunnable { routine_id, state } => write!(
                formatter,
                "routine {} is {state:?}, not Runnable",
                routine_id.get()
            ),
            Self::QueueInvariantViolation(id) => write!(
                formatter,
                "Processor queue index is inconsistent for routine {}",
                id.get()
            ),
            Self::LockPoisoned(message) => {
                write!(formatter, "Processor state lock was poisoned: {message}")
            }
            Self::Routine(error) => write!(formatter, "could not inspect routine: {error}"),
        }
    }
}

impl Error for ProcessorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ParallelismUnavailable(error) => Some(error),
            Self::Routine(error) => Some(error),
            Self::EmptyPool
            | Self::AlreadyOwned
            | Self::NotOwner
            | Self::DuplicateRoutine(_)
            | Self::RoutineNotRunnable { .. }
            | Self::QueueInvariantViolation(_)
            | Self::LockPoisoned(_) => None,
        }
    }
}

impl From<RoutineError> for ProcessorError {
    fn from(error: RoutineError) -> Self {
        Self::Routine(error)
    }
}
