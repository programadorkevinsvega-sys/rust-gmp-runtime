use std::cell::Cell;
use std::sync::Arc;
use std::thread::{self, ThreadId};

use crate::processor::{ProcessorBinding, ProcessorPool};
use crate::routine::RoutineRunError;

use super::control::WorkerControl;
use super::dispatch::{DispatchTask, WorkerCommand};
use crate::machine::{MachineError, MachineId, RoutineJoinError};

thread_local! {
    static CURRENT_MACHINE_ID: Cell<Option<MachineId>> = const { Cell::new(None) };
}

pub(super) fn run_worker(
    id: MachineId,
    processor_id: crate::processor::ProcessorId,
    processors: Arc<ProcessorPool>,
    control: Arc<WorkerControl>,
    commands: std::sync::mpsc::Receiver<WorkerCommand>,
) -> Result<(), MachineError> {
    let result = worker_loop(id, processor_id, &processors, &control, commands);
    if let Err(error) = &result {
        control.mark_failed(error.to_string())?;
    }
    result
}

fn worker_loop(
    id: MachineId,
    processor_id: crate::processor::ProcessorId,
    processors: &ProcessorPool,
    control: &WorkerControl,
    commands: std::sync::mpsc::Receiver<WorkerCommand>,
) -> Result<(), MachineError> {
    register_current_machine(id)?;
    let processor = processors
        .get(processor_id)
        .ok_or(MachineError::EmptyProcessorPool)?;
    let binding = match processor.bind_current() {
        Ok(binding) => binding,
        Err(error) => {
            control.mark_failed(error.to_string())?;
            return Ok(());
        }
    };
    let mut binding = binding;
    let thread_id = thread::current().id();
    let machine_tls_id = current_machine_id();
    control.mark_idle(thread_id, machine_tls_id)?;
    for command in commands {
        match command {
            WorkerCommand::Run(task) => {
                execute_task(task, id, &mut binding, control, thread_id, machine_tls_id)?;
            }
            WorkerCommand::Stop => break,
        }
    }
    drop(binding);
    control.mark_terminated()?;
    Ok(())
}

fn execute_task(
    mut task: DispatchTask,
    machine_id: MachineId,
    binding: &mut ProcessorBinding<'_>,
    control: &WorkerControl,
    thread_id: ThreadId,
    tls_machine_id: Option<MachineId>,
) -> Result<(), MachineError> {
    let execution = execute_queued_task(&mut task, machine_id, binding, control);
    control.mark_idle(thread_id, tls_machine_id)?;
    match execution {
        Ok(Ok(())) => task.complete(Ok(())),
        Ok(Err(error)) => task.complete(Err(RoutineJoinError::Execution(error))),
        Err(error) => {
            task.fail(machine_id, error.to_string());
            return Err(error);
        }
    }
    Ok(())
}

fn execute_queued_task(
    task: &mut DispatchTask,
    machine_id: MachineId,
    binding: &mut ProcessorBinding<'_>,
    control: &WorkerControl,
) -> Result<Result<(), RoutineRunError>, MachineError> {
    binding
        .enqueue(&task.routine)
        .map_err(|error| MachineError::WorkerFailed {
            machine_id,
            message: error.to_string(),
        })?;
    let routine = binding
        .dequeue()
        .map_err(|error| MachineError::WorkerFailed {
            machine_id,
            message: error.to_string(),
        })?
        .ok_or_else(|| MachineError::WorkerFailed {
            machine_id,
            message: "queued routine disappeared before dispatch".into(),
        })?;
    if routine.id() != task.routine.id() {
        return Err(MachineError::WorkerFailed {
            machine_id,
            message: "local queue returned a different routine".into(),
        });
    }
    let permit = routine
        .try_start()
        .map_err(|error| MachineError::WorkerFailed {
            machine_id,
            message: format!(
                "could not claim routine {} before context transfer: {error}",
                routine.id().get()
            ),
        })?;
    control.mark_running(routine.id())?;
    let entry = task
        .take_entry()
        .ok_or_else(|| MachineError::WorkerFailed {
            machine_id,
            message: "dispatch entry was already consumed".into(),
        })?;
    Ok(routine.run_once_with_permit(entry, permit))
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
