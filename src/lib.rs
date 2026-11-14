//! Rust GMP runtime building blocks.

mod context;
/// Native worker Machines and their Processor lifecycle.
pub mod machine;
/// Logical Processor capacities and their local runnable queues.
pub mod processor;
/// Lifecycle and identity types for stackful execution routines.
pub mod routine;
/// Stable virtual-memory stacks with protected guard regions.
pub mod stack;
