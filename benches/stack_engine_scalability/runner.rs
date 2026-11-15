use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::stack::{StackConfig, StackEngine, StackManager};

use super::batch::{self, Timing};
use super::report;
use super::workload::Workload;
use super::{SAMPLE_COUNT, STACK_SIZE, TASK_COUNTS, WORKER_COUNTS, engine_name, parse_engine};

pub(super) fn run_child(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 3 {
        return Err("invalid child arguments".into());
    }
    let engine = parse_engine(&args[1])?;
    let workers = args[2].parse::<usize>()?;
    if !WORKER_COUNTS.contains(&workers) {
        return Err("unsupported worker count".into());
    }

    let stack_start = Instant::now();
    let stacks = StackManager::new(StackConfig::new(STACK_SIZE)?.with_engine(engine))?;
    let stack_setup = stack_start.elapsed();
    let machine_start = Instant::now();
    let mut machines = MachineManager::start(Arc::new(ProcessorPool::new(workers)?))?;
    let machine_setup = machine_start.elapsed();
    batch::validate_execution(&machines, &stacks, workers)?;

    let cases = cases();
    let mut case_samples = (0..cases.len())
        .map(|_| Vec::with_capacity(SAMPLE_COUNT))
        .collect::<Vec<_>>();
    for (workload, task_count) in cases.iter().copied() {
        batch::run_batch(&machines, &stacks, task_count.min(64), workload, false)?;
    }
    for sample in 0..SAMPLE_COUNT {
        for offset in 0..cases.len() {
            let case_index = (sample + offset) % cases.len();
            let (workload, task_count) = cases[case_index];
            let timing = batch::run_batch(&machines, &stacks, task_count, workload, false)?;
            print_sample(engine, workers, workload, task_count, sample, &timing);
            case_samples[case_index].push(timing);
        }
    }
    for ((workload, task_count), samples) in cases.into_iter().zip(&case_samples) {
        report::report_case(engine, workers, workload, task_count, samples);
    }
    report_runtime(
        &stacks,
        &mut machines,
        engine,
        workers,
        stack_setup,
        machine_setup,
    )?;
    Ok(())
}

fn cases() -> Vec<(Workload, usize)> {
    Workload::ALL
        .into_iter()
        .flat_map(|workload| TASK_COUNTS.map(|tasks| (workload, tasks)))
        .collect()
}

fn print_sample(
    engine: StackEngine,
    workers: usize,
    workload: Workload,
    tasks: usize,
    sample: usize,
    timing: &Timing,
) {
    println!(
        "sample engine={} workers={workers} workload={} tasks={tasks} sample={sample} stack_alloc_ns={} dispatch_to_join_ns={} end_to_end_ns={} ns_per_task={} tasks_per_sec={:.2}",
        engine_name(engine),
        workload.name(),
        timing.stack_allocation.as_nanos(),
        timing.dispatch_to_join.as_nanos(),
        timing.end_to_end.as_nanos(),
        timing.end_to_end.as_nanos() / tasks as u128,
        tasks as f64 / timing.end_to_end.as_secs_f64(),
    );
}

fn report_runtime(
    stacks: &StackManager,
    machines: &mut MachineManager,
    engine: StackEngine,
    workers: usize,
    stack_setup: std::time::Duration,
    machine_setup: std::time::Duration,
) -> Result<(), Box<dyn Error>> {
    let metrics = stacks.metrics()?;
    let shutdown_start = Instant::now();
    machines.shutdown()?;
    println!(
        "process_summary engine={} workers={workers} stack_manager_setup_ns={} machine_startup_ns={} machine_shutdown_ns={} reserved_bytes={} accessible_bytes={} active_stacks={} free_stacks={} allocations={} releases={} process_vsize={:?} process_rss={:?} process_vmas={:?}",
        engine_name(engine),
        stack_setup.as_nanos(),
        machine_setup.as_nanos(),
        shutdown_start.elapsed().as_nanos(),
        metrics.reserved_virtual_bytes,
        metrics.accessible_virtual_bytes,
        metrics.active_stacks,
        metrics.free_stacks,
        metrics.successful_allocations,
        metrics.released_stacks,
        metrics.process_virtual_bytes,
        metrics.process_resident_bytes,
        metrics.process_vma_count,
    );
    Ok(())
}
