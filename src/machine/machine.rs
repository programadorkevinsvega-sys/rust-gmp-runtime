use std::thread::ThreadId;

use crate::processor::ProcessorId;
use crate::routine::RoutineId;

use super::MachineId;

/// Lifecycle state of a native worker Machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineState {
    /// The OS thread was created and is acquiring its Processor.
    Starting,
    /// The worker is parked and holds its assigned Processor.
    Idle,
    /// The worker is creating a selected routine and acquiring its stack.
    Preparing {
        /// Routine being prepared for dispatch.
        routine_id: RoutineId,
    },
    /// The worker is dispatching this routine on its assigned Processor.
    Running {
        /// Routine currently dispatched by this Machine.
        routine_id: RoutineId,
    },
    /// Shutdown was requested and the worker is releasing its Processor.
    Stopping,
    /// The worker exited and released its Processor.
    Terminated,
    /// Startup or worker control failed.
    Failed {
        /// Failure information retained for diagnostics.
        message: String,
    },
}

/// Read-only association and lifecycle snapshot for a worker Machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineSnapshot {
    /// Stable manager-local Machine identifier.
    pub id: MachineId,
    /// Processor assigned to this worker for its lifetime.
    pub processor_id: ProcessorId,
    /// Native thread identity once startup has completed.
    pub thread_id: Option<ThreadId>,
    /// Machine identity registered in this native thread's runtime TLS.
    pub tls_machine_id: Option<MachineId>,
    /// Current worker lifecycle state.
    pub state: MachineState,
}
