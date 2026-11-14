use std::error::Error;
use std::hint::black_box;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::routine::{Routine, RoutineCompletion, RoutineError, RoutineMetadata, RoutineState};
use crate::stack::{StackBounds, StackConfig, StackManager};

static ENTRY_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENTRY_STACK_MARKER: AtomicUsize = AtomicUsize::new(0);
static ENTRY_SAW_RUNNING_STATE: AtomicUsize = AtomicUsize::new(0);
static ENTRY_ROUTINE: Mutex<Option<Routine>> = Mutex::new(None);
static CONTEXT_TEST_LOCK: Mutex<()> = Mutex::new(());

extern "C" fn record_entry() {
    let marker = 0_u8;
    let address = black_box(&marker) as *const u8 as usize;
    ENTRY_STACK_MARKER.store(address, Ordering::Release);
    ENTRY_CALLS.fetch_add(1, Ordering::AcqRel);
    let running = match ENTRY_ROUTINE.lock() {
        Ok(routine) => match routine.as_ref() {
            Some(routine) => {
                matches!(routine.state(), Ok(RoutineState::Running))
                    && matches!(
                        routine.context_info(),
                        Err(RoutineError::ContextUnavailableWhileRunning)
                    )
            }
            None => false,
        },
        Err(poisoned) => {
            drop(poisoned.into_inner());
            false
        }
    };
    ENTRY_SAW_RUNNING_STATE.store(usize::from(running), Ordering::Release);
}

#[test]
fn routine_runs_on_its_stack_and_returns_after_completing_state() -> Result<(), Box<dyn Error>> {
    let _test_guard = CONTEXT_TEST_LOCK
        .lock()
        .map_err(|error| format!("context test lock poisoned: {error}"))?;
    ENTRY_CALLS.store(0, Ordering::Release);
    ENTRY_STACK_MARKER.store(0, Ordering::Release);
    ENTRY_SAW_RUNNING_STATE.store(0, Ordering::Release);
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;
    *ENTRY_ROUTINE
        .lock()
        .map_err(|error| format!("test routine lock poisoned: {error}"))? = Some(routine.clone());
    routine.publish()?;
    let bounds = routine.stack_bounds()?;

    routine.run_once(record_entry)?;

    assert_eq!(ENTRY_CALLS.load(Ordering::Acquire), 1);
    assert_eq!(ENTRY_SAW_RUNNING_STATE.load(Ordering::Acquire), 1);
    let entry_stack = ENTRY_STACK_MARKER.load(Ordering::Acquire);
    assert!((bounds.usable_start..bounds.usable_end).contains(&entry_stack));
    assert_eq!(routine.state()?, RoutineState::Dead);
    assert_eq!(routine.completion()?, Some(RoutineCompletion::Completed));
    let context = routine.context_info()?;
    let saved_stack = context
        .saved_stack_pointer
        .ok_or("context SP was not saved")?;
    assert!((bounds.usable_start..bounds.usable_end).contains(&saved_stack));
    assert_eq!(saved_stack % 16, 8);
    ENTRY_ROUTINE
        .lock()
        .map_err(|error| format!("test routine lock poisoned: {error}"))?
        .take();
    Ok(())
}

#[test]
fn an_unpublished_routine_cannot_transfer_control() -> Result<(), Box<dyn Error>> {
    let _test_guard = CONTEXT_TEST_LOCK
        .lock()
        .map_err(|error| format!("context test lock poisoned: {error}"))?;
    ENTRY_CALLS.store(0, Ordering::Release);
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;

    assert!(matches!(
        routine.run_once(record_entry),
        Err(crate::routine::RoutineError::InvalidTransition {
            current: RoutineState::Created,
            requested: RoutineState::Running,
        })
    ));
    assert_eq!(ENTRY_CALLS.load(Ordering::Acquire), 0);
    assert_eq!(routine.state()?, RoutineState::Created);
    Ok(())
}

#[test]
fn rejects_empty_or_inverted_stack_bounds() {
    let empty = StackBounds {
        guard_start: 0,
        guard_end: 0,
        usable_start: 64,
        usable_end: 64,
    };
    assert!(matches!(
        super::machine::MachineContext::initial_stack_pointer(empty),
        Err(RoutineError::InvalidContextStackBounds)
    ));
    let too_small = StackBounds {
        guard_start: 0,
        guard_end: 0,
        usable_start: 64,
        usable_end: 128,
    };
    assert!(matches!(
        super::machine::MachineContext::initial_stack_pointer(too_small),
        Err(RoutineError::InsufficientContextStack)
    ));
}
