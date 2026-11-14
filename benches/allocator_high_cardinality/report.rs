use std::error::Error;
use std::panic::resume_unwind;
use std::thread;
use std::time::Duration;

use super::SAMPLES;
use rust_gmp_runtime::stack::{StackBenchmarkSnapshot, StackMetrics};

pub(super) struct SampleReport<'a> {
    pub workers: usize,
    pub strategy: &'static str,
    pub snapshots: bool,
    pub sample: usize,
    pub elapsed: Duration,
    pub latencies: &'a [u128],
    pub snapshotter_timing: StackBenchmarkSnapshot,
    pub worker_timings: &'a [(usize, StackBenchmarkSnapshot)],
    pub snapshot_count: usize,
    pub snapshot_ns: u128,
    pub accounting_before: &'a StackMetrics,
    pub accounting_after: &'a StackMetrics,
}

pub(super) fn join_snapshotter(
    worker: Option<thread::JoinHandle<Result<(usize, u128), String>>>,
) -> Result<(usize, u128), Box<dyn Error>> {
    let Some(worker) = worker else {
        return Ok((0, 0));
    };
    match worker.join() {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(error)) => Err(error.into()),
        Err(payload) => resume_unwind(payload),
    }
}

pub(super) fn print_sample(report: SampleReport<'_>) {
    let SampleReport {
        workers,
        strategy,
        snapshots,
        sample,
        elapsed,
        latencies,
        snapshotter_timing,
        worker_timings,
        snapshot_count,
        snapshot_ns,
        accounting_before,
        accounting_after,
    } = report;
    let failed_allocations = accounting_after
        .failed_allocations
        .saturating_sub(accounting_before.failed_allocations);
    let cleanup_failures = accounting_after
        .cleanup_failures
        .saturating_sub(accounting_before.cleanup_failures);
    let preflight_rejections = accounting_after
        .vma_preflight_rejections
        .saturating_sub(accounting_before.vma_preflight_rejections);
    println!(
        "high_cardinality strategy={strategy} snapshots={snapshots} workers={workers} sample={sample} operations={} throughput_ops_sec={} p50_ns={} p95_ns={} p99_ns={} max_ns={} active_mappings_before={} active_mappings_after={} reserved_bytes_before={} reserved_bytes_after={} failed_allocations={failed_allocations} cleanup_failures={cleanup_failures} preflight_rejections={preflight_rejections} vmas_before={} vmas_after={} snapshotter_lock_acquisitions={} snapshotter_lock_wait_total_ns={} snapshotter_lock_hold_segments={} snapshotter_lock_hold_total_ns={} snapshotter_mmap_calls={} snapshotter_mprotect_calls={} snapshotter_munmap_calls={} snapshot_count={snapshot_count} snapshot_total_ns={snapshot_ns}",
        latencies.len(),
        ops_per_second(latencies.len(), elapsed),
        percentile(latencies, 50),
        percentile(latencies, 95),
        percentile(latencies, 99),
        latencies.iter().copied().max().unwrap_or(0),
        accounting_before.active_mappings,
        accounting_after.active_mappings,
        accounting_before.reserved_virtual_bytes,
        accounting_after.reserved_virtual_bytes,
        accounting_before.vma_current,
        accounting_after.vma_current,
        snapshotter_timing.lock_acquisitions,
        snapshotter_timing.lock_wait_ns,
        snapshotter_timing.lock_hold_segments,
        snapshotter_timing.lock_hold_ns,
        snapshotter_timing.mmap_calls,
        snapshotter_timing.mprotect_calls,
        snapshotter_timing.munmap_calls
    );
    for (worker, timing) in worker_timings {
        println!(
            "high_cardinality_worker strategy={strategy} snapshots={snapshots} workers={workers} sample={sample} worker={worker} lock_acquisitions={} lock_wait_total_ns={} lock_wait_avg_ns={} lock_hold_segments={} lock_hold_total_ns={} lock_hold_avg_ns={} mmap_calls={} mmap_total_ns={} mmap_avg_ns={} mprotect_calls={} mprotect_total_ns={} mprotect_avg_ns={} munmap_calls={} munmap_total_ns={} munmap_avg_ns={}",
            timing.lock_acquisitions,
            timing.lock_wait_ns,
            average(timing.lock_wait_ns, timing.lock_acquisitions),
            timing.lock_hold_segments,
            timing.lock_hold_ns,
            average(timing.lock_hold_ns, timing.lock_hold_segments),
            timing.mmap_calls,
            timing.mmap_ns,
            average(timing.mmap_ns, timing.mmap_calls),
            timing.mprotect_calls,
            timing.mprotect_ns,
            average(timing.mprotect_ns, timing.mprotect_calls),
            timing.munmap_calls,
            timing.munmap_ns,
            average(timing.munmap_ns, timing.munmap_calls)
        );
    }
}

pub(super) fn report_summary(
    strategy: &'static str,
    workers: usize,
    snapshots: bool,
    wall: &mut [Duration],
    latencies: &mut [u128],
) {
    wall.sort_unstable();
    let middle = wall[wall.len() / 2].as_nanos();
    println!(
        "high_cardinality_summary strategy={strategy} snapshots={snapshots} workers={workers} operations={} median_throughput_ops_sec={} p50_ns={} p95_ns={} p99_ns={} max_ns={} wall_spread_ns={}",
        latencies.len(),
        u128::try_from(latencies.len() / SAMPLES)
            .unwrap_or(u128::MAX)
            .saturating_mul(1_000_000_000)
            / middle.max(1),
        percentile(latencies, 50),
        percentile(latencies, 95),
        percentile(latencies, 99),
        latencies.iter().copied().max().unwrap_or(0),
        wall[wall.len() - 1]
            .as_nanos()
            .saturating_sub(wall[0].as_nanos())
    );
}

fn average(total: u64, count: u64) -> u64 {
    total / count.max(1)
}

fn ops_per_second(operations: usize, elapsed: Duration) -> u128 {
    u128::try_from(operations)
        .unwrap_or(u128::MAX)
        .saturating_mul(1_000_000_000)
        / elapsed.as_nanos().max(1)
}

fn percentile(values: &[u128], percentile: usize) -> u128 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = sorted.len().saturating_mul(percentile).div_ceil(100);
    sorted[index.saturating_sub(1).min(sorted.len() - 1)]
}
