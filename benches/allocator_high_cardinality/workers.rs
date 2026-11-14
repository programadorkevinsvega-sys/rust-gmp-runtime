use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use rust_gmp_runtime::stack::{StackBenchmarkSnapshot, StackManager};

use super::OPERATIONS_PER_WORKER;

type WorkerResult = Result<(Vec<u128>, StackBenchmarkSnapshot), String>;

pub(super) fn spawn_workers(
    manager: &StackManager,
    count: usize,
    barrier: &Arc<Barrier>,
) -> Vec<thread::JoinHandle<WorkerResult>> {
    (0..count)
        .map(|_| {
            let manager = manager.clone();
            let barrier = Arc::clone(barrier);
            thread::spawn(move || {
                let (manager, timing) = manager.benchmark_worker();
                let mut times = Vec::with_capacity(OPERATIONS_PER_WORKER);
                barrier.wait();
                for _ in 0..OPERATIONS_PER_WORKER {
                    let start = Instant::now();
                    let stack = manager.allocate().map_err(|error| error.to_string())?;
                    drop(stack);
                    times.push(start.elapsed().as_nanos());
                }
                Ok((times, timing.snapshot_and_reset()))
            })
        })
        .collect()
}

pub(super) fn spawn_snapshotter(
    manager: &StackManager,
    barrier: &Arc<Barrier>,
    done: &Arc<AtomicBool>,
) -> thread::JoinHandle<Result<(usize, u128), String>> {
    let manager = manager.clone();
    let barrier = Arc::clone(barrier);
    let done = Arc::clone(done);
    thread::spawn(move || {
        let mut count = 0;
        let mut total_ns = 0_u128;
        barrier.wait();
        loop {
            let started = Instant::now();
            manager.vma_snapshot().map_err(|error| error.to_string())?;
            total_ns = total_ns.saturating_add(started.elapsed().as_nanos());
            count += 1;
            if done.load(Ordering::Acquire) {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok((count, total_ns))
    })
}
