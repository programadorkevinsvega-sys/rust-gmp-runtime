use std::error::Error;
use std::fmt;

use super::id::RoutineIdError;
use super::state::RoutineState;
use crate::stack::StackError;

/// Failure while creating or advancing a routine's lifecycle.
#[derive(Debug)]
pub enum RoutineError {
    /// The process-wide routine identifier space is exhausted.
    Identifier(RoutineIdError),
    /// Stack allocation or configuration failed.
    Stack(StackError),
    /// The routine lifecycle lock was poisoned by a panic.
    StateLockPoisoned {
        /// Details from the poisoned lifecycle mutex.
        message: String,
    },
    /// The routine stack lock was poisoned by a panic.
    StackLockPoisoned {
        /// Details from the poisoned stack mutex.
        message: String,
    },
    /// The routine context lock was poisoned by a panic.
    ContextLockPoisoned {
        /// Details from the poisoned context mutex.
        message: String,
    },
    /// The stack upper bound cannot represent a nonzero address.
    InvalidContextStackBound {
        /// Rejected address value.
        value: usize,
    },
    /// A scheduling counter cannot be incremented without wrapping.
    SchedulingCounterOverflow,
    /// The requested lifecycle transition is not permitted.
    InvalidTransition {
        /// State observed while holding the lifecycle lock.
        current: RoutineState,
        /// State requested by the caller.
        requested: RoutineState,
    },
}

impl fmt::Display for RoutineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identifier(error) => write!(formatter, "could not allocate routine id: {error}"),
            Self::Stack(error) => write!(formatter, "could not allocate routine stack: {error}"),
            Self::StateLockPoisoned { message } => {
                write!(formatter, "routine lifecycle lock was poisoned: {message}")
            }
            Self::StackLockPoisoned { message } => {
                write!(formatter, "routine stack lock was poisoned: {message}")
            }
            Self::ContextLockPoisoned { message } => {
                write!(formatter, "routine context lock was poisoned: {message}")
            }
            Self::InvalidContextStackBound { value } => {
                write!(formatter, "invalid routine context stack bound {value}")
            }
            Self::SchedulingCounterOverflow => {
                formatter.write_str("routine scheduling counter overflowed")
            }
            Self::InvalidTransition { current, requested } => {
                write!(
                    formatter,
                    "invalid routine transition from {current:?} to {requested:?}"
                )
            }
        }
    }
}

impl Error for RoutineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Identifier(error) => Some(error),
            Self::Stack(error) => Some(error),
            Self::StateLockPoisoned { .. }
            | Self::StackLockPoisoned { .. }
            | Self::ContextLockPoisoned { .. }
            | Self::InvalidContextStackBound { .. }
            | Self::SchedulingCounterOverflow
            | Self::InvalidTransition { .. } => None,
        }
    }
}

impl From<RoutineIdError> for RoutineError {
    fn from(error: RoutineIdError) -> Self {
        Self::Identifier(error)
    }
}

impl From<StackError> for RoutineError {
    fn from(error: StackError) -> Self {
        Self::Stack(error)
    }
}
