//! Checked routine entry and return around the architecture switch.

use super::machine::MachineContext;
use crate::routine::{Routine, RoutineError, RoutinePanicPayload, RoutineRunError};
use std::num::NonZeroUsize;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) struct SwitchFrame {
    pub(crate) scheduler: *mut MachineContext,
    pub(crate) routine: *mut MachineContext,
    pub(crate) invoke_entry: unsafe fn(*mut ()) -> Result<(), RoutinePanicPayload>,
    pub(crate) entry_data: *mut (),
    pub(crate) outcome: Option<Result<(), RoutinePanicPayload>>,
    pub(crate) permit: Option<crate::routine::ExecutionPermit>,
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(crate) fn run_once<F>(_routine: &Routine, _entry: F) -> Result<(), RoutineRunError>
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    Err(RoutineError::UnsupportedContextArchitecture.into())
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(crate) fn run_once_with_permit<F>(
    _routine: &Routine,
    _entry: F,
    _permit: crate::routine::ExecutionPermit,
) -> Result<(), RoutineRunError>
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    Err(RoutineError::UnsupportedContextArchitecture.into())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) fn run_once<F>(routine: &Routine, entry: F) -> Result<(), RoutineRunError>
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    run_once_inner(routine, entry, None)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) fn run_once_with_permit<F>(
    routine: &Routine,
    entry: F,
    permit: crate::routine::ExecutionPermit,
) -> Result<(), RoutineRunError>
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    run_once_inner(routine, entry, Some(permit))
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn run_once_inner<F>(
    routine: &Routine,
    entry: F,
    supplied_permit: Option<crate::routine::ExecutionPermit>,
) -> Result<(), RoutineRunError>
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    let bounds = routine.stack_bounds()?;
    let initial_stack_pointer = MachineContext::initial_stack_pointer(bounds)?;
    let mut context = routine.inner.lock_context()?;
    if context.started {
        return Err(RoutineError::ContextAlreadyStarted.into());
    }
    let permit = match supplied_permit {
        Some(permit) => permit,
        None => routine.try_start()?,
    };
    let mut scheduler = MachineContext::empty();
    let routine_context = &mut context.machine as *mut MachineContext;
    let mut entry = Some(entry);
    let mut frame = SwitchFrame {
        scheduler: &mut scheduler,
        routine: routine_context,
        invoke_entry: invoke_entry::<F>,
        entry_data: (&mut entry as *mut Option<F>).cast(),
        outcome: None,
        permit: Some(permit),
    };
    context.started = true;
    context
        .machine
        .initialize(initial_stack_pointer, &mut frame as *mut _ as *mut ());

    // SAFETY: The permit has moved the routine to Running; its owned stack and
    // context stay locked and stable until the trampoline marks it Dead.
    unsafe {
        super::arch::x86_64::switch(&mut scheduler, &context.machine);
    }
    let outcome = frame
        .outcome
        .take()
        .unwrap_or_else(|| abort_context("routine trampoline returned without an outcome"));
    let permit = match frame.permit.take() {
        Some(permit) => permit,
        None => abort_context("scheduler resumed before the routine completed its permit"),
    };
    let completion = if outcome.is_err() {
        permit.complete_after_panic()
    } else {
        permit.complete()
    };
    if let Err(error) = completion {
        abort_context(&format!(
            "routine completion failed after scheduler return: {error}"
        ));
    }
    context.saved_stack_pointer = Some(
        context
            .machine
            .saved_stack_pointer()
            .and_then(NonZeroUsize::new)
            .ok_or(RoutineError::InvalidContextStackBound { value: 0 })?,
    );
    match outcome {
        Ok(()) => Ok(()),
        Err(payload) => Err(RoutineRunError::Panicked(payload)),
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) extern "C" fn enter_routine(frame_pointer: *mut SwitchFrame) -> ! {
    // SAFETY: The scheduler frame remains live on its suspended stack until
    // this routine switches back; the initial context stores its exact address.
    let (invoke_entry, entry_data) = unsafe {
        let frame = &*frame_pointer;
        (frame.invoke_entry, frame.entry_data)
    };
    // The Rust callback catches unwind before returning to this extern "C"
    // frame, so no panic can cross the assembly boundary.
    // SAFETY: The entry pointer references the live typed slot on the suspended
    // scheduler stack and is invoked exactly once.
    let outcome = unsafe { invoke_entry(entry_data) };
    // SAFETY: The scheduler frame remains live and suspended until we switch
    // back; the routine owns the frame exclusively while executing.
    let frame = unsafe { &mut *frame_pointer };
    frame.outcome = Some(outcome);

    // SAFETY: The live frame owns both contexts and the execution permit until
    // control has returned to the scheduler stack.
    let routine_context = unsafe { &mut *frame.routine };
    // SAFETY: The suspended scheduler frame remains live during this transfer.
    let scheduler_context = unsafe { &*frame.scheduler };
    // SAFETY: Completion has moved the routine to Dead, and both contexts are
    // valid and nonaliasing for the duration of this switch.
    unsafe { super::arch::x86_64::switch(routine_context, scheduler_context) };
    abort_context("completed routine context was resumed");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn invoke_entry<F>(entry_data: *mut ()) -> Result<(), RoutinePanicPayload>
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    // SAFETY: `run_once` keeps the typed entry slot alive on the suspended
    // scheduler stack until this callback has returned.
    let entry = unsafe { (*entry_data.cast::<Option<F>>()).take() };
    let entry = match entry {
        Some(entry) => entry,
        None => abort_context("routine trampoline invoked an empty entry"),
    };
    std::panic::catch_unwind(entry)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn abort_context(message: &str) -> ! {
    eprintln!("fatal: {message}");
    std::process::abort()
}
