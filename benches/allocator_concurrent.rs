//! Measures concurrent stack allocation, release, and VMA refresh activity.

use std::error::Error;
use std::panic::resume_unwind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use rust_gmp_runtime::stack::{StackConfig, StackManager};

const SAMPLE_COUNT: usize = 10;
const OPERATIONS_PER_WORKER: usize = 5_000;
const WORKER_COUNTS: [usize; 4] = [1, 2, 4, 8];
const REFRESH_INTERVAL: Duration = Duration::from_millis(1);

fn main() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    for workers in WORKER_COUNTS {
        measure_worker_count(&manager, workers)?;
    }
    Ok(())
}

fn measure_worker_count(manager: &StackManager, worker_count: usize) -> Result<(), Box<dyn Error>> {
    let mut operation_latencies =
        Vec::with_capacity(SAMPLE_COUNT * worker_count * OPERATIONS_PER_WORKER);
    let mut all_latencies = Vec::with_capacity(SAMPLE_COUNT * worker_count * OPERATIONS_PER_WORKER);
    let mut wall_times = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let before = manager.vma_snapshot()?;
        let start = Arc::new(Barrier::new(worker_count + 2));
        let done = Arc::new(AtomicBool::new(false));
        let snapshotter = spawn_snapshotter(manager, &start, &done);
        let workers = spawn_allocators(manager, worker_count, &start);

        let started = Instant::now();
        start.wait();
        let mut worker_error = None;
        let mut worker_panic = None;
        for worker in workers {
            let latencies = match worker.join() {
                Ok(Ok(latencies)) => latencies,
                Ok(Err(error)) => {
                    worker_error = Some(error);
                    continue;
                }
                Err(payload) => {
                    worker_panic = Some(payload);
                    continue;
                }
            };
            operation_latencies.extend(latencies);
        }
        let elapsed = started.elapsed();
        done.store(true, Ordering::Release);
        let (snapshot_count, snapshot_ns) = match snapshotter.join() {
            Ok(result) => result.map_err(std::io::Error::other)?,
            Err(payload) => resume_unwind(payload),
        };
        let after = manager.vma_snapshot()?;
        let metrics = manager.metrics()?;
        wall_times.push(elapsed);
        let sample_max = operation_latencies.iter().copied().max().unwrap_or(0);
        println!(
            "allocator_concurrent workers={worker_count} sample={sample} operations={} wall_ns={} throughput_ops_sec={} p50_ns={} p99_ns={} max_ns={sample_max} refresh_count={snapshot_count} refresh_total_ns={snapshot_ns} vmas_before={} vmas_after={} runtime_mapping_count={} runtime_vma_estimate={} reserved_virtual_bytes={} preflight_rejections={} allocation_failures={}",
            worker_count * OPERATIONS_PER_WORKER,
            elapsed.as_nanos(),
            operations_per_second(worker_count * OPERATIONS_PER_WORKER, elapsed),
            percentile(&operation_latencies, 50),
            percentile(&operation_latencies, 99),
            before.current,
            after.current,
            metrics.runtime_mapping_count,
            metrics.runtime_vma_estimate,
            metrics.reserved_virtual_bytes,
            metrics.vma_preflight_rejections,
            metrics.allocation_failures
        );
        all_latencies.append(&mut operation_latencies);
        if let Some(payload) = worker_panic {
            resume_unwind(payload);
        }
        if let Some(error) = worker_error {
            return Err(error.into());
        }
    }
    report_summary(worker_count, &mut wall_times, &mut all_latencies);
    Ok(())
}

fn spawn_allocators(
    manager: &StackManager,
    worker_count: usize,
    start: &Arc<Barrier>,
) -> Vec<thread::JoinHandle<Result<Vec<u128>, String>>> {
    (0..worker_count)
        .map(|_| {
            let manager = manager.clone();
            let start = Arc::clone(start);
            thread::spawn(move || {
                let mut latencies = Vec::with_capacity(OPERATIONS_PER_WORKER);
                start.wait();
                for _ in 0..OPERATIONS_PER_WORKER {
                    let operation_start = Instant::now();
                    let stack = manager.allocate().map_err(|error| error.to_string())?;
                    drop(stack);
                    latencies.push(operation_start.elapsed().as_nanos());
                }
                Ok(latencies)
            })
        })
        .collect()
}

fn spawn_snapshotter(
    manager: &StackManager,
    start: &Arc<Barrier>,
    done: &Arc<AtomicBool>,
) -> thread::JoinHandle<Result<(usize, u128), String>> {
    let manager = manager.clone();
    let start = Arc::clone(start);
    let done = Arc::clone(done);
    thread::spawn(move || {
        let mut count = 0;
        let mut elapsed_total = 0_u128;
        start.wait();
        loop {
            let refresh_start = Instant::now();
            manager.vma_snapshot().map_err(|error| error.to_string())?;
            elapsed_total = elapsed_total.saturating_add(refresh_start.elapsed().as_nanos());
            count += 1;
            if done.load(Ordering::Acquire) {
                break;
            }
            thread::sleep(REFRESH_INTERVAL);
        }
        Ok((count, elapsed_total))
    })
}

fn operations_per_second(operations: usize, elapsed: Duration) -> u128 {
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

fn report_summary(worker_count: usize, wall_times: &mut [Duration], latencies: &mut [u128]) {
    wall_times.sort_unstable();
    let median_wall = wall_times[wall_times.len() / 2].as_nanos();
    println!(
        "allocator_concurrent_summary workers={worker_count} samples={SAMPLE_COUNT} operations_per_sample={} median_wall_ns={median_wall} median_throughput_ops_sec={} p50_ns={} p99_ns={} max_ns={} wall_spread_ns={}",
        worker_count * OPERATIONS_PER_WORKER,
        u128::try_from(worker_count * OPERATIONS_PER_WORKER)
            .unwrap_or(u128::MAX)
            .saturating_mul(1_000_000_000)
            / median_wall.max(1),
        percentile(latencies, 50),
        percentile(latencies, 99),
        latencies.iter().copied().max().unwrap_or(0),
        wall_times[wall_times.len() - 1]
            .as_nanos()
            .saturating_sub(wall_times[0].as_nanos())
    );
}
