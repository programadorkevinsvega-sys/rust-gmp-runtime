use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::StackManager;

use super::support::{panic_message, run_work, verify_stats};

#[derive(Clone, Debug)]
pub(super) struct Sample {
    pub(super) submit_ns: u128,
    pub(super) post_submit_drain_ns: u128,
    pub(super) end_to_end_ns: u128,
    pub(super) publisher_dispatch_ns: Vec<u128>,
    pub(super) selected_by_processor: Vec<u64>,
    pub(super) steal_attempts: u64,
    pub(super) failed_steal_attempts: u64,
    pub(super) stolen_tasks: u64,
    pub(super) peak_local_queues: Vec<usize>,
}

pub(super) fn run_sample(
    manager: &Arc<MachineManager>,
    stack_manager: &StackManager,
    task_count: usize,
    publisher_count: usize,
    work_units: usize,
) -> Result<Sample, Box<dyn Error>> {
    let routines = (0..task_count)
        .map(|_| Routine::new(stack_manager, RoutineMetadata::new()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut assignments = Vec::with_capacity(publisher_count);
    let mut routines = routines.into_iter();
    let base = task_count / publisher_count;
    let remainder = task_count % publisher_count;
    for index in 0..publisher_count {
        let count = base + usize::from(index < remainder);
        assignments.push(routines.by_ref().take(count).collect::<Vec<_>>());
    }
    if routines.next().is_some() {
        return Err("attribution harness did not assign all routines".into());
    }

    let before = manager.scheduler_stats()?;
    let ready = Arc::new(Barrier::new(publisher_count + 1));
    let start = Arc::new(Barrier::new(publisher_count + 1));
    let mut producer_threads = Vec::with_capacity(publisher_count);
    for assigned in assignments {
        let manager = manager.clone();
        let publisher_ready = Arc::clone(&ready);
        let publisher_start = Arc::clone(&start);
        producer_threads.push(thread::spawn(move || {
            publisher_ready.wait();
            publisher_start.wait();
            let dispatch_started = Instant::now();
            let mut handles = Vec::with_capacity(assigned.len());
            for (index, routine) in assigned.into_iter().enumerate() {
                let handle =
                    manager.dispatch(routine, move || run_work(work_units, index as u64))?;
                handles.push(handle);
            }
            Ok::<_, Box<dyn Error + Send + Sync>>((handles, dispatch_started.elapsed().as_nanos()))
        }));
    }

    ready.wait();
    let started = Instant::now();
    start.wait();
    let mut handles = Vec::with_capacity(task_count);
    let mut publisher_dispatch_ns = Vec::with_capacity(publisher_count);
    for producer in producer_threads {
        let (mut produced, elapsed_ns) = producer
            .join()
            .map_err(|payload| format!("publisher thread panicked: {}", panic_message(payload)))?
            .map_err(|error| format!("publisher failed: {error}"))?;
        handles.append(&mut produced);
        publisher_dispatch_ns.push(elapsed_ns);
    }
    let submitted = Instant::now();
    let submit_ns = submitted.duration_since(started).as_nanos();
    for handle in handles {
        handle.join()?;
    }
    let end_to_end_ns = started.elapsed().as_nanos();
    let post_submit_drain_ns = end_to_end_ns.saturating_sub(submit_ns);
    let after = manager.scheduler_stats()?;
    verify_stats(&before, &after, task_count)?;
    if after.injection_queue_len != 0
        || after.global_queue_len != 0
        || after.local_queue_lengths.iter().any(|len| *len != 0)
    {
        return Err("attribution sample completed with runnable tasks still queued".into());
    }
    Ok(Sample {
        submit_ns,
        post_submit_drain_ns,
        end_to_end_ns,
        publisher_dispatch_ns,
        selected_by_processor: after
            .tasks_by_processor
            .iter()
            .zip(&before.tasks_by_processor)
            .map(|(after, before)| after.saturating_sub(*before))
            .collect(),
        steal_attempts: after.steal_attempts.saturating_sub(before.steal_attempts),
        failed_steal_attempts: after
            .failed_steal_attempts
            .saturating_sub(before.failed_steal_attempts),
        stolen_tasks: after.stolen_tasks.saturating_sub(before.stolen_tasks),
        peak_local_queues: after.peak_local_queue_lengths,
    })
}
