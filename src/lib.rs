//! Rust GMP runtime building blocks.

mod context;
/// Native worker Machines and their Processor lifecycle.
pub mod machine;
/// Point-in-time diagnostic snapshots.
pub mod metrics;
/// Logical Processor capacities and their local runnable queues.
pub mod processor;
/// Synchronized runnable work queues used by the scheduler.
pub(crate) mod queue;
/// Lifecycle and identity types for stackful execution routines.
pub mod routine;
/// G-M-P work selection and load balancing.
pub(crate) mod scheduler;
#[cfg(feature = "scheduler-benchmark")]
#[doc(hidden)]
pub mod scheduler_benchmark;
/// Stable virtual-memory stacks with protected guard regions.
pub mod stack;
