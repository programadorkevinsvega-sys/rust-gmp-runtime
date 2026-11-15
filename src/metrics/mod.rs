//! Diagnostic snapshots exposed by the runtime.

#[cfg(feature = "scheduler-diagnostics")]
mod scheduler_diagnostics;
mod scheduler_stats;

#[cfg(feature = "scheduler-diagnostics")]
pub(crate) use scheduler_diagnostics::{ProcessorCounters, SchedulerDiagnosticsStore};
#[cfg(feature = "scheduler-diagnostics")]
pub use scheduler_diagnostics::{ProcessorSchedulerDiagnostics, SchedulerDiagnostics};
pub use scheduler_stats::SchedulerStats;
