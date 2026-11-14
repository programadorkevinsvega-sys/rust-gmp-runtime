use super::*;

pub fn run_controlled_sample(
    allocator: Arc<Allocator>,
    workers: usize,
    operations_per_worker: usize,
    instrumented: bool,
    take_snapshots: bool,
    force_refill: bool,
) -> Result<ControlledSample, Box<dyn Error>> {
    let participants = workers
        .checked_add(1)
        .ok_or("worker barrier participant count overflow")?;
    let barriers = WorkerBarriers {
        ready: Arc::new(Barrier::new(participants)),
        start: Arc::new(Barrier::new(participants)),
        snapshot_start: take_snapshots.then(|| Arc::new(Barrier::new(participants))),
    };
    let snapshots_finished = Arc::new(AtomicBool::new(false));
    let worker_threads = spawn_workers(
        Arc::clone(&allocator),
        barriers.clone(),
        Arc::clone(&snapshots_finished),
        workers,
        operations_per_worker,
        instrumented,
        force_refill,
    );
    let snapshot_thread = barriers
        .snapshot_start
        .clone()
        .map(|barrier| spawn_snapshotter(allocator, barrier, snapshots_finished));

    barriers.ready.wait();
    let started = Instant::now();
    barriers.start.wait();

    let snapshot = match snapshot_thread {
        Some(thread) => Some(match thread.join() {
            Ok(Ok(report)) => report,
            Ok(Err(error)) => return Err(error.into()),
            Err(payload) => std::panic::resume_unwind(payload),
        }),
        None => None,
    };
    let mut latencies_ns = Vec::new();
    let mut total_operations = 0usize;
    let mut worker_error = None;
    for worker in worker_threads {
        match worker.join() {
            Ok(Ok(mut report)) => {
                total_operations = total_operations
                    .checked_add(report.operations)
                    .ok_or("concurrent operation count overflow")?;
                latencies_ns.append(&mut report.latencies_ns);
            }
            Ok(Err(error)) => worker_error = Some(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    if let Some(error) = worker_error {
        return Err(error.into());
    }
    let elapsed_ns = started.elapsed().as_nanos();
    Ok(ControlledSample {
        elapsed_ns,
        operations: total_operations,
        latencies_ns,
        snapshot,
    })
}
