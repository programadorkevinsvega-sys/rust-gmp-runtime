use std::error::Error;
use std::io;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::stack::{GoInspiredOptions, StackConfig, StackEngine, StackManager};

use self::validation::validate_metrics;
use self::workload::Workload;
use super::super::WORKER_COUNTS;

#[path = "runner/validation.rs"]
mod validation;
#[path = "runner/worker.rs"]
mod worker;

const SAMPLE_COUNT: usize = 10;
const TASK_COUNT: usize = 256;
const WARMUP_TASKS_PER_WORKER: usize = 64;
const STACK_SIZE: usize = 64 * 1024;
const SLOTS_PER_SPAN: usize = 64;
const CACHE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AllocationMode {
    Global,
    ProcessorCache,
}

impl AllocationMode {
    pub(super) fn parse(value: &str) -> Result<Self, io::Error> {
        match value {
            "global" => Ok(Self::Global),
            "processor-cache" => Ok(Self::ProcessorCache),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown allocation mode",
            )),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::ProcessorCache => "processor-cache",
        }
    }
}

pub(crate) fn run_child(args: &[String]) -> Result<(), Box<dyn Error>> {
    let args = if args.last().is_some_and(|arg| arg == "--bench") {
        &args[..args.len() - 1]
    } else {
        args
    };
    let [_, mode, worker_count] = args else {
        return Err("isolated run requires allocation mode and worker count".into());
    };
    let mode = AllocationMode::parse(mode)?;
    let workers = worker_count.parse::<usize>()?;
    if !WORKER_COUNTS.contains(&workers) {
        return Err("unsupported worker count".into());
    }

    let config = StackConfig::new(STACK_SIZE)?.with_engine(StackEngine::GoInspired);
    let options = GoInspiredOptions::default()
        .with_slots_per_span(SLOTS_PER_SPAN)
        .with_cache_bytes(CACHE_BYTES);
    let stacks = Arc::new(StackManager::with_go_inspired_options(config, options)?);
    let processors = Arc::new(ProcessorPool::new(workers)?);
    let ready = Arc::new(Barrier::new(workers + 1));
    let start = Arc::new(Barrier::new(workers + 1));
    let done = Arc::new(Barrier::new(workers + 1));
    let mut worker_threads = Vec::with_capacity(workers);

    println!(
        "routine_processor_cache_sample_start mode={} workers={workers} stack_bytes={STACK_SIZE} slots_per_span={SLOTS_PER_SPAN} cache_bytes={CACHE_BYTES} samples={SAMPLE_COUNT} tasks={TASK_COUNT} instrumentation=false",
        mode.name()
    );

    for worker_index in 0..workers {
        let stacks = Arc::clone(&stacks);
        let processors = Arc::clone(&processors);
        let ready = Arc::clone(&ready);
        let start = Arc::clone(&start);
        let done = Arc::clone(&done);
        worker_threads.push(thread::spawn(move || {
            worker::run_worker(worker::WorkerConfig {
                worker_index,
                workers,
                mode,
                stacks,
                processors,
                ready,
                start,
                done,
            })
        }));
    }

    let mut samples = Vec::with_capacity(Workload::ALL.len() * SAMPLE_COUNT);
    for workload in Workload::ALL {
        ready.wait();
        for sample in 0..SAMPLE_COUNT {
            let started = Instant::now();
            start.wait();
            done.wait();
            let elapsed_ns = started.elapsed().as_nanos();
            let throughput = TASK_COUNT as f64 / (elapsed_ns as f64 / 1_000_000_000.0);
            println!(
                "routine_processor_sample mode={} workers={workers} workload={} sample={sample} operations={TASK_COUNT} completed_operations={TASK_COUNT} elapsed_ns={elapsed_ns} ops_s={throughput:.2}",
                mode.name(),
                workload.name()
            );
            samples.push((workload, elapsed_ns));
        }
    }

    let mut first_worker_error = None;
    for worker in worker_threads {
        match worker.join() {
            Ok(Ok(())) => {}
            Ok(Err(message)) => {
                if first_worker_error.is_none() {
                    first_worker_error = Some(message);
                }
            }
            Err(payload) => {
                if first_worker_error.is_none() {
                    first_worker_error = Some(format!("worker panicked: {payload:?}"));
                }
            }
        }
    }
    if let Some(message) = first_worker_error {
        return Err(io::Error::other(message).into());
    }

    let before_trim = stacks.metrics()?;
    let go_metrics = before_trim
        .go_inspired
        .as_ref()
        .ok_or("Go-inspired metrics missing")?;
    let expected_operations = u64::try_from(
        workers
            .checked_mul(WARMUP_TASKS_PER_WORKER)
            .ok_or("warmup operation count overflow")?
            .checked_mul(Workload::ALL.len())
            .ok_or("warmup operation count overflow")?
            .checked_add(TASK_COUNT * SAMPLE_COUNT * Workload::ALL.len())
            .ok_or("operation count overflow")?,
    )?;
    validate_metrics(mode, workers, expected_operations, go_metrics)?;
    println!(
        "routine_processor_validation mode={} workers={workers} allocations={} releases={} cache_hits={} cache_misses={} refills={} refill_slots={} active={} local_free={} failures={} exactly_once=true",
        mode.name(),
        go_metrics.allocations,
        go_metrics.releases,
        go_metrics.cache_hits,
        go_metrics.cache_misses,
        go_metrics.cache_refills,
        go_metrics.cache_refill_slots,
        go_metrics.active_stacks,
        go_metrics.local_free_slots,
        go_metrics
            .allocation_failures
            .checked_add(go_metrics.cleanup_failures)
            .ok_or("failure count overflow")?
    );

    stacks.trim_to_retained_spans(0)?;
    let final_metrics = stacks.metrics()?;
    if final_metrics.reserved_virtual_bytes != 0
        || final_metrics.accessible_virtual_bytes != 0
        || final_metrics.active_stacks != 0
    {
        return Err(format!("trim left stack resources: {final_metrics:?}").into());
    }
    report::summarize(mode, workers, &samples);
    Ok(())
}

#[path = "report.rs"]
mod report;
#[path = "workload.rs"]
mod workload;
