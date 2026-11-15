use std::error::Error;
use std::io;
use std::sync::Arc;
use std::time::Instant;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::routine::RoutineMetadata;
use rust_gmp_runtime::stack::{GoInspiredOptions, StackConfig, StackEngine, StackManager};

#[path = "validation.rs"]
mod validation;
#[path = "workload.rs"]
mod workload;

pub(super) const SAMPLE_COUNT: usize = 30;
pub(super) const TASK_COUNT: usize = 256;
const WARMUP_COUNT: usize = 64;
pub(super) const VALIDATION_COUNT: usize = 128;
const STACK_SIZE: usize = 64 * 1024;
const SLOTS_PER_SPAN: usize = 64;
const CACHE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
enum AllocationMode {
    Global,
    ProcessorCache,
}

impl AllocationMode {
    fn parse(value: &str) -> Result<Self, io::Error> {
        match value {
            "global" => Ok(Self::Global),
            "processor-cache" => Ok(Self::ProcessorCache),
            _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "unknown mode")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::ProcessorCache => "processor-cache",
        }
    }
}

pub(super) fn run_child(args: &[String]) -> Result<(), Box<dyn Error>> {
    let args = if args.last().is_some_and(|argument| argument == "--bench") {
        &args[..args.len() - 1]
    } else {
        args
    };
    let [_, mode, worker_count] = args else {
        return Err("isolated run requires mode and worker count".into());
    };
    let mode = AllocationMode::parse(mode)?;
    let workers = worker_count.parse::<usize>()?;
    if ![1, 2, 4, 8, 12, 16].contains(&workers) {
        return Err("unsupported worker count".into());
    }
    let config = StackConfig::new(STACK_SIZE)?.with_engine(StackEngine::GoInspired);
    let options = GoInspiredOptions::default()
        .with_slots_per_span(SLOTS_PER_SPAN)
        .with_cache_bytes(CACHE_BYTES);
    let stacks = Arc::new(StackManager::with_go_inspired_options(config, options)?);
    let processors = Arc::new(ProcessorPool::new(workers)?);
    let mut manager = match mode {
        AllocationMode::Global => {
            MachineManager::start_with_stack_manager(Arc::clone(&processors), Arc::clone(&stacks))?
        }
        AllocationMode::ProcessorCache => MachineManager::start_with_processor_caches(
            Arc::clone(&processors),
            Arc::clone(&stacks),
        )?,
    };
    println!(
        "machine_processor_cache_start mode={} workers={workers} stack_bytes={STACK_SIZE} slots_per_span={SLOTS_PER_SPAN} cache_bytes={CACHE_BYTES} samples={SAMPLE_COUNT} tasks={TASK_COUNT} instrumentation=false",
        mode.name()
    );
    warm_up(&manager)?;
    let mut samples = Vec::with_capacity(workload::Workload::ALL.len() * SAMPLE_COUNT);
    for workload in workload::Workload::ALL {
        for sample in 0..SAMPLE_COUNT {
            let started = Instant::now();
            run_batch(&manager, workload, TASK_COUNT)?;
            let elapsed_ns = started.elapsed().as_nanos();
            println!(
                "machine_processor_cache_sample mode={} workers={workers} workload={} sample={sample} operations={TASK_COUNT} elapsed_ns={elapsed_ns}",
                mode.name(),
                workload.name()
            );
            samples.push((workload.name(), elapsed_ns));
        }
    }
    let observed = validation::verify(&manager, VALIDATION_COUNT)?;
    manager.shutdown()?;
    validation::check(&stacks, mode, workers, observed)?;
    summarize(mode, workers, &samples);
    Ok(())
}

fn warm_up(manager: &MachineManager) -> Result<(), Box<dyn Error>> {
    for workload in workload::Workload::ALL {
        run_batch(manager, workload, WARMUP_COUNT)?;
    }
    Ok(())
}

fn run_batch(
    manager: &MachineManager,
    workload: workload::Workload,
    count: usize,
) -> Result<(), Box<dyn Error>> {
    let mut handles = Vec::with_capacity(count);
    for seed in 0..count {
        handles.push(manager.dispatch_new(RoutineMetadata::new(), move || {
            workload.run(seed as u64);
        })?);
    }
    for handle in handles {
        handle.join()?;
    }
    Ok(())
}

fn summarize(mode: AllocationMode, workers: usize, samples: &[(&str, u128)]) {
    for workload in workload::Workload::ALL {
        let mut values = samples
            .iter()
            .filter(|(name, _)| *name == workload.name())
            .map(|(_, elapsed)| *elapsed)
            .collect::<Vec<_>>();
        values.sort_unstable();
        let median = values[values.len() / 2];
        println!(
            "machine_processor_cache_summary mode={} workers={workers} workload={} samples={} median_ns={median} min_ns={} max_ns={}",
            mode.name(),
            workload.name(),
            values.len(),
            values[0],
            values[values.len() - 1]
        );
    }
}
