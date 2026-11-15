use super::types::*;
use super::*;

pub(super) fn run_batch(
    manager: &MachineManager,
    stack_manager: &StackManager,
    task_count: usize,
    workload: Workload,
    scenario: Scenario,
) -> Result<Sample, Box<dyn Error>> {
    let routines = (0..task_count)
        .map(|_| Routine::new(stack_manager, RoutineMetadata::new()))
        .collect::<Result<Vec<_>, _>>()?;
    let gate = if scenario == Scenario::Burst {
        Some(Arc::new((Mutex::new(false), Condvar::new())))
    } else {
        None
    };
    let gate_errors = Arc::new(AtomicUsize::new(0));
    let before = manager.scheduler_stats()?;
    let started = Instant::now();
    let mut handles = Vec::with_capacity(task_count);
    for (index, routine) in routines.into_iter().enumerate() {
        let entry_gate = gate.as_ref().map(Arc::clone);
        let entry_errors = Arc::clone(&gate_errors);
        let handle = manager.dispatch(routine, move || {
            if let Some(gate) = entry_gate.as_ref() {
                let (lock, available) = gate.as_ref();
                let mut released = match lock.lock() {
                    Ok(released) => released,
                    Err(error) => {
                        eprintln!("burst entry failed to acquire gate: {error}");
                        entry_errors.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                };
                while !*released {
                    released = match available.wait(released) {
                        Ok(released) => released,
                        Err(error) => {
                            eprintln!("burst entry failed waiting on gate: {error}");
                            entry_errors.fetch_add(1, Ordering::Relaxed);
                            return;
                        }
                    };
                }
            }
            workload.run(black_box(index as u64 + 1));
        });
        match handle {
            Ok(handle) => handles.push(handle),
            Err(error) => {
                release_burst_gate(&gate)?;
                return Err(Box::new(error));
            }
        }
    }
    let submitted = Instant::now();
    let submit_ns = submitted.duration_since(started).as_nanos();
    release_burst_gate(&gate)?;
    for handle in handles {
        handle.join()?;
    }
    if gate_errors.load(Ordering::Relaxed) != 0 {
        return Err("burst gate was poisoned before all task entries were released".into());
    }
    let end_to_end = started.elapsed();
    let end_to_end_ns = end_to_end.as_nanos();
    let drain_ns = end_to_end_ns.saturating_sub(submit_ns);
    let after = manager.scheduler_stats()?;
    verify_counters(&before, &after, task_count)?;
    if after.injection_queue_len != 0
        || after.global_queue_len != 0
        || after.local_queue_lengths.iter().any(|length| *length != 0)
    {
        return Err("benchmark ended with runnable work still queued".into());
    }
    Ok(Sample {
        submit_ns,
        drain_ns,
        end_to_end_ns,
        throughput_per_second: task_count as f64 / end_to_end.as_secs_f64(),
        stolen_tasks: after.stolen_tasks.saturating_sub(before.stolen_tasks),
        tasks_by_processor: after
            .tasks_by_processor
            .iter()
            .zip(&before.tasks_by_processor)
            .map(|(after, before)| after.saturating_sub(*before))
            .collect(),
        peak_injection_queue_len: after.peak_injection_queue_len,
        peak_global_queue_len: after.peak_global_queue_len,
        peak_local_queue_lengths: after.peak_local_queue_lengths,
        steal_attempts: after.steal_attempts.saturating_sub(before.steal_attempts),
        failed_steal_attempts: after
            .failed_steal_attempts
            .saturating_sub(before.failed_steal_attempts),
    })
}

fn release_burst_gate(gate: &Option<Arc<(Mutex<bool>, Condvar)>>) -> Result<(), Box<dyn Error>> {
    if let Some(gate) = gate {
        let (lock, available) = gate.as_ref();
        let mut released = lock
            .lock()
            .map_err(|error| format!("burst gate poisoned: {error}"))?;
        *released = true;
        available.notify_all();
    }
    Ok(())
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
        return Err("parked-wake entry did not record its latency".into());
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
            return Err("workers did not all park before the wake sample".into());
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
