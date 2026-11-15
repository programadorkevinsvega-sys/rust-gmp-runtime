use std::thread::ThreadId;

use crate::routine::RoutineRunError;
use crate::scheduler::Scheduler;

use super::super::control::WorkerControl;
use super::super::dispatch::DispatchTask;
use super::WorkerStackState;
use crate::machine::{MachineError, MachineId, RoutineJoinError};

pub(super) fn execute_task(
    mut task: DispatchTask,
    machine_id: MachineId,
    scheduler: &Scheduler,
    control: &WorkerControl,
    thread_id: ThreadId,
    tls_machine_id: Option<MachineId>,
    mut stacks: WorkerStackState<'_>,
) -> Result<(), MachineError> {
    if task.needs_preparation() {
        control.mark_preparing(task.routine_id())?;
        scheduler.record_worker_state_transition()?;
    }
    let execution = execute_queued_task(&mut task, machine_id, scheduler, control, &mut stacks);
    control.mark_idle(thread_id, tls_machine_id)?;
    scheduler.record_worker_state_transition()?;
    match execution {
        Ok(Ok(())) => {
            scheduler.record_completed()?;
            task.complete(Ok(()));
        }
        Ok(Err(error)) => {
            scheduler.record_completed()?;
            task.complete(Err(RoutineJoinError::Execution(error)));
        }
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
    scheduler: &Scheduler,
    control: &WorkerControl,
    stacks: &mut WorkerStackState<'_>,
) -> Result<Result<(), RoutineRunError>, MachineError> {
    if task.needs_preparation() {
        let stack_manager = stacks
            .stack_manager
            .ok_or(MachineError::StackManagerNotConfigured)?;
        if let Err(error) = task.prepare_routine(stack_manager, stacks.cache.as_deref_mut()) {
            return Ok(Err(RoutineRunError::Runtime(error)));
        }
    }
    let routine = task
        .routine()
        .cloned()
        .ok_or_else(|| MachineError::WorkerFailed {
            machine_id,
            message: "dispatch task no longer owns its routine".into(),
        })?;
    let permit = routine
        .try_start_dispatched()
        .map_err(|error| MachineError::WorkerFailed {
            machine_id,
            message: format!(
                "could not claim routine {} before context transfer: {error}",
                routine.id().get()
            ),
        })?;
    control.mark_running(routine.id())?;
    scheduler.record_worker_state_transition()?;
    let entry = task
        .take_entry()
        .ok_or_else(|| MachineError::WorkerFailed {
            machine_id,
            message: "dispatch entry was already consumed".into(),
        })?;
    #[cfg(feature = "scheduler-diagnostics")]
    let active_started = std::time::Instant::now();
    let outcome = routine.run_once_with_permit(entry, permit);
    #[cfg(feature = "scheduler-diagnostics")]
    scheduler.record_active(machine_id.index(), active_started.elapsed().as_nanos())?;
    if task.returns_stack_to_cache() {
        let cache = stacks
            .cache
            .as_deref_mut()
            .ok_or_else(|| MachineError::WorkerFailed {
                machine_id,
                message: "routine requested Processor cache release without a worker cache".into(),
            })?;
        if let Err(error) = task.release_stack_to_cache(cache) {
            return Ok(Err(RoutineRunError::Runtime(error)));
        }
    }
    Ok(outcome)
}
