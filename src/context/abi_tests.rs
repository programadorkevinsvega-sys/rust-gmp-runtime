use std::error::Error;
use std::hint::black_box;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::machine::MachineContext;
use crate::routine::{Routine, RoutineMetadata, RoutineState};
use crate::stack::{StackConfig, StackManager};

use super::abi_test_support::{
    FpControlGuard, context_bootstrap_address, oxido_test_run_context_probe,
    oxido_test_stack_pointer,
};

static CONTEXT_TEST_LOCK: Mutex<()> = Mutex::new(());
static REPEATED_ENTRY_COUNT: AtomicUsize = AtomicUsize::new(0);
static REPEATED_ENTRY_STACK: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct ContextSwitchFrame {
    scheduler: *mut MachineContext,
    routine: *mut MachineContext,
}

fn repeated_entry() {
    // SAFETY: The assembly helper only reads its current stack pointer.
    let stack_pointer = unsafe { oxido_test_stack_pointer() };
    REPEATED_ENTRY_STACK.store(stack_pointer, Ordering::Release);
    REPEATED_ENTRY_COUNT.fetch_add(nested_rust_call(), Ordering::AcqRel);
}

#[inline(never)]
fn nested_rust_call() -> usize {
    black_box(1)
}

#[test]
fn repeated_one_shot_switches_preserve_stack_and_rust_call_abi() -> Result<(), Box<dyn Error>> {
    let _test_guard = CONTEXT_TEST_LOCK
        .lock()
        .map_err(|error| format!("context test lock poisoned: {error}"))?;
    const ITERATIONS: usize = 32;
    REPEATED_ENTRY_COUNT.store(0, Ordering::Release);
    REPEATED_ENTRY_STACK.store(0, Ordering::Release);
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;

    for _ in 0..ITERATIONS {
        let routine = Routine::new(&manager, RoutineMetadata::new())?;
        routine.publish()?;
        let bounds = routine.stack_bounds()?;
        routine.run_once(repeated_entry)?;

        assert_eq!(routine.state()?, RoutineState::Dead);
        let stack_pointer = REPEATED_ENTRY_STACK.load(Ordering::Acquire);
        assert!((bounds.usable_start..bounds.usable_end).contains(&stack_pointer));
        assert_eq!(stack_pointer % 16, 8);
    }

    assert_eq!(REPEATED_ENTRY_COUNT.load(Ordering::Acquire), ITERATIONS);
    Ok(())
}

#[test]
fn context_switch_preserves_callee_saved_registers() -> Result<(), Box<dyn Error>> {
    let _test_guard = CONTEXT_TEST_LOCK
        .lock()
        .map_err(|error| format!("context test lock poisoned: {error}"))?;
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let stack = manager.allocate()?;
    let stack_pointer = MachineContext::initial_stack_pointer(stack.bounds())?;
    let mut scheduler = MachineContext::empty();
    let mut routine = MachineContext::empty();
    let mut frame = ContextSwitchFrame {
        scheduler: &mut scheduler,
        routine: &mut routine,
    };
    routine.initialize_test(
        stack_pointer,
        context_bootstrap_address(),
        (&mut frame as *mut ContextSwitchFrame).cast(),
    );

    // SAFETY: The test contexts, frame, and owned stack remain live throughout
    // the round trip; the bootstrap switches back without unwinding.
    let preserved = unsafe { oxido_test_run_context_probe(&mut scheduler, &routine) };

    assert_eq!(preserved, 0x3f);
    let saved_stack_pointer = routine
        .saved_stack_pointer()
        .ok_or("round-trip did not save the routine stack pointer")?;
    assert!(
        (stack.bounds().usable_start..stack.bounds().usable_end).contains(&saved_stack_pointer)
    );
    Ok(())
}

#[test]
fn context_switch_restores_x87_and_mxcsr_control_words() -> Result<(), Box<dyn Error>> {
    let _test_guard = CONTEXT_TEST_LOCK
        .lock()
        .map_err(|error| format!("context test lock poisoned: {error}"))?;
    let control_guard = FpControlGuard::with_rounding_toward_zero();
    let expected = FpControlGuard::current_control_words();
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;
    routine.publish()?;
    routine.run_once(|| black_box(()))?;
    let actual = FpControlGuard::current_control_words();
    drop(control_guard);

    assert_eq!(actual.0 & 0x0c00, expected.0 & 0x0c00);
    assert_eq!(actual.1 & 0x6000, expected.1 & 0x6000);
    Ok(())
}
