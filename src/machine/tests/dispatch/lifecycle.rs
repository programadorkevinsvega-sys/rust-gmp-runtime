use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use crate::machine::{MachineError, MachineManager, RoutineJoinError};
use crate::processor::ProcessorPool;
use crate::routine::RoutineCompletion;
use crate::stack::{StackConfig, StackManager};

use super::super::new_routine;

#[test]
fn dispatched_panic_is_reported_without_killing_the_worker() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let processors = Arc::new(ProcessorPool::new(1)?);
    let mut machines = MachineManager::start(processors)?;
    let panicked = new_routine(&stack_manager)?;
    let panic_result = machines
        .dispatch(panicked.clone(), || {
            std::panic::resume_unwind(Box::new("routine failure"))
        })?
        .join();
    assert!(matches!(
        panic_result,
        Err(RoutineJoinError::Execution(
            crate::routine::RoutineRunError::Panicked(_)
        ))
    ));
    assert_eq!(panicked.state()?, crate::routine::RoutineState::Dead);
    assert_eq!(panicked.completion()?, Some(RoutineCompletion::Panicked));

    let completed = new_routine(&stack_manager)?;
    machines.dispatch(completed.clone(), || {})?.join()?;
    assert_eq!(completed.completion()?, Some(RoutineCompletion::Completed));
    machines.shutdown()?;
    Ok(())
}

#[test]
fn dispatch_after_shutdown_is_rejected_and_routine_is_discarded() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let processors = Arc::new(ProcessorPool::new(1)?);
    let mut machines = MachineManager::start(processors)?;
    machines.shutdown()?;
    let routine = new_routine(&stack_manager)?;
    let error = match machines.dispatch(routine.clone(), || {}) {
        Ok(_) => return Err("dispatch after shutdown unexpectedly succeeded".into()),
        Err(error) => error,
    };
    let message = match error {
        MachineError::DispatchRejected { message, .. } => message,
        unexpected @ (MachineError::EmptyProcessorPool
        | MachineError::StackManagerNotConfigured
        | MachineError::ProcessorCachesRequireGoInspired
        | MachineError::Routine(_)
        | MachineError::WorkerChannelClosed { .. }
        | MachineError::ThreadSpawn { .. }
        | MachineError::StartupCleanup { .. }
        | MachineError::ControlPoisoned { .. }
        | MachineError::WorkerPanicked { .. }
        | MachineError::WorkerFailed { .. }
        | MachineError::SchedulerQueue(_)) => return Err(Box::new(unexpected)),
    };
    assert!(!message.contains("rollback failed"));
    assert_eq!(machines.scheduler_stats()?.rejected_publications, 1);
    assert_eq!(routine.state()?, crate::routine::RoutineState::Dead);
    assert_eq!(routine.completion()?, Some(RoutineCompletion::Abandoned));
    Ok(())
}

#[test]
fn shutdown_drains_work_over_the_injection_queue_capacity() -> Result<(), Box<dyn Error>> {
    const QUEUED_ROUTINES: usize = 1_025;

    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    let processors = Arc::new(ProcessorPool::new(1)?);
    let mut machines = MachineManager::start(processors)?;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = new_routine(&stack_manager)?;
    let first_handle = machines.dispatch(first, move || {
        assert!(entered_tx.send(()).is_ok());
        assert!(release_rx.recv().is_ok());
    })?;
    entered_rx.recv_timeout(Duration::from_secs(2))?;

    let executed = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(QUEUED_ROUTINES);
    for _ in 0..QUEUED_ROUTINES {
        let routine = new_routine(&stack_manager)?;
        let executed_by_entry = Arc::clone(&executed);
        handles.push(machines.dispatch(routine, move || {
            executed_by_entry.fetch_add(1, Ordering::SeqCst);
        })?);
    }

    release_tx.send(())?;
    machines.shutdown()?;
    first_handle.join()?;
    for handle in handles {
        handle.join()?;
    }
    assert_eq!(executed.load(Ordering::SeqCst), QUEUED_ROUTINES);
    Ok(())
}
