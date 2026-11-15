//! Identity, metadata, and lifecycle for stackful execution routines.

#[cfg(feature = "runtime-benchmark-instrumentation")]
mod benchmark;
mod context_storage;
mod error;
mod handle;
mod id;
mod lifecycle;
mod metadata;
mod permit;
mod scheduling;
mod state;

#[cfg(feature = "runtime-benchmark-instrumentation")]
pub use benchmark::RoutineBenchmarkTiming;
pub use context_storage::RoutineContextInfo;
pub use error::{RoutineError, RoutinePanicPayload, RoutineRunError};
pub use handle::Routine;
pub use id::{RoutineId, RoutineIdError};
pub use metadata::RoutineMetadata;
pub use permit::ExecutionPermit;
pub use scheduling::{BlockingReason, RoutineBlockingInfo, RoutineSchedulingInfo, WaitReason};
pub use state::{RoutineCompletion, RoutineState};
