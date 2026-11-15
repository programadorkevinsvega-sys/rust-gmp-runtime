//! Process-isolated, stackful throughput comparison for production stack engines.

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Output};

use rust_gmp_runtime::stack::StackEngine;

#[path = "stack_engine_scalability/batch.rs"]
mod batch;
#[path = "stack_engine_scalability/report.rs"]
mod report;
#[path = "stack_engine_scalability/runner.rs"]
mod runner;
#[path = "stack_engine_scalability/workload.rs"]
mod workload;

const SAMPLE_COUNT: usize = 10;
const TASK_COUNTS: [usize; 3] = [32, 256, 1_024];
const WORKER_COUNTS: [usize; 6] = [1, 2, 4, 8, 12, 16];
const STACK_SIZE: usize = 64 * 1024;
const CHILD_FLAG: &str = "--isolated";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == CHILD_FLAG) {
        return runner::run_child(&args);
    }
    if !args.is_empty() && args.as_slice() != ["--bench"] {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid arguments").into());
    }
    run_matrix()
}

fn run_matrix() -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    println!(
        "stack_engine_scalability process_isolated=true samples={SAMPLE_COUNT} tasks={TASK_COUNTS:?} workers={WORKER_COUNTS:?} workloads=noop,cpu64,cpu2048 stack_bytes={STACK_SIZE}"
    );
    for (worker_index, workers) in WORKER_COUNTS.into_iter().enumerate() {
        let engines = if worker_index % 2 == 0 {
            [StackEngine::Fixed, StackEngine::GoInspired]
        } else {
            [StackEngine::GoInspired, StackEngine::Fixed]
        };
        for engine in engines {
            let output = Command::new(&executable)
                .arg(CHILD_FLAG)
                .arg(engine_name(engine))
                .arg(workers.to_string())
                .output()?;
            forward_output(output, engine, workers)?;
        }
    }
    Ok(())
}

fn forward_output(
    output: Output,
    engine: StackEngine,
    workers: usize,
) -> Result<(), Box<dyn Error>> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!(
            "isolated {engine:?}/{workers} process exited with {}",
            output.status
        ))
        .into());
    }
    io::stdout().write_all(&output.stdout)?;
    Ok(())
}

fn parse_engine(name: &str) -> Result<StackEngine, io::Error> {
    match name {
        "Fixed" => Ok(StackEngine::Fixed),
        "GoInspired" => Ok(StackEngine::GoInspired),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown stack engine",
        )),
    }
}

fn engine_name(engine: StackEngine) -> &'static str {
    match engine {
        StackEngine::Fixed => "Fixed",
        StackEngine::GoInspired => "GoInspired",
    }
}
