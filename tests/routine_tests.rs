//! Integration tests for the routine identity and lifecycle API.

use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use rust_gmp_runtime::routine::{
    BlockingReason, Routine, RoutineCompletion, RoutineError, RoutineMetadata, RoutineState,
};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

fn make_routine() -> Result<Routine, Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    Ok(Routine::new(&manager, RoutineMetadata::new())?)
}

#[test]
fn routine_lifecycle_requires_valid_transitions() -> Result<(), Box<dyn Error>> {
    let routine = make_routine()?;
    assert_eq!(routine.state()?, RoutineState::Created);
    assert!(routine.wake().is_err());

    routine.publish()?;
    let permit = routine.try_start()?;
    assert!(routine.try_start().is_err());
    permit.yield_to_scheduler()?;

    assert!(routine.publish().is_err());
    assert_eq!(routine.state()?, RoutineState::Runnable);
    routine.try_start()?.complete()?;
    assert_eq!(routine.state()?, RoutineState::Dead);
    assert_eq!(routine.completion()?, Some(RoutineCompletion::Completed));
    Ok(())
}

#[test]
fn wait_and_syscall_returns_use_distinct_states() -> Result<(), Box<dyn Error>> {
    let routine = make_routine()?;
    routine.publish()?;
    routine.try_start()?.wait()?;
    assert_eq!(routine.state()?, RoutineState::Waiting);
    assert_eq!(
        routine
            .scheduling_info()?
            .blocking
            .map(|information| information.reason),
        Some(BlockingReason::Synchronization)
    );
    routine.wake()?;
    routine.try_start()?.begin_syscall()?;
    assert_eq!(routine.state()?, RoutineState::Syscall);
    routine.return_from_syscall()?;
    routine.try_start()?.complete()?;
    Ok(())
}

#[test]
fn dropping_an_execution_permit_prevents_reexecution() -> Result<(), Box<dyn Error>> {
    let routine = make_routine()?;
    routine.publish()?;
    drop(routine.try_start()?);
    assert_eq!(routine.state()?, RoutineState::Dead);
    assert_eq!(routine.completion()?, Some(RoutineCompletion::Abandoned));
    assert!(routine.try_start().is_err());
    Ok(())
}

#[test]
fn concurrent_workers_cannot_claim_the_same_routine() -> Result<(), Box<dyn Error>> {
    let routine = make_routine()?;
    routine.publish()?;
    let barrier = Arc::new(Barrier::new(2));
    let successful_claims = Arc::new(AtomicUsize::new(0));
    let mut workers = Vec::new();

    for _ in 0..2 {
        let routine = routine.clone();
        let barrier = Arc::clone(&barrier);
        let successful_claims = Arc::clone(&successful_claims);
        workers.push(thread::spawn(move || {
            barrier.wait();
            match routine.try_start() {
                Ok(permit) => {
                    successful_claims.fetch_add(1, Ordering::Relaxed);
                    permit.complete()
                }
                Err(error) => match error {
                    RoutineError::InvalidTransition { .. } => Ok(()),
                    other @ (RoutineError::Identifier(_)
                    | RoutineError::Stack(_)
                    | RoutineError::StateLockPoisoned { .. }
                    | RoutineError::StackLockPoisoned { .. }
                    | RoutineError::ContextLockPoisoned { .. }
                    | RoutineError::InvalidContextStackBound { .. }
                    | RoutineError::SchedulingCounterOverflow) => Err(other),
                },
            }
        }));
    }

    for worker in workers {
        match worker.join() {
            Ok(result) => result?,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    assert_eq!(successful_claims.load(Ordering::Relaxed), 1);
    assert_eq!(routine.state()?, RoutineState::Dead);
    Ok(())
}

#[test]
fn routine_retains_its_stack_until_all_handles_are_dropped() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new().with_name("worker"))?;
    let other_handle = routine.clone();
    assert_eq!(routine.metadata().name(), Some("worker"));
    assert_eq!(manager.metrics()?.active_stacks, 1);
    drop(routine);
    assert_eq!(manager.metrics()?.active_stacks, 1);
    drop(other_handle);
    assert_eq!(manager.metrics()?.active_stacks, 0);
    Ok(())
}

#[test]
fn execution_permit_retains_stack_after_public_handle_is_dropped() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;
    routine.publish()?;
    let permit = routine.try_start()?;
    drop(routine);

    assert_eq!(manager.metrics()?.active_stacks, 1);
    permit.complete()?;
    assert_eq!(manager.metrics()?.active_stacks, 0);
    Ok(())
}
