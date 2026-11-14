use std::error::Error;
use std::fmt;
use std::io;

use super::MachineId;

/// Failure while creating, observing, or stopping a Machine worker.
#[derive(Debug)]
pub enum MachineError {
    /// The Processor pool contains no execution capacity.
    EmptyProcessorPool,
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
    },
    /// The worker exited with an error.
    WorkerFailed {
        /// Worker that exited unexpectedly.
        machine_id: MachineId,
        /// Failure details.
        message: String,
    },
}

impl fmt::Display for MachineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProcessorPool => {
                formatter.write_str("Machine manager requires at least one Processor")
            }
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
            Self::WorkerPanicked { machine_id } => {
                write!(formatter, "Machine {} worker panicked", machine_id.index())
            }
            Self::WorkerFailed {
                machine_id,
                message,
            } => write!(
                formatter,
                "Machine {} worker failed: {message}",
                machine_id.index()
            ),
        }
    }
}

impl Error for MachineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ThreadSpawn { source, .. } => Some(source),
            Self::EmptyProcessorPool
            | Self::StartupCleanup { .. }
            | Self::ControlPoisoned { .. }
            | Self::WorkerPanicked { .. }
            | Self::WorkerFailed { .. } => None,
        }
    }
}
