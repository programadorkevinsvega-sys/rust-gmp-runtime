use std::error::Error;
use std::sync::Arc;
use std::thread;

use crate::routine::{Routine, RoutineMetadata};
use crate::stack::{StackConfig, StackManager};

use super::{ProcessorError, ProcessorPool, ProcessorState};

fn runnable_routine() -> Result<Routine, Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;
    routine.publish()?;
    Ok(routine)
}

#[test]
fn pool_count_is_configurable_and_zero_is_rejected() -> Result<(), Box<dyn Error>> {
    let pool = ProcessorPool::new(3)?;
    assert_eq!(pool.len(), 3);
    assert_eq!(
        pool.iter()
            .map(|processor| processor.id().index())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(matches!(
        ProcessorPool::new(0),
        Err(ProcessorError::EmptyPool)
    ));
    Ok(())
}

#[test]
fn host_default_uses_reported_logical_parallelism() -> Result<(), Box<dyn Error>> {
    let expected = thread::available_parallelism()?.get();
    let pool = ProcessorPool::default_for_host()?;
    assert_eq!(pool.len(), expected);
    assert!(!pool.is_empty());
    Ok(())
}

#[test]
fn binding_is_exclusive_and_released_with_its_guard() -> Result<(), Box<dyn Error>> {
    let pool = ProcessorPool::new(1)?;
    let processor = pool
        .iter()
        .next()
        .ok_or_else(|| std::io::Error::other("pool unexpectedly empty"))?;
    let binding = processor.bind_current()?;
    assert!(matches!(processor.state()?, ProcessorState::Owned { .. }));
    assert!(matches!(
        processor.bind_current(),
        Err(ProcessorError::AlreadyOwned)
    ));
    drop(binding);
    assert_eq!(processor.state()?, ProcessorState::Unowned);
    Ok(())
}

#[test]
fn local_queue_accepts_only_unique_runnable_routines() -> Result<(), Box<dyn Error>> {
    let pool = ProcessorPool::new(1)?;
    let processor = pool
        .iter()
        .next()
        .ok_or_else(|| std::io::Error::other("pool unexpectedly empty"))?;
    let routine = runnable_routine()?;
    let mut binding = processor.bind_current()?;
    binding.enqueue(&routine)?;
    assert_eq!(binding.queue_len()?, 1);
    assert!(matches!(
        binding.enqueue(&routine),
        Err(ProcessorError::DuplicateRoutine(_))
    ));
    let dequeued = binding
        .dequeue()?
        .ok_or_else(|| std::io::Error::other("expected queued routine"))?;
    assert_eq!(dequeued.id(), routine.id());
    assert_eq!(binding.queue_len()?, 0);
    Ok(())
}

#[test]
fn local_queue_rejects_routines_before_publication() -> Result<(), Box<dyn Error>> {
    let pool = ProcessorPool::new(1)?;
    let processor = pool
        .iter()
        .next()
        .ok_or_else(|| std::io::Error::other("pool unexpectedly empty"))?;
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;
    let mut binding = processor.bind_current()?;
    assert!(matches!(
        binding.enqueue(&routine),
        Err(ProcessorError::RoutineNotRunnable { .. })
    ));
    assert_eq!(binding.queue_len()?, 0);
    Ok(())
}

#[test]
fn processor_pool_can_be_shared_between_threads() -> Result<(), Box<dyn Error>> {
    let pool = Arc::new(ProcessorPool::new(1)?);
    let other_pool = Arc::clone(&pool);
    let handle = thread::spawn(move || {
        let Some(processor) = other_pool.iter().next() else {
            return Err(ProcessorError::EmptyPool);
        };
        let _binding = processor.bind_current()?;
        Ok::<(), ProcessorError>(())
    });
    match handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => return Err(Box::new(error)),
        Err(payload) => std::panic::resume_unwind(payload),
    }
    let processor = pool
        .iter()
        .next()
        .ok_or_else(|| std::io::Error::other("pool unexpectedly empty"))?;
    assert_eq!(processor.state()?, ProcessorState::Unowned);
    Ok(())
}

#[test]
fn a_processor_cannot_be_bound_from_two_threads_at_once() -> Result<(), Box<dyn Error>> {
    let pool = Arc::new(ProcessorPool::new(1)?);
    let processor = pool
        .iter()
        .next()
        .ok_or_else(|| std::io::Error::other("pool unexpectedly empty"))?;
    let binding = processor.bind_current()?;
    let other_pool = Arc::clone(&pool);
    let handle = thread::spawn(move || {
        let Some(other_processor) = other_pool.iter().next() else {
            return false;
        };
        matches!(
            other_processor.bind_current(),
            Err(ProcessorError::AlreadyOwned)
        )
    });
    let rejected_second_binding = match handle.join() {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    assert!(rejected_second_binding);
    drop(binding);
    assert_eq!(processor.state()?, ProcessorState::Unowned);
    Ok(())
}
