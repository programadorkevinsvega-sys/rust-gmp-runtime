use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use crate::machine::MachineManager;
use crate::processor::ProcessorPool;
use crate::routine::{Routine, RoutineError, RoutineMetadata, RoutineState};
use crate::stack::{StackConfig, StackManager};

#[test]
fn queued_routine_cannot_be_claimed_or_discarded_by_a_clone() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(16 * 1024)?)?;
    let mut machines = MachineManager::start(Arc::new(ProcessorPool::new(1)?))?;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = Routine::new(&stack_manager, RoutineMetadata::new())?;
    let first_handle = machines.dispatch(first.clone(), move || {
        if entered_tx.send(()).is_ok() {
            match release_rx.recv_timeout(Duration::from_secs(3)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {}
            }
        }
    })?;
    entered_rx.recv_timeout(Duration::from_secs(2))?;

    let queued = Routine::new(&stack_manager, RoutineMetadata::new())?;
    let queued_handle = machines.dispatch(queued.clone(), || {})?;
    let discard = queued.discard();
    let claim = queued.try_start();
    let follower_runs = Arc::new(AtomicUsize::new(0));
    let follower_count = Arc::clone(&follower_runs);
    let follower = Routine::new(&stack_manager, RoutineMetadata::new())?;
    let follower_handle = machines.dispatch(follower.clone(), move || {
        follower_count.fetch_add(1, Ordering::SeqCst);
    })?;

    release_tx.send(())?;
    first_handle.join()?;
    queued_handle.join()?;
    follower_handle.join()?;
    machines.shutdown()?;

    assert!(matches!(discard, Err(RoutineError::DispatchReserved)));
    assert!(matches!(claim, Err(RoutineError::DispatchReserved)));
    assert_eq!(queued.state()?, RoutineState::Dead);
    assert_eq!(
        queued.completion()?,
        Some(crate::routine::RoutineCompletion::Completed)
    );
    assert_eq!(follower_runs.load(Ordering::SeqCst), 1);
    assert_eq!(follower.state()?, RoutineState::Dead);
    Ok(())
}

#[test]
fn join_waits_until_the_worker_releases_its_routine_reference() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(16 * 1024)?)?;
    let mut machines = MachineManager::start(Arc::new(ProcessorPool::new(1)?))?;
    let routine = Routine::new(&stack_manager, RoutineMetadata::new())?;
    let handle = machines.dispatch(routine, || {})?;
    handle.join()?;
    assert_eq!(stack_manager.metrics()?.active_stacks, 0);
    machines.shutdown()?;
    Ok(())
}
