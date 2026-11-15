use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::stack::{GoInspiredOptions, StackConfig, StackEngine, StackManager};

use super::batch;
use super::parse_engine;
use super::report;
use super::workload::Workload;

const SAMPLE_COUNT: usize = 10;
const TASK_COUNT: usize = 256;
const STACK_SIZE: usize = 64 * 1024;
const WORKER_COUNTS: [usize; 4] = [1, 2, 4, 8];
const SLOTS_PER_SPAN: usize = 64;

pub(super) fn run_child(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 3 {
        return Err("invalid child arguments".into());
    }
    let engine = parse_engine(&args[1])?;
    let workers = args[2].parse::<usize>()?;
    if !WORKER_COUNTS.contains(&workers) {
        return Err("unsupported worker count".into());
    }

    let mut machines = MachineManager::start(Arc::new(ProcessorPool::new(workers)?))?;
    let validation_stacks = new_stack_manager(engine)?;
    batch::validate(&machines, &validation_stacks, workers.max(32))?;
    drop(validation_stacks);

    for workload in Workload::ALL {
        run_cold_samples(engine, workers, workload, &machines)?;
        run_hot_and_precreated(engine, workers, workload, &machines)?;
    }

    machines.shutdown()?;
    Ok(())
}

fn run_cold_samples(
    engine: StackEngine,
    workers: usize,
    workload: Workload,
    machines: &MachineManager,
) -> Result<(), Box<dyn Error>> {
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let manager_started = Instant::now();
        let stacks = new_stack_manager(engine)?;
        let manager_setup_ns = manager_started.elapsed().as_nanos();
        let created = batch::create_batch(&stacks, TASK_COUNT)?;
        let timing = batch::execute_batch(machines, created, workload, false, true)?;
        report::print_sample(
            engine,
            workers,
            "cold_full",
            workload,
            sample,
            manager_setup_ns,
            timing,
        );
        samples.push(timing);
        drop(stacks);
    }
    report::report_summary(engine, workers, "cold_full", workload, &samples);
    Ok(())
}

fn run_hot_and_precreated(
    engine: StackEngine,
    workers: usize,
    workload: Workload,
    machines: &MachineManager,
) -> Result<(), Box<dyn Error>> {
    let stacks = new_stack_manager(engine)?;
    warm_pool(machines, &stacks, workload)?;
    let mut hot_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut precreated_samples = Vec::with_capacity(SAMPLE_COUNT);

    for sample in 0..SAMPLE_COUNT {
        let created = batch::create_batch(&stacks, TASK_COUNT)?;
        let timing = batch::execute_batch(machines, created, workload, false, true)?;
        report::print_sample(engine, workers, "hot_full", workload, sample, 0, timing);
        hot_samples.push(timing);
    }

    warm_pool(machines, &stacks, workload)?;
    for sample in 0..SAMPLE_COUNT {
        let created = batch::create_batch(&stacks, TASK_COUNT)?;
        let timing = batch::execute_batch(machines, created, workload, false, false)?;
        report::print_sample(engine, workers, "precreated", workload, sample, 0, timing);
        precreated_samples.push(timing);
    }

    report::report_summary(engine, workers, "hot_full", workload, &hot_samples);
    report::report_summary(engine, workers, "precreated", workload, &precreated_samples);
    print_layout(engine, &stacks)?;
    Ok(())
}

fn warm_pool(
    machines: &MachineManager,
    stacks: &StackManager,
    workload: Workload,
) -> Result<(), Box<dyn Error>> {
    let created = batch::create_batch(stacks, TASK_COUNT)?;
    batch::execute_batch(machines, created, workload, false, true)?;
    Ok(())
}

fn new_stack_manager(engine: StackEngine) -> Result<StackManager, Box<dyn Error>> {
    let config = StackConfig::new(STACK_SIZE)?.with_engine(engine);
    match engine {
        StackEngine::Fixed => Ok(StackManager::new(config)?),
        StackEngine::GoInspired => Ok(StackManager::with_go_inspired_options(
            config,
            GoInspiredOptions::default().with_slots_per_span(SLOTS_PER_SPAN),
        )?),
    }
}

fn print_layout(engine: StackEngine, stacks: &StackManager) -> Result<(), Box<dyn Error>> {
    let page_size = stacks.page_size();
    let usable_size = stacks.usable_size();
    let stride = page_size
        .checked_add(usable_size)
        .ok_or("stack stride overflow")?;
    let span_size = stride
        .checked_mul(SLOTS_PER_SPAN)
        .ok_or("span size overflow")?;
    let pages_per_stack = usable_size.checked_div(page_size).ok_or("zero page size")?;
    let metrics = stacks.metrics()?;
    let engine_metrics = metrics.go_inspired;
    let (span_count, enabled_bytes, reserved_bytes, free_slots) = match engine_metrics {
        Some(metrics) => (
            metrics
                .spans_created
                .checked_sub(metrics.spans_released)
                .ok_or("released span count exceeds created span count")?,
            metrics.accessible_virtual_bytes,
            metrics.reserved_virtual_bytes,
            metrics
                .global_free_slots
                .checked_add(metrics.local_free_slots)
                .ok_or("free slot count overflow")?,
        ),
        None => (
            0,
            metrics.accessible_virtual_bytes,
            metrics.reserved_virtual_bytes,
            metrics.free_stacks,
        ),
    };
    if engine == StackEngine::GoInspired {
        let expected_reserved = span_size
            .checked_mul(usize::try_from(span_count)?)
            .ok_or("reserved span total overflow")?;
        if usize::try_from(reserved_bytes)? != expected_reserved {
            return Err(format!(
                "span accounting mismatch: expected {expected_reserved}, observed {reserved_bytes}"
            )
            .into());
        }
        if enabled_bytes % u64::try_from(usable_size)? != 0 {
            return Err("accessible bytes are not a whole number of usable stack pages".into());
        }
    }
    let enabled_slots = usize::try_from(enabled_bytes)?
        .checked_div(usable_size)
        .ok_or("zero usable stack size")?;
    let enabled_pages = enabled_bytes
        .checked_div(u64::try_from(page_size)?)
        .ok_or("zero page size")?;
    println!(
        "layout engine={engine:?} requested_usable_bytes={STACK_SIZE} page_size={page_size} usable_bytes={usable_size} guard_bytes={page_size} pages_per_stack={pages_per_stack} stride_bytes={stride} slots_per_span={SLOTS_PER_SPAN} span_bytes={span_size} span_count={span_count} free_slots={free_slots} enabled_slots={enabled_slots} enabled_pages={enabled_pages} enabled_usable_bytes={enabled_bytes} reserved_virtual_bytes={reserved_bytes}"
    );
    Ok(())
}
