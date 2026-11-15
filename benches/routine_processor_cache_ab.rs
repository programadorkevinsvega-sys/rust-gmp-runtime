//! Compares stackful Routine execution with global and Processor-owned caches.

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Output};

#[path = "routine_processor_cache_ab/mod.rs"]
mod routine_processor_cache_ab;

const CHILD_FLAG: &str = "--isolated";
const WORKER_COUNTS: [usize; 6] = [1, 2, 4, 8, 12, 16];

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == CHILD_FLAG) {
        return routine_processor_cache_ab::runner::run_child(&args);
    }
    if !args.is_empty() && args.as_slice() != ["--bench"] {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid arguments").into());
    }

    let executable = std::env::current_exe()?;
    println!(
        "routine_processor_cache_ab process_isolated=true instrumentation=false workers={WORKER_COUNTS:?} samples=10 tasks=256 workloads=noop,cpu64,cpu4096"
    );
    for (index, workers) in WORKER_COUNTS.into_iter().enumerate() {
        let modes = if index % 2 == 0 {
            [
                routine_processor_cache_ab::runner::AllocationMode::Global,
                routine_processor_cache_ab::runner::AllocationMode::ProcessorCache,
            ]
        } else {
            [
                routine_processor_cache_ab::runner::AllocationMode::ProcessorCache,
                routine_processor_cache_ab::runner::AllocationMode::Global,
            ]
        };
        for mode in modes {
            let output = Command::new(&executable)
                .arg(CHILD_FLAG)
                .arg(mode.name())
                .arg(workers.to_string())
                .output()?;
            forward_output(output, mode, workers)?;
        }
    }
    Ok(())
}

fn forward_output(
    output: Output,
    mode: routine_processor_cache_ab::runner::AllocationMode,
    workers: usize,
) -> Result<(), Box<dyn Error>> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!(
            "isolated {mode:?}/{workers} process exited with {}",
            output.status
        ))
        .into());
    }
    io::stdout().write_all(&output.stdout)?;
    Ok(())
}
