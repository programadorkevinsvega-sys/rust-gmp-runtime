//! A/B of complete production Routine execution with isolated stack engines.

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Output};

use rust_gmp_runtime::stack::StackEngine;

#[path = "routine_execution_ab/batch.rs"]
mod batch;
#[path = "routine_execution_ab/report.rs"]
mod report;
#[path = "routine_execution_ab/runner.rs"]
mod runner;
#[path = "routine_execution_ab/workload.rs"]
mod workload;

const CHILD_FLAG: &str = "--isolated";
const WORKER_COUNTS: [usize; 4] = [1, 2, 4, 8];

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
        "routine_execution_ab process_isolated=true workers={WORKER_COUNTS:?} samples=10 tasks=256 workloads=noop,cpu64,cpu4096"
    );
    for (index, workers) in WORKER_COUNTS.into_iter().enumerate() {
        let engines = if index % 2 == 0 {
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

fn engine_name(engine: StackEngine) -> &'static str {
    match engine {
        StackEngine::Fixed => "Fixed",
        StackEngine::GoInspired => "GoInspired",
    }
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
