use std::cell::Cell;
use std::sync::Arc;
use std::thread;

use crate::processor::ProcessorPool;
use crate::scheduler::Scheduler;
use crate::stack::{ProcessorStackCache, StackManager};

use super::control::WorkerControl;
use crate::machine::{MachineError, MachineId};

#[path = "execution/task.rs"]
mod task;

thread_local! {
    static CURRENT_MACHINE_ID: Cell<Option<MachineId>> = const { Cell::new(None) };
}

pub(super) struct WorkerStackState<'worker> {
    stack_manager: Option<&'worker StackManager>,
    cache: Option<&'worker mut ProcessorStackCache>,
}

pub(super) fn run_worker(
    id: MachineId,
    processor_id: crate::processor::ProcessorId,
    processors: Arc<ProcessorPool>,
    scheduler: Arc<Scheduler>,
    stack_manager: Option<Arc<StackManager>>,
    processor_caches: bool,
    control: Arc<WorkerControl>,
) -> Result<(), MachineError> {
    let result = worker_loop(
        id,
        processor_id,
        &processors,
        &scheduler,
        stack_manager.as_deref(),
        processor_caches,
        &control,
    );
    if let Err(error) = &result {
        control.mark_failed(error.to_string())?;
        scheduler.record_worker_state_transition()?;
    }
    result
}

fn worker_loop(
    id: MachineId,
    processor_id: crate::processor::ProcessorId,
    processors: &ProcessorPool,
    scheduler: &Scheduler,
    stack_manager: Option<&StackManager>,
    processor_caches: bool,
    control: &WorkerControl,
) -> Result<(), MachineError> {
    register_current_machine(id)?;
    let processor = processors
        .get(processor_id)
        .ok_or(MachineError::EmptyProcessorPool)?;
    let binding = match processor.bind_current() {
        Ok(binding) => binding,
        Err(error) => {
            control.mark_failed(error.to_string())?;
            scheduler.record_worker_state_transition()?;
            return Ok(());
        }
    };
    if binding.processor().id() != processor_id {
        return Err(MachineError::WorkerFailed {
            machine_id: id,
            message: "worker bound to a different Processor".to_owned(),
        });
    }
    let mut cache = if processor_caches {
        let stack_manager = stack_manager.ok_or(MachineError::StackManagerNotConfigured)?;
        Some(
            stack_manager
                .processor_cache()
                .map_err(crate::routine::RoutineError::from)?,
        )
    } else {
        None
    };
    let thread_id = thread::current().id();
    let machine_tls_id = current_machine_id();
    control.mark_idle(thread_id, machine_tls_id)?;
    scheduler.record_worker_state_transition()?;
    while let Some(task) = scheduler.wait_next(processor_id.index())? {
        task::execute_task(
            task,
            id,
            scheduler,
            control,
            thread_id,
            machine_tls_id,
            WorkerStackState {
                stack_manager,
                cache: cache.as_mut(),
            },
        )?;
    }
    if let Some(cache) = &mut cache {
        cache.clear().map_err(crate::routine::RoutineError::from)?;
    }
    drop(binding);
    control.mark_terminated()?;
    scheduler.record_worker_state_transition()?;
    Ok(())
}

fn register_current_machine(id: MachineId) -> Result<(), MachineError> {
    CURRENT_MACHINE_ID.with(|current| match current.get() {
        Some(previous) => Err(MachineError::WorkerFailed {
            machine_id: id,
            message: format!(
                "worker thread already registered as Machine {}",
                previous.index()
            ),
        }),
        None => {
            current.set(Some(id));
            Ok(())
        }
    })
}

fn current_machine_id() -> Option<MachineId> {
    CURRENT_MACHINE_ID.with(Cell::get)
}
