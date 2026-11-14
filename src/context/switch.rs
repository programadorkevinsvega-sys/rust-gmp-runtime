//! Checked routine entry and return around the architecture switch.

use super::machine::MachineContext;
use crate::routine::{Routine, RoutineError};
use std::num::NonZeroUsize;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) struct SwitchFrame {
    pub(crate) scheduler: *mut MachineContext,
    pub(crate) routine: *mut MachineContext,
    pub(crate) entry: extern "C" fn(),
    pub(crate) permit: Option<crate::routine::ExecutionPermit>,
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(crate) fn run_once(_routine: &Routine, _entry: extern "C" fn()) -> Result<(), RoutineError> {
    Err(RoutineError::UnsupportedContextArchitecture)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) fn run_once(routine: &Routine, entry: extern "C" fn()) -> Result<(), RoutineError> {
    let bounds = routine.stack_bounds()?;
    let initial_stack_pointer = MachineContext::initial_stack_pointer(bounds)?;
    let mut context = routine.inner.lock_context()?;
    if context.started {
        return Err(RoutineError::ContextAlreadyStarted);
    }
    let permit = routine.try_start()?;
    let mut scheduler = MachineContext::empty();
    let routine_context = &mut context.machine as *mut MachineContext;
    let mut frame = SwitchFrame {
        scheduler: &mut scheduler,
        routine: routine_context,
        entry,
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
    context.saved_stack_pointer = Some(
        context
            .machine
            .saved_stack_pointer()
            .and_then(NonZeroUsize::new)
            .ok_or(RoutineError::InvalidContextStackBound { value: 0 })?,
    );
    if frame.permit.is_some() {
        abort_context("scheduler resumed before the routine completed its permit");
    }
    Ok(())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) extern "C" fn enter_routine(frame_pointer: *mut SwitchFrame) -> ! {
    // SAFETY: The scheduler frame remains live on its suspended stack until
    // this routine switches back; the initial context stores its exact address.
    let entry = unsafe { (*frame_pointer).entry };
    entry();

    // SAFETY: The same live scheduler frame owns the permit and both contexts.
    let frame = unsafe { &mut *frame_pointer };
    let permit = match frame.permit.take() {
        Some(permit) => permit,
        None => abort_context("routine trampoline lost its execution permit"),
    };
    if let Err(error) = permit.complete() {
        abort_context(&format!(
            "routine completion failed before scheduler return: {error}"
        ));
    }

    // SAFETY: Completion has moved the routine to Dead, and both context
    // pointers remain valid for the duration of this switch.
    // SAFETY: The live frame owns both contexts until the stack transfer ends.
    let routine_context = unsafe { &mut *frame.routine };
    // SAFETY: The suspended scheduler frame remains live during this transfer.
    let scheduler_context = unsafe { &*frame.scheduler };
    // SAFETY: Completion has moved the routine to Dead, and both contexts are
    // valid and nonaliasing for the duration of this switch.
    unsafe { super::arch::x86_64::switch(routine_context, scheduler_context) };
    abort_context("completed routine context was resumed");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn abort_context(message: &str) -> ! {
    eprintln!("fatal: {message}");
    std::process::abort()
}
