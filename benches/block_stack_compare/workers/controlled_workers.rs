use super::*;

pub(crate) fn spawn_workers(
    allocator: Arc<Allocator>,
    barriers: WorkerBarriers,
    snapshots_finished: Arc<AtomicBool>,
    count: usize,
    operations_per_worker: usize,
    instrumented: bool,
    force_refill: bool,
) -> Vec<JoinHandle<Result<WorkerReport, String>>> {
    (0..count)
        .map(|_| {
            let allocator = Arc::clone(&allocator);
            let barriers = barriers.clone();
            let take_snapshots = barriers.snapshot_start.is_some();
            let snapshots_finished = Arc::clone(&snapshots_finished);
            thread::spawn(move || {
                let cache = (|| {
                    let mut cache = allocator
                        .worker_cache()
                        .map_err(|error| error.to_string())?;
                    let warm_stack = allocator
                        .allocate_for_worker(&mut cache)
                        .map_err(|error| error.to_string())?;
                    allocator
                        .release_for_worker(warm_stack, &mut cache)
                        .map_err(|error| error.to_string())?;
                    Ok(cache)
                })();
                barriers.ready.wait();
                barriers.start.wait();
                let mut cache = match cache {
                    Ok(cache) => cache,
                    Err(error) => {
                        if let Some(snapshot_start) = &barriers.snapshot_start {
                            snapshot_start.wait();
                        }
                        return Err(error);
                    }
                };
                let mut report = WorkerReport {
                    operations: 0,
                    latencies_ns: Vec::with_capacity(operations_per_worker),
                };
                let first_operation = run_operation(
                    &allocator,
                    &mut cache,
                    &mut report,
                    instrumented,
                    take_snapshots,
                    operations_per_worker,
                    force_refill,
                );
                if let Some(snapshot_start) = barriers.snapshot_start {
                    snapshot_start.wait();
                }
                first_operation?;
                while report.operations < operations_per_worker
                    || (take_snapshots && !snapshots_finished.load(Ordering::Acquire))
                {
                    run_operation(
                        &allocator,
                        &mut cache,
                        &mut report,
                        instrumented,
                        take_snapshots,
                        operations_per_worker,
                        force_refill,
                    )?;
                }
                Ok(report)
            })
        })
        .collect()
}
pub(crate) fn spawn_snapshotter(
    allocator: Arc<Allocator>,
    snapshot_start: Arc<Barrier>,
    snapshots_finished: Arc<AtomicBool>,
) -> JoinHandle<Result<SnapshotReport, String>> {
    thread::spawn(move || {
        let mut report = SnapshotReport {
            durations_ns: Vec::with_capacity(SNAPSHOTS_PER_SAMPLE),
            vma_counts: Vec::with_capacity(SNAPSHOTS_PER_SAMPLE),
        };
        snapshot_start.wait();
        let result = (|| {
            for _ in 0..SNAPSHOTS_PER_SAMPLE {
                let started = Instant::now();
                let snapshot = allocator
                    .vma_snapshot()
                    .map_err(|error| error.to_string())?;
                report
                    .durations_ns
                    .push(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX));
                report.vma_counts.push(snapshot.current);
            }
            Ok(report)
        })();
        snapshots_finished.store(true, Ordering::Release);
        result
    })
}
pub(super) fn run_operation(
    allocator: &Allocator,
    cache: &mut WorkerCache,
    report: &mut WorkerReport,
    instrumented: bool,
    take_snapshots: bool,
    minimum_operations: usize,
    force_refill: bool,
) -> Result<(), String> {
    let operation_started = instrumented.then(Instant::now);
    let stack = allocator
        .allocate_for_worker(cache)
        .map_err(|error| error.to_string())?;
    allocator
        .release_for_worker(stack, cache)
        .map_err(|error| error.to_string())?;
    if force_refill {
        allocator
            .flush_worker_cache(cache)
            .map_err(|error| error.to_string())?;
    }
    report.operations = report
        .operations
        .checked_add(1)
        .ok_or_else(|| "worker operation count overflow".to_owned())?;
    if !take_snapshots
        || report.operations <= minimum_operations
        || report.operations % SNAPSHOT_LATENCY_SAMPLE_INTERVAL == 0
    {
        if let Some(operation_started) = operation_started {
            report
                .latencies_ns
                .push(u64::try_from(operation_started.elapsed().as_nanos()).unwrap_or(u64::MAX));
        }
    }
    Ok(())
}
pub(crate) struct WorkerReport {
    pub(super) operations: usize,
    pub(super) latencies_ns: Vec<u64>,
}

#[derive(Clone)]
pub(crate) struct WorkerBarriers {
    pub(super) ready: Arc<Barrier>,
    pub(super) start: Arc<Barrier>,
    pub(super) snapshot_start: Option<Arc<Barrier>>,
}
