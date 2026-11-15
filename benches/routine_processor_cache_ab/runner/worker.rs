use std::sync::{Arc, Barrier};

use rust_gmp_runtime::processor::{ProcessorBinding, ProcessorPool};
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::StackManager;

use super::workload::Workload;
use super::{AllocationMode, SAMPLE_COUNT, TASK_COUNT, WARMUP_TASKS_PER_WORKER};

pub(super) struct WorkerConfig {
    pub(super) worker_index: usize,
    pub(super) workers: usize,
    pub(super) mode: AllocationMode,
    pub(super) stacks: Arc<StackManager>,
    pub(super) processors: Arc<ProcessorPool>,
    pub(super) ready: Arc<Barrier>,
    pub(super) start: Arc<Barrier>,
    pub(super) done: Arc<Barrier>,
}

pub(super) fn run_worker(config: WorkerConfig) -> Result<(), String> {
    let WorkerConfig {
        worker_index,
        workers,
        mode,
        stacks,
        processors,
        ready,
        start,
        done,
    } = config;
    let mut first_error = None;
    let binding: Option<ProcessorBinding<'_>> = match processors.iter().nth(worker_index) {
        Some(processor) => match processor.bind_current() {
            Ok(binding) => Some(binding),
            Err(error) => {
                first_error = Some(error.to_string());
                None
            }
        },
        None => {
            first_error = Some(format!("Processor {worker_index} is missing"));
            None
        }
    };
    let mut cache = None;
    if mode == AllocationMode::ProcessorCache && first_error.is_none() {
        match stacks.processor_cache() {
            Ok(processor_cache) => cache = Some(processor_cache),
            Err(error) => first_error = Some(error.to_string()),
        }
    }
    let (first_task, task_count) = task_range(worker_index, workers);

    for workload in Workload::ALL {
        if first_error.is_none() {
            if let Err(message) = run_tasks(
                &stacks,
                &mut cache,
                workload,
                first_task,
                WARMUP_TASKS_PER_WORKER,
            ) {
                first_error = Some(message);
            }
        }
        ready.wait();
        for _ in 0..SAMPLE_COUNT {
            start.wait();
            if first_error.is_none() {
                if let Err(message) =
                    run_tasks(&stacks, &mut cache, workload, first_task, task_count)
                {
                    first_error = Some(message);
                }
            }
            done.wait();
        }
    }

    drop(cache);
    drop(binding);
    match first_error {
        Some(message) => Err(message),
        None => Ok(()),
    }
}

fn run_tasks(
    stacks: &StackManager,
    cache: &mut Option<rust_gmp_runtime::stack::ProcessorStackCache>,
    workload: Workload,
    first_task: usize,
    task_count: usize,
) -> Result<(), String> {
    for offset in 0..task_count {
        let task_index = first_task
            .checked_add(offset)
            .ok_or_else(|| "task index overflow".to_owned())?;
        let metadata = RoutineMetadata::new();
        let routine = match cache {
            Some(cache) => Routine::new_with_processor_cache(stacks, cache, metadata),
            None => Routine::new(stacks, metadata),
        }
        .map_err(|error| error.to_string())?;
        routine.publish().map_err(|error| error.to_string())?;
        routine
            .run_once(|| workload.run(task_index as u64))
            .map_err(|error| error.to_string())?;
        if let Some(cache) = cache {
            routine
                .release_stack_to_processor_cache(cache)
                .map_err(|error| error.to_string())?;
        }
        drop(routine);
    }
    Ok(())
}

fn task_range(worker_index: usize, workers: usize) -> (usize, usize) {
    let base = TASK_COUNT / workers;
    let remainder = TASK_COUNT % workers;
    let preceding_extra = worker_index.min(remainder);
    let first = worker_index * base + preceding_extra;
    let count = base + usize::from(worker_index < remainder);
    (first, count)
}
