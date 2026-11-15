//! Diagnostic-only instrumentation of real scheduler lock and Processor timings.

use std::env;
use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use rust_gmp_runtime::machine::{MachineManager, RoutineJoinHandle};
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

const TASK_COUNT: usize = 4_096;
const STACK_BYTES: usize = 32 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let workers = parse_env("RUST_GMP_DIAGNOSTIC_WORKERS", 4_usize)?;
    let publishers = parse_env("RUST_GMP_DIAGNOSTIC_PUBLISHERS", 4_usize)?;
    let task_count = parse_env("RUST_GMP_DIAGNOSTIC_TASKS", TASK_COUNT)?;
    if workers == 0 || workers > 64 {
        return Err("RUST_GMP_DIAGNOSTIC_WORKERS must be between 1 and 64".into());
    }
    if publishers == 0 || publishers > 64 {
        return Err("RUST_GMP_DIAGNOSTIC_PUBLISHERS must be between 1 and 64".into());
    }
    if task_count == 0 {
        return Err("RUST_GMP_DIAGNOSTIC_TASKS must be positive".into());
    }

    let stack_manager = StackManager::new(StackConfig::new(STACK_BYTES)?)?;
    let manager = Arc::new(MachineManager::start(Arc::new(ProcessorPool::new(
        workers,
    )?))?);
    let mut routines = Vec::with_capacity(task_count);
    for _ in 0..task_count {
        routines.push(Routine::new(&stack_manager, RoutineMetadata::new())?);
    }
    let mut producer_threads = Vec::with_capacity(publishers);
    let mut task_handles = Vec::with_capacity(task_count);
    let tasks_per_publisher = task_count / publishers;
    let remainder = task_count % publishers;
    let start = Arc::new(Barrier::new(publishers + 1));
    let mut routines = routines.into_iter();
    for publisher_index in 0..publishers {
        let manager = Arc::clone(&manager);
        let publisher_tasks = tasks_per_publisher + usize::from(publisher_index < remainder);
        let publisher_routines = routines.by_ref().take(publisher_tasks).collect::<Vec<_>>();
        let publisher_start = Arc::clone(&start);
        producer_threads.push(thread::spawn(move || {
            let mut handles = Vec::with_capacity(publisher_tasks);
            publisher_start.wait();
            for routine in publisher_routines {
                let handle = manager
                    .dispatch(routine, || {})
                    .map_err(|error| error.to_string())?;
                handles.push(handle);
            }
            Ok::<_, String>(handles)
        }));
    }
    if routines.next().is_some() {
        return Err("diagnostic benchmark did not distribute every routine".into());
    }
    start.wait();
    for producer in producer_threads {
        let handles = producer
            .join()
            .map_err(|payload| {
                format!(
                    "diagnostic publisher thread panicked: {}",
                    panic_message(payload)
                )
            })?
            .map_err(|message| format!("diagnostic publisher failed: {message}"))?;
        task_handles.extend(handles);
    }

    let drain_started = Instant::now();
    join_all(task_handles)?;
    let drain_ns = drain_started.elapsed().as_nanos();
    let stats = manager.scheduler_stats()?;
    let diagnostics = manager.scheduler_diagnostics();
    if stats.published_tasks != task_count as u64
        || stats.selected_tasks != task_count as u64
        || stats.completed_tasks != task_count as u64
        || stats.tasks_by_processor.iter().sum::<u64>() != task_count as u64
    {
        return Err(format!(
            "scheduler diagnostic run accounting mismatch: published={} selected={} completed={} by_processor={:?}",
            stats.published_tasks,
            stats.selected_tasks,
            stats.completed_tasks,
            stats.tasks_by_processor,
        )
        .into());
    }

    println!(
        "scheduler_diagnostics target={}-{} workers={workers} publishers={publishers} tasks={task_count} workload=noop instrumentation=enabled performance_timing=not_comparable mutex_wait_ns={} mutex_hold_ns={} park_events={} wake_notifications={} wake_returns={} drain_ns={drain_ns}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        diagnostics.mutex_wait_ns,
        diagnostics.mutex_hold_ns,
        stats.park_events,
        stats.wake_notifications,
        stats.wake_returns,
    );
    for processor in diagnostics.processors {
        println!(
            "processor_diagnostics p={} selected={} steal_attempts={} failed_steals={} stolen_in={} stolen_out={} mutex_wait_ns={} mutex_hold_ns={} publish_mutex_wait_ns={} publish_mutex_hold_ns={} worker_mutex_wait_ns={} worker_mutex_hold_ns={} selection_ns={} no_work_ns={} active_ns={} parked_ns={} wake_returns={}",
            processor.processor_index,
            processor.selected_tasks,
            processor.steal_attempts,
            processor.failed_steal_attempts,
            processor.stolen_in,
            processor.stolen_out,
            processor.mutex_wait_ns,
            processor.mutex_hold_ns,
            processor.publish_mutex_wait_ns,
            processor.publish_mutex_hold_ns,
            processor.worker_mutex_wait_ns,
            processor.worker_mutex_hold_ns,
            processor.selection_ns,
            processor.no_work_ns,
            processor.active_ns,
            processor.parked_ns,
            processor.wake_returns,
        );
    }
    let mut manager = Arc::try_unwrap(manager).map_err(|manager| {
        format!(
            "diagnostic manager still has {} outstanding references",
            Arc::strong_count(&manager)
        )
    })?;
    manager.shutdown()?;
    Ok(())
}

fn join_all(handles: Vec<RoutineJoinHandle>) -> Result<(), Box<dyn Error>> {
    for handle in handles {
        handle.join()?;
    }
    Ok(())
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload")
        .to_owned()
}

fn parse_env<T>(name: &str, default: T) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    match env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}
