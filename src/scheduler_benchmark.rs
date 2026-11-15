//! Benchmark-only helpers for deterministic scheduler queue experiments.

use std::error::Error;

use crate::stack::StackManager;

mod experiment;
mod trace;
mod types;

pub use experiment::run_imbalanced_steal_configured;
pub use types::{StealSample, TaskTransferTrace};

/// Runs a scheduler-only experiment with one loaded local queue and idle thieves.
///
/// The task count must be a positive multiple of the local queue capacity.
/// Processor zero is deliberately left without a worker; all other Processors
/// start empty and compete to steal its queued work. This isolates queue
/// selection and task transfer; it does not execute routine closures.
///
/// # Errors
///
/// Returns an error if arguments are invalid, routine allocation or queue
/// publication fails, scheduler accounting is inconsistent, or a worker
/// thread panics.
pub fn run_imbalanced_steal(
    processor_count: usize,
    task_count: usize,
    stack_manager: &StackManager,
) -> Result<StealSample, Box<dyn Error>> {
    experiment::run_imbalanced_steal_configured(
        processor_count,
        task_count,
        experiment::STEAL_BATCH,
        false,
        stack_manager,
    )
}
