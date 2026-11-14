//! Measures allocator contention while a large set of stacks remains retained.

use std::error::Error;
use std::panic::resume_unwind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Instant;

use rust_gmp_runtime::stack::{Stack, StackBenchmarkMetrics, StackConfig, StackManager};

#[path = "allocator_high_cardinality/report.rs"]
mod report;
#[path = "allocator_high_cardinality/workers.rs"]
mod workers;

const BASE_STACKS: usize = 100_000;
const WORKER_COUNTS: [usize; 4] = [1, 2, 4, 8];
const OPERATIONS_PER_WORKER: usize = 5_000;
const SAMPLES: usize = 10;

fn main() -> Result<(), Box<dyn Error>> {
    let config = StackConfig::new(8 * 1024)?;
    for (strategy, two_phase) in [("serialized", false), ("two_phase", true)] {
        let (manager, timing) = if two_phase {
            StackManager::new_benchmarked_two_phase(config)?
        } else {
            StackManager::new_benchmarked(config)?
        };
        run_strategy(&manager, &timing, strategy)?;
    }
    Ok(())
}

fn run_strategy(
    manager: &StackManager,
    timing: &StackBenchmarkMetrics,
    strategy: &'static str,
) -> Result<(), Box<dyn Error>> {
    let initial = manager.vma_snapshot()?;
    let required = u64::try_from(BASE_STACKS)
        .unwrap_or(u64::MAX)
        .saturating_mul(2)
        .saturating_add(1_024);
    if initial.current.saturating_add(required) > initial.limit {
        println!(
            "high_cardinality skipped stacks={BASE_STACKS} current_vmas={} required_vmas={required} limit={}",
            initial.current, initial.limit
        );
        return Ok(());
    }
    let retained = allocate_batch(manager, BASE_STACKS)?;
    println!(
        "high_cardinality strategy={strategy} retained={} vmas={}",
        retained.len(),
        manager.vma_snapshot()?.current,
    );
    for snapshots in [false, true] {
        for workers in WORKER_COUNTS {
            run_scenario(manager, timing, strategy, workers, snapshots)?;
        }
    }
    drop(retained);
    let final_metrics = manager.metrics()?;
    if final_metrics.active_mappings != 0 {
        return Err(format!(
            "{strategy} left {} stack mappings after releasing the retained set",
            final_metrics.active_mappings
        )
        .into());
    }
    println!(
        "high_cardinality_final strategy={strategy} active_mappings={} failed_allocations={} vma_preflight_rejections={} vmas={}",
        final_metrics.active_mappings,
        final_metrics.failed_allocations,
        final_metrics.vma_preflight_rejections,
        final_metrics.vma_current
    );
    Ok(())
}

fn allocate_batch(manager: &StackManager, count: usize) -> Result<Vec<Stack>, Box<dyn Error>> {
    let mut stacks = Vec::with_capacity(count);
    for _ in 0..count {
        stacks.push(manager.allocate()?);
    }
    Ok(stacks)
}

fn run_scenario(
    manager: &StackManager,
    timing: &StackBenchmarkMetrics,
    strategy: &'static str,
    workers: usize,
    snapshots: bool,
) -> Result<(), Box<dyn Error>> {
    let mut wall_samples = Vec::with_capacity(SAMPLES);
    let mut latencies = Vec::with_capacity(SAMPLES * workers * OPERATIONS_PER_WORKER);
    for sample in 0..SAMPLES {
        let accounting_before = manager.metrics()?;
        timing.snapshot_and_reset();
        let barrier = Arc::new(Barrier::new(workers + 1 + usize::from(snapshots)));
        let done = Arc::new(AtomicBool::new(false));
        let snapshotter = if snapshots {
            Some(workers::spawn_snapshotter(manager, &barrier, &done))
        } else {
            None
        };
        let threads = workers::spawn_workers(manager, workers, &barrier);
        let started = Instant::now();
        barrier.wait();
        let mut sample_latencies = Vec::with_capacity(workers * OPERATIONS_PER_WORKER);
        let mut first_error = None;
        let mut first_panic = None;
        let mut worker_timings = Vec::with_capacity(workers);
        for (worker, thread) in threads.into_iter().enumerate() {
            match thread.join() {
                Ok(Ok((times, timing))) => {
                    sample_latencies.extend(times);
                    worker_timings.push((worker, timing));
                }
                Ok(Err(error)) => first_error = Some(error),
                Err(payload) => first_panic = Some(payload),
            }
        }
        let elapsed = started.elapsed();
        done.store(true, Ordering::Release);
        let (snapshot_count, snapshot_ns) = report::join_snapshotter(snapshotter)?;
        let measured = timing.snapshot_and_reset();
        let accounting_after = manager.metrics()?;
        wall_samples.push(elapsed);
        report::print_sample(report::SampleReport {
            strategy,
            workers,
            snapshots,
            sample,
            elapsed,
            latencies: &sample_latencies,
            snapshotter_timing: measured,
            worker_timings: &worker_timings,
            snapshot_count,
            snapshot_ns,
            accounting_before: &accounting_before,
            accounting_after: &accounting_after,
        });
        latencies.extend(sample_latencies);
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
        if let Some(error) = first_error {
            return Err(error.into());
        }
    }
    report::report_summary(
        strategy,
        workers,
        snapshots,
        &mut wall_samples,
        &mut latencies,
    );
    Ok(())
}
