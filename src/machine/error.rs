use std::error::Error;
use std::fmt;
use std::io;

use super::MachineId;
use crate::routine::{RoutineError, RoutineId};

/// Failure while creating, observing, or stopping a Machine worker.
#[derive(Debug)]
pub enum MachineError {
    /// The Processor pool contains no execution capacity.
    EmptyProcessorPool,
    /// Deferred routine dispatch requires a configured stack manager.
    StackManagerNotConfigured,
    /// Per-Processor stack caches require the Go-inspired stack engine.
    ProcessorCachesRequireGoInspired,
    /// A routine lifecycle transition failed during dispatch admission.
    Routine(RoutineError),
    /// A legacy worker wake channel is no longer available.
    WorkerChannelClosed {
        /// Worker that could not receive the command.
        machine_id: MachineId,
        /// Channel send failure details.
        message: String,
    },
    /// A routine could not be dispatched and rolled back to a terminal state.
    DispatchRejected {
        /// Worker selected for the initial wake-up.
        machine_id: MachineId,
        /// Routine whose dispatch failed.
        routine_id: RoutineId,
        /// Dispatch and rollback failure details.
        message: String,
    },
    /// The OS rejected creation of a worker thread.
    ThreadSpawn {
        /// Identity assigned to the worker that could not start.
        machine_id: MachineId,
        /// OS thread creation failure.
        source: io::Error,
    },
    /// Startup failed and cleanup of already-started workers also failed.
    StartupCleanup {
        /// Original startup failure.
        startup: String,
        /// Worker stop/join failure during rollback.
        cleanup: String,
    },
    /// A worker's lifecycle lock or condition variable was poisoned.
    ControlPoisoned {
        /// Worker whose control state could not be used.
        machine_id: MachineId,
        /// Poisoning details.
        message: String,
    },
    /// A worker panicked before its join completed.
    WorkerPanicked {
        /// Worker whose thread panicked.
        machine_id: MachineId,
        /// Panic payload summary returned by the native thread join.
        message: String,
    },
    /// The worker exited with an error.
    WorkerFailed {
        /// Worker that exited unexpectedly.
        machine_id: MachineId,
        /// Failure details.
        message: String,
    },
    /// The scheduler could not preserve a queue operation.
    SchedulerQueue(String),
}

impl fmt::Display for MachineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProcessorPool => {
                formatter.write_str("Machine manager requires at least one Processor")
            }
            Self::StackManagerNotConfigured => {
                formatter.write_str("Machine manager has no configured stack manager")
            }
            Self::ProcessorCachesRequireGoInspired => {
                formatter.write_str("Processor stack caches require the Go-inspired engine")
            }
            Self::Routine(error) => write!(formatter, "routine dispatch admission failed: {error}"),
            Self::WorkerChannelClosed {
                machine_id,
                message,
            } => write!(
                formatter,
                "Machine {} is no longer accepting work: {message}",
                machine_id.index(),
            ),
            Self::DispatchRejected {
                machine_id,
                routine_id,
                message,
            } => write!(
                formatter,
                "Machine {} rejected routine {}: {message}",
                machine_id.index(),
                routine_id.get()
            ),
            Self::ThreadSpawn { machine_id, source } => {
                write!(
                    formatter,
                    "could not spawn Machine {}: {source}",
                    machine_id.index()
                )
            }
            Self::StartupCleanup { startup, cleanup } => write!(
                formatter,
                "Machine startup failed ({startup}); worker cleanup also failed ({cleanup})"
            ),
            Self::ControlPoisoned {
                machine_id,
                message,
            } => write!(
                formatter,
                "Machine {} control state was poisoned: {message}",
                machine_id.index()
            ),
            Self::WorkerPanicked {
                machine_id,
                message,
            } => write!(
                formatter,
                "Machine {} worker panicked: {message}",
                machine_id.index()
            ),
            Self::WorkerFailed {
                machine_id,
                message,
            } => write!(
                formatter,
                "Machine {} worker failed: {message}",
                machine_id.index()
            ),
            Self::SchedulerQueue(message) => {
                write!(formatter, "scheduler queue operation failed: {message}")
            }
        }
    }
}

impl Error for MachineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ThreadSpawn { source, .. } => Some(source as &(dyn Error + 'static)),
            Self::Routine(error) => Some(error as &(dyn Error + 'static)),
            Self::EmptyProcessorPool
            | Self::StackManagerNotConfigured
            | Self::ProcessorCachesRequireGoInspired
            | Self::WorkerChannelClosed { .. }
            | Self::DispatchRejected { .. }
            | Self::StartupCleanup { .. }
            | Self::ControlPoisoned { .. }
            | Self::WorkerPanicked { .. }
            | Self::WorkerFailed { .. }
            | Self::SchedulerQueue(_) => None,
        }
    }
}

impl From<RoutineError> for MachineError {
    fn from(error: RoutineError) -> Self {
        Self::Routine(error)
    }
}
