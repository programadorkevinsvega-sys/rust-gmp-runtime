//! Measures Go-inspired span trimming after real stackful execution.

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{GoInspiredOptions, StackConfig, StackEngine, StackManager};

#[path = "stack_trim_workers/child.rs"]
mod child;
#[path = "stack_trim_workers/validation.rs"]
mod validation;

const CHILD_FLAG: &str = "--isolated";
const WORKER_COUNTS: [usize; 6] = [1, 2, 4, 8, 12, 16];
const SAMPLE_COUNT: usize = 3;
const TASK_COUNT: usize = 1_024;
const STACK_SIZE: usize = 64 * 1024;
const SLOTS_PER_SPAN: usize = 64;

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == CHILD_FLAG) {
        return child::run_child(&args);
    }
    if !args.is_empty() && args.as_slice() != ["--bench"] {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid arguments").into());
    }

    let samples = env_usize("RUST_GMP_TRIM_SAMPLES", SAMPLE_COUNT)?;
    if samples == 0 {
        return Err("sample count must be nonzero".into());
    }
    println!(
        "stack_trim_workers process_isolated=true engine=GoInspired workers={WORKER_COUNTS:?} samples={samples} tasks={TASK_COUNT} stack_bytes={STACK_SIZE} slots_per_span={SLOTS_PER_SPAN}"
    );
    let executable = std::env::current_exe()?;
    for workers in WORKER_COUNTS {
        for sample in 0..samples {
            let output = Command::new(&executable)
                .arg(CHILD_FLAG)
                .arg(workers.to_string())
                .arg(sample.to_string())
                .output()?;
            forward_output(output, workers, sample)?;
        }
    }
    Ok(())
}

fn forward_output(output: Output, workers: usize, sample: usize) -> Result<(), Box<dyn Error>> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!(
            "isolated {workers}-worker sample {sample} exited with {}",
            output.status
        ))
        .into());
    }
    io::stdout().write_all(&output.stdout)?;
    Ok(())
}

fn env_usize(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}
