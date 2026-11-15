//! F5.4 throughput and publication-to-start latency characterization.

use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::metrics::SchedulerStats;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

#[path = "scheduler/execution.rs"]
mod execution;
#[path = "scheduler/runner.rs"]
mod runner;
#[path = "scheduler/support.rs"]
mod support;
#[path = "scheduler/types.rs"]
mod types;

fn main() -> Result<(), Box<dyn Error>> {
    runner::run()
}
