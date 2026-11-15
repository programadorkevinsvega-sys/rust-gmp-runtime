//! Process-isolated comparison of global and Processor-local stack allocation.

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Output};

#[path = "machine_processor_cache_ab/runner.rs"]
mod runner;

const CHILD_FLAG: &str = "--isolated";
const WORKER_COUNTS: [usize; 6] = [1, 2, 4, 8, 12, 16];

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|argument| argument == CHILD_FLAG) {
        return runner::run_child(&args);
    }
    args.retain(|argument| argument != "--bench");
    let reverse_order = match args.as_slice() {
        [] => false,
        [argument] if argument == "--reverse-order" => true,
        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid arguments").into());
        }
    };
    let executable = std::env::current_exe()?;
    println!(
        "machine_processor_cache_ab process_isolated=true instrumentation=false workers={WORKER_COUNTS:?} samples={} tasks_per_sample={} workloads=noop,cpu64,cpu4096",
        runner::SAMPLE_COUNT,
        runner::TASK_COUNT
    );
    for (index, workers) in WORKER_COUNTS.into_iter().enumerate() {
        let cache_first = (index % 2 != 0) ^ reverse_order;
        let modes = if cache_first {
            ["processor-cache", "global"]
        } else {
            ["global", "processor-cache"]
        };
        for mode in modes {
            let output = Command::new(&executable)
                .arg(CHILD_FLAG)
                .arg(mode)
                .arg(workers.to_string())
                .output()?;
            forward_output(output, mode, workers)?;
        }
    }
    Ok(())
}

fn forward_output(output: Output, mode: &str, workers: usize) -> Result<(), Box<dyn Error>> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!(
            "isolated {mode}/{workers} process exited with {}",
            output.status
        ))
        .into());
    }
    io::stdout().write_all(&output.stdout)?;
    Ok(())
}
