use super::support::percentile_u64;
use super::types::*;
use super::*;

pub(super) fn run_batch(
    manager: &MachineManager,
    stack_manager: &StackManager,
    task_count: usize,
    workload: Workload,
    instrument_latency: bool,
) -> Result<Sample, Box<dyn Error>> {
    let routines = (0..task_count)
        .map(|_| Routine::new(stack_manager, RoutineMetadata::new()))
        .collect::<Result<Vec<_>, _>>()?;
    let latencies = instrument_latency.then(|| {
        Arc::new(
            (0..task_count)
                .map(|_| AtomicU64::new(0))
                .collect::<Vec<_>>(),
        )
    });
    let entries = instrument_latency.then(|| {
        Arc::new(
            (0..task_count)
                .map(|_| AtomicUsize::new(0))
                .collect::<Vec<_>>(),
        )
    });
    let before = manager.scheduler_stats()?;
    let started = Instant::now();
    let mut handles = Vec::with_capacity(task_count);
    for (index, routine) in routines.into_iter().enumerate() {
        let handle = if instrument_latency {
            let task_started = Instant::now();
            let task_latencies = Arc::clone(
                latencies
                    .as_ref()
                    .ok_or("latency instrumentation storage is missing")?,
            );
            let task_entries = Arc::clone(
                entries
                    .as_ref()
                    .ok_or("latency entry-count storage is missing")?,
            );
            manager.dispatch(routine, move || {
                let latency_ns =
                    u64::try_from(task_started.elapsed().as_nanos()).unwrap_or(u64::MAX);
                task_latencies[index].store(latency_ns, Ordering::Relaxed);
                task_entries[index].fetch_add(1, Ordering::Relaxed);
                workload.run(black_box(index as u64 + 1));
            })?
        } else {
            manager.dispatch(routine, move || {
                workload.run(black_box(index as u64 + 1));
            })?
        };
        handles.push(handle);
    }
    let submitted = Instant::now();
    let submit_ns = submitted.duration_since(started).as_nanos();
    for handle in handles {
        handle.join()?;
    }
    let end_to_end = started.elapsed();
    let end_to_end_ns = end_to_end.as_nanos();
    let drain_ns = end_to_end_ns.saturating_sub(submit_ns);

    if let Some(entries) = &entries {
        if entries
            .iter()
            .any(|entry| entry.load(Ordering::Relaxed) != 1)
        {
            return Err("benchmark detected missing or duplicate routine execution".into());
        }
    }
    let mut task_latencies = latencies.as_ref().map(|latencies| {
        latencies
            .iter()
            .map(|latency| latency.load(Ordering::Relaxed))
            .collect::<Vec<_>>()
    });
    let after = manager.scheduler_stats()?;
    verify_counters(&before, &after, task_count)?;
    if after.injection_queue_len != 0
        || after.global_queue_len != 0
        || after.local_queue_lengths.iter().any(|length| *length != 0)
    {
        return Err("benchmark ended with runnable work still queued".into());
    }
    let stolen_tasks = after.stolen_tasks.saturating_sub(before.stolen_tasks);
    let park_events = after.park_events.saturating_sub(before.park_events);
    let wake_notifications = after
        .wake_notifications
        .saturating_sub(before.wake_notifications);
    let tasks_by_processor = after
        .tasks_by_processor
        .iter()
        .zip(&before.tasks_by_processor)
        .map(|(after, before)| after.saturating_sub(*before))
        .collect();
    let worker_state_transitions = after
        .worker_state_transitions
        .saturating_sub(before.worker_state_transitions);
    Ok(Sample {
        submit_ns,
        drain_ns,
        end_to_end_ns,
        throughput_per_second: task_count as f64 / end_to_end.as_secs_f64(),
        schedule_p50_ns: task_latencies
            .as_mut()
            .map(|latencies| percentile_u64(latencies, 50)),
        schedule_p95_ns: task_latencies
            .as_mut()
            .map(|latencies| percentile_u64(latencies, 95)),
        stolen_tasks,
        park_events,
        wake_notifications,
        tasks_by_processor,
        peak_injection_queue_len: after.peak_injection_queue_len,
        peak_global_queue_len: after.peak_global_queue_len,
        peak_local_queue_lengths: after.peak_local_queue_lengths,
        worker_state_transitions,
    })
}

pub(super) fn run_parked_wake(
    manager: &MachineManager,
    stack_manager: &StackManager,
    worker_count: usize,
) -> Result<u64, Box<dyn Error>> {
    wait_until_parked(manager, worker_count)?;
    let routine = Routine::new(stack_manager, RoutineMetadata::new())?;
    let latency_ns = Arc::new(AtomicU64::new(0));
    let latency_in_entry = Arc::clone(&latency_ns);
    let started = Instant::now();
    let handle = manager.dispatch(routine, move || {
        latency_in_entry.store(
            u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    })?;
    handle.join()?;
    let measured = latency_ns.load(Ordering::Relaxed);
    if measured == 0 {
        return Err("parked-wake benchmark entry did not record its latency".into());
    }

    wait_until_parked(manager, worker_count)?;
    Ok(measured)
}

fn wait_until_parked(manager: &MachineManager, worker_count: usize) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if manager.scheduler_stats()?.parked_workers == worker_count {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("workers did not all park before the wake-latency sample".into());
        }
        std::thread::yield_now();
    }
}

fn verify_counters(
    before: &SchedulerStats,
    after: &SchedulerStats,
    expected: usize,
) -> Result<(), Box<dyn Error>> {
    let expected = u64::try_from(expected)?;
    if after.published_tasks.saturating_sub(before.published_tasks) != expected
        || after.selected_tasks.saturating_sub(before.selected_tasks) != expected
        || after.completed_tasks.saturating_sub(before.completed_tasks) != expected
        || after
            .tasks_by_processor
            .iter()
            .zip(&before.tasks_by_processor)
            .map(|(after, before)| after.saturating_sub(*before))
            .sum::<u64>()
            != expected
    {
        return Err("scheduler activity counters did not match submitted work".into());
    }
    Ok(())
}
