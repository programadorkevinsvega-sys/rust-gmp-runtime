//! Measures one-shot routine dispatch through a bounded Machine worker pool.

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

const SAMPLE_COUNT: usize = 10;
const TASK_COUNT: usize = 128;
const WARMUP_COUNT: usize = 16;
const STACK_SIZE: usize = 32 * 1024;
const WORKER_COUNTS: [usize; 3] = [1, 2, 4];

static ENTRY_COUNT: AtomicUsize = AtomicUsize::new(0);

fn main() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    println!(
        "machine_dispatch target={}-{} samples={SAMPLE_COUNT} tasks_per_sample={TASK_COUNT} stack_bytes={STACK_SIZE} timed_work=enqueue_context_switch_and_join",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for worker_count in WORKER_COUNTS {
        measure_worker_count(&stack_manager, worker_count)?;
    }
    let expected_entries = WORKER_COUNTS.len() * (SAMPLE_COUNT * TASK_COUNT + WARMUP_COUNT);
    let observed_entries = ENTRY_COUNT.load(Ordering::Relaxed);
    if observed_entries != expected_entries {
        return Err(format!(
            "expected {expected_entries} benchmark entries, observed {observed_entries}"
        )
        .into());
    }
    Ok(())
}

fn measure_worker_count(
    stack_manager: &StackManager,
    worker_count: usize,
) -> Result<(), Box<dyn Error>> {
    let processors = Arc::new(ProcessorPool::new(worker_count)?);
    let mut machines = MachineManager::start(processors)?;
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);

    let warmup = prepare_routines(stack_manager, WARMUP_COUNT)?;
    let mut warmup_handles = Vec::with_capacity(WARMUP_COUNT);
    for routine in warmup {
        warmup_handles.push(machines.dispatch(routine, benchmark_entry)?);
    }
    for handle in warmup_handles {
        handle.join()?;
    }

    for sample in 0..SAMPLE_COUNT {
        let routines = prepare_routines(stack_manager, TASK_COUNT)?;
        let started = Instant::now();
        let mut handles = Vec::with_capacity(TASK_COUNT);
        for routine in routines {
            handles.push(machines.dispatch(routine, benchmark_entry)?);
        }
        for handle in handles {
            handle.join()?;
        }
        let elapsed = started.elapsed();
        samples.push(elapsed);
        println!(
            "machine_dispatch workers={worker_count} sample={sample} batch_ns={} ns_per_task={} tasks_per_second={}",
            elapsed.as_nanos(),
            elapsed.as_nanos() / TASK_COUNT as u128,
            TASK_COUNT as f64 / elapsed.as_secs_f64()
        );
    }

    machines.shutdown()?;
    report_summary(worker_count, &mut samples);
    Ok(())
}

fn prepare_routines(
    stack_manager: &StackManager,
    count: usize,
) -> Result<Vec<Routine>, Box<dyn Error>> {
    let mut routines = Vec::with_capacity(count);
    for _ in 0..count {
        let routine = Routine::new(stack_manager, RoutineMetadata::new())?;
        routines.push(routine);
    }
    Ok(routines)
}

#[inline(never)]
fn benchmark_entry() {
    ENTRY_COUNT.fetch_add(1, Ordering::Relaxed);
}

fn report_summary(worker_count: usize, samples: &mut [Duration]) {
    samples.sort_unstable();
    let median_ns = samples[samples.len() / 2].as_nanos();
    let maximum_ns = samples[samples.len() - 1].as_nanos();
    let minimum_ns = samples[0].as_nanos();
    let median_ns_per_task = median_ns / TASK_COUNT as u128;
    let median_tasks_per_second = TASK_COUNT as f64 / samples[samples.len() / 2].as_secs_f64();
    println!(
        "machine_dispatch summary workers={worker_count} samples={} median_batch_ns={median_ns} median_ns_per_task={median_ns_per_task} median_tasks_per_second={median_tasks_per_second} p99_max_observed_ns={maximum_ns} spread_ns={}",
        samples.len(),
        maximum_ns - minimum_ns
    );
}
