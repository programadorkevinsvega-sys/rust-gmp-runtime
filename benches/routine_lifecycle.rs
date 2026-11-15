//! Compares production stack engines in sequential, process-isolated runs.

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Output};
use std::time::Instant;

use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackConfig, StackEngine, StackManager};

const SAMPLE_COUNT: usize = 10;
const TASK_COUNT: usize = 64;
const STACK_SIZE: usize = 64 * 1024;
const WORKER_FLAG: &str = "--isolated-sample";

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() == 3 && arguments[0] == WORKER_FLAG {
        return run_isolated_sample(parse_engine(&arguments[1])?, arguments[2].parse()?);
    }
    if !arguments.is_empty() && arguments.as_slice() != ["--bench"] {
        return Err(
            io::Error::new(io::ErrorKind::InvalidInput, "invalid benchmark arguments").into(),
        );
    }

    let executable = std::env::current_exe()?;
    let mut fixed_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut go_inspired_samples = Vec::with_capacity(SAMPLE_COUNT);

    for sample in 0..SAMPLE_COUNT {
        let engines = if sample % 2 == 0 {
            [StackEngine::Fixed, StackEngine::GoInspired]
        } else {
            [StackEngine::GoInspired, StackEngine::Fixed]
        };
        for engine in engines {
            let output = Command::new(&executable)
                .arg(WORKER_FLAG)
                .arg(engine_name(engine))
                .arg(sample.to_string())
                .output()?;
            let elapsed_ns = consume_sample_output(output, engine, sample)?;
            match engine {
                StackEngine::Fixed => fixed_samples.push(elapsed_ns),
                StackEngine::GoInspired => go_inspired_samples.push(elapsed_ns),
            }
        }
    }

    report_samples("Fixed", &mut fixed_samples);
    report_samples("GoInspired", &mut go_inspired_samples);
    Ok(())
}

fn run_isolated_sample(engine: StackEngine, sample: usize) -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(STACK_SIZE)?.with_engine(engine))?;
    run_routines(&manager)?;

    let started = Instant::now();
    run_routines(&manager)?;
    let elapsed_ns = started.elapsed().as_nanos();
    let metrics = manager.metrics()?;

    println!(
        "sample engine={} sample={sample} process_isolated=true tasks={TASK_COUNT} stack_size={STACK_SIZE} batch_ns={elapsed_ns} ns_per_routine={} process_vsize={:?} process_rss={:?} process_vmas={:?} reserved_bytes={} accessible_bytes={} active_stacks={} free_stacks={} allocations={} releases={}",
        engine_name(engine),
        elapsed_ns / TASK_COUNT as u128,
        metrics.process_virtual_bytes,
        metrics.process_resident_bytes,
        metrics.process_vma_count,
        metrics.reserved_virtual_bytes,
        metrics.accessible_virtual_bytes,
        metrics.active_stacks,
        metrics.free_stacks,
        metrics.successful_allocations,
        metrics.released_stacks,
    );
    Ok(())
}

fn run_routines(manager: &StackManager) -> Result<(), Box<dyn Error>> {
    for _ in 0..TASK_COUNT {
        let routine = Routine::new(manager, RoutineMetadata::new())?;
        routine.publish()?;
        routine.try_start()?.complete()?;
    }
    Ok(())
}

fn consume_sample_output(
    output: Output,
    engine: StackEngine,
    sample: usize,
) -> Result<u128, Box<dyn Error>> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!(
            "isolated {engine:?} sample {sample} exited with {}",
            output.status
        ))
        .into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    io::stdout().write_all(stdout.as_bytes())?;
    let elapsed = stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("batch_ns="))
        .ok_or_else(|| io::Error::other("isolated sample did not report batch_ns"))?
        .parse::<u128>()?;
    Ok(elapsed)
}

fn parse_engine(name: &str) -> Result<StackEngine, io::Error> {
    match name {
        "Fixed" => Ok(StackEngine::Fixed),
        "GoInspired" => Ok(StackEngine::GoInspired),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown stack engine: {name}"),
        )),
    }
}

fn engine_name(engine: StackEngine) -> &'static str {
    match engine {
        StackEngine::Fixed => "Fixed",
        StackEngine::GoInspired => "GoInspired",
    }
}

fn report_samples(label: &str, samples: &mut [u128]) {
    samples.sort_unstable();
    let upper_middle = samples[samples.len() / 2];
    let lower_middle = samples[samples.len() / 2 - 1];
    let median = lower_middle + (upper_middle - lower_middle) / 2;
    let maximum = samples[samples.len() - 1];
    let minimum = samples[0];
    println!(
        "summary engine={label} samples={} median_batch_ns={median} max_observed_batch_ns={maximum} min_batch_ns={minimum} spread_ns={}",
        samples.len(),
        maximum - minimum
    );
}
