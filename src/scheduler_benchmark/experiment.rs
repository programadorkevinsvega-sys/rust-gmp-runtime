use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use crate::machine::MachineId;
use crate::machine::worker::dispatch::DispatchTask;
use crate::queue::LOCAL_QUEUE_CAPACITY;
use crate::routine::{Routine, RoutineMetadata};
use crate::scheduler::Scheduler;
use crate::stack::StackManager;

use super::trace;
use super::types::StealSample;

/// Runs the controlled steal experiment with a configurable steal batch and
/// optional per-task tracing.
///
/// Tracing adds recording work under the scheduler lock, so traced runs are
/// diagnostic only and their elapsed times must not be used as performance
/// measurements.
pub fn run_imbalanced_steal_configured(
    processor_count: usize,
    task_count: usize,
    steal_batch: usize,
    trace_tasks: bool,
    stack_manager: &StackManager,
) -> Result<StealSample, Box<dyn Error>> {
    if processor_count == 0 {
        return Err("processor_count must be positive".into());
    }
    if task_count == 0 || task_count % LOCAL_QUEUE_CAPACITY != 0 {
        return Err(
            format!("task_count must be a positive multiple of {LOCAL_QUEUE_CAPACITY}").into(),
        );
    }

    let scheduler = Arc::new(Scheduler::new(processor_count)?);
    let stealers = processor_count.saturating_sub(1);
    scheduler
        .benchmark_configure_stealing(
            steal_batch,
            if trace_tasks {
                task_count
                    .checked_mul(processor_count)
                    .ok_or("steal trace capacity overflow")?
            } else {
                0
            },
        )
        .map_err(|message| format!("cannot configure benchmark stealing: {message}"))?;
    if stealers == 0 {
        return Ok(StealSample {
            elapsed_ns: 0,
            stolen_tasks: 0,
            steal_attempts: 0,
            failed_steal_attempts: 0,
            tasks_by_processor: vec![0; processor_count],
            thief_drain_ns: Vec::new(),
            thief_barrier_wait_ns: Vec::new(),
            task_traces: Vec::new(),
        });
    }
    let barrier = Arc::new(Barrier::new(stealers + 1));
    let rounds = task_count / LOCAL_QUEUE_CAPACITY;
    let mut tasks = Vec::with_capacity(task_count);
    let mut handles = Vec::with_capacity(task_count);
    let mut routine_ids = Vec::with_capacity(task_count);
    for _ in 0..task_count {
        let routine = Routine::new(stack_manager, RoutineMetadata::new())?;
        routine_ids.push(routine.id().get());
        routine.reserve_for_dispatch()?;
        let (task, handle) = DispatchTask::new(MachineId::new(0), routine, || {});
        tasks.push(task);
        handles.push(handle);
    }
    let mut workers = Vec::with_capacity(stealers);
    let mut worker_drain_ns = vec![0_u128; stealers];
    let mut worker_barrier_wait_ns = vec![0_u128; stealers];
    for processor_index in 1..processor_count {
        let worker_scheduler = Arc::clone(&scheduler);
        let worker_barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(u128, u128), String> {
            let mut drain_ns = 0_u128;
            let mut barrier_wait_ns = 0_u128;
            for _ in 0..rounds {
                let barrier_started = trace_tasks.then(Instant::now);
                worker_barrier.wait();
                if let Some(started) = barrier_started {
                    barrier_wait_ns = barrier_wait_ns.saturating_add(started.elapsed().as_nanos());
                }
                let started = Instant::now();
                while let Some(task) = worker_scheduler
                    .take_next(processor_index)
                    .map_err(|error| error.to_string())?
                {
                    drop(task);
                }
                drain_ns = drain_ns.saturating_add(started.elapsed().as_nanos());
                let barrier_started = trace_tasks.then(Instant::now);
                worker_barrier.wait();
                if let Some(started) = barrier_started {
                    barrier_wait_ns = barrier_wait_ns.saturating_add(started.elapsed().as_nanos());
                }
            }
            Ok((drain_ns, barrier_wait_ns))
        }));
    }

    let mut elapsed_ns = 0_u128;
    let mut tasks = tasks.into_iter();
    for _ in 0..rounds {
        for task in tasks.by_ref().take(LOCAL_QUEUE_CAPACITY) {
            scheduler
                .benchmark_publish_local(0, task)
                .map_err(|(task, message)| {
                    drop(task);
                    message
                })?;
        }
        let started = Instant::now();
        barrier.wait();
        barrier.wait();
        elapsed_ns = elapsed_ns.saturating_add(started.elapsed().as_nanos());
    }

    for ((drain_total, barrier_total), worker) in worker_drain_ns
        .iter_mut()
        .zip(&mut worker_barrier_wait_ns)
        .zip(workers)
    {
        (*drain_total, *barrier_total) = worker
            .join()
            .map_err(|payload| format!("steal worker panicked: {}", trace::panic_message(payload)))?
            .map_err(|message| format!("steal worker failed: {message}"))?;
    }
    for handle in handles {
        if handle.join().is_ok() {
            return Err("unexecuted scheduler benchmark task reported success".into());
        }
    }
    let stats = scheduler.stats()?;
    let expected = u64::try_from(task_count)?;
    if stats.published_tasks != expected
        || stats.selected_tasks != expected
        || stats.stolen_tasks < expected
        || stats.local_queue_lengths.iter().any(|length| *length != 0)
    {
        return Err(format!(
            "imbalanced steal experiment accounting mismatch: published={} selected={} stolen={} queues={:?} expected={expected}",
            stats.published_tasks,
            stats.selected_tasks,
            stats.stolen_tasks,
            stats.local_queue_lengths
        )
        .into());
    }
    let task_traces = if trace_tasks {
        trace::build_task_traces(&routine_ids, scheduler.benchmark_steal_events()?)?
    } else {
        Vec::new()
    };
    Ok(StealSample {
        elapsed_ns,
        stolen_tasks: stats.stolen_tasks,
        steal_attempts: stats.steal_attempts,
        failed_steal_attempts: stats.failed_steal_attempts,
        tasks_by_processor: stats.tasks_by_processor,
        thief_drain_ns: worker_drain_ns,
        thief_barrier_wait_ns: worker_barrier_wait_ns,
        task_traces,
    })
}

pub(super) const STEAL_BATCH: usize = 16;
