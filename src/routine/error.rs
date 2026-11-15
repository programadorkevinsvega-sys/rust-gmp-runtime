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
    /// The stack bounds are inverted or empty.
    InvalidContextStackBounds,
    /// The usable stack cannot satisfy the initial ABI stack layout.
    InsufficientContextStack,
    /// Context switching is unavailable on the current operating system/CPU.
    UnsupportedContextArchitecture,
    /// A routine context has already been initialized and cannot be restarted.
    ContextAlreadyStarted,
    /// Context details are unavailable while the routine is executing.
    ContextUnavailableWhileRunning,
    /// A stack lease cannot be returned to a Processor cache before completion.
    StackReleaseBeforeCompletion {
        /// Lifecycle state observed when the release was requested.
        state: RoutineState,
    },
    /// A scheduling counter cannot be incremented without wrapping.
    SchedulingCounterOverflow,
    /// The routine is reserved by a worker dispatch and cannot be claimed externally.
    DispatchReserved,
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
            Self::InvalidContextStackBounds => {
                formatter.write_str("routine context stack bounds are empty or inverted")
            }
            Self::InsufficientContextStack => {
                formatter.write_str("routine stack is too small for the initial ABI context")
            }
            Self::UnsupportedContextArchitecture => {
                formatter.write_str("context switching is unsupported on this target")
            }
            Self::ContextAlreadyStarted => {
                formatter.write_str("routine context has already been started")
            }
            Self::ContextUnavailableWhileRunning => {
                formatter.write_str("routine context cannot be inspected while it is running")
            }
            Self::StackReleaseBeforeCompletion { state } => write!(
                formatter,
                "routine stack cannot be returned before completion (state: {state:?})"
            ),
            Self::SchedulingCounterOverflow => {
                formatter.write_str("routine scheduling counter overflowed")
            }
            Self::DispatchReserved => {
                formatter.write_str("routine is reserved for worker dispatch")
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
            | Self::InvalidContextStackBounds
            | Self::InsufficientContextStack
            | Self::UnsupportedContextArchitecture
            | Self::ContextAlreadyStarted
            | Self::ContextUnavailableWhileRunning
            | Self::StackReleaseBeforeCompletion { .. }
            | Self::SchedulingCounterOverflow
            | Self::DispatchReserved
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

/// Panic payload produced by a routine entry under `panic = "unwind"`.
pub type RoutinePanicPayload = Box<dyn std::any::Any + Send + 'static>;

/// Outcome error returned when a routine panics or runtime setup fails.
pub enum RoutineRunError {
    /// The runtime rejected setup before transferring control to the routine.
    Runtime(RoutineError),
    /// The routine panicked; ownership of the original payload is returned.
    Panicked(RoutinePanicPayload),
}

impl fmt::Debug for RoutineRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => formatter.debug_tuple("Runtime").field(error).finish(),
            Self::Panicked(_) => formatter.write_str("Panicked(..)"),
        }
    }
}

impl fmt::Display for RoutineRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "routine runtime failure: {error}"),
            Self::Panicked(_) => formatter.write_str("routine panicked"),
        }
    }
}

impl Error for RoutineRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Panicked(_) => None,
        }
    }
}

impl From<RoutineError> for RoutineRunError {
    fn from(error: RoutineError) -> Self {
        Self::Runtime(error)
    }
}
