use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::machine::{MachineError, MachineManager};
use crate::processor::ProcessorPool;
use crate::routine::{RoutineCompletion, RoutineMetadata, RoutineState};
use crate::stack::{StackConfig, StackEngine, StackManager};

#[test]
fn go_inspired_stack_runs_on_machine_and_releases_after_completion() -> Result<(), Box<dyn Error>> {
    let stacks =
        StackManager::new(StackConfig::new(32 * 1024)?.with_engine(StackEngine::GoInspired))?;
    let routine = crate::routine::Routine::new(&stacks, RoutineMetadata::new())?;
    let processors = Arc::new(ProcessorPool::new(1)?);
    let mut machines = MachineManager::start(processors)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let closure_executions = Arc::clone(&executions);
    machines
        .dispatch(routine.clone(), move || {
            closure_executions.fetch_add(1, Ordering::SeqCst);
        })?
        .join()?;
    machines.shutdown()?;

    assert_eq!(routine.completion()?, Some(RoutineCompletion::Completed));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(routine.state()?, RoutineState::Dead);
    drop(routine);
    assert_eq!(stacks.metrics()?.active_stacks, 0);
    assert_eq!(stacks.engine(), StackEngine::GoInspired);
    assert!(stacks.trim_to_retained_spans(0)? > 0);
    Ok(())
}

#[test]
fn worker_created_routines_reuse_the_processor_owned_stack_cache() -> Result<(), Box<dyn Error>> {
    const ROUTINE_COUNT: usize = 32;
    let stacks = Arc::new(StackManager::new(
        StackConfig::new(32 * 1024)?.with_engine(StackEngine::GoInspired),
    )?);
    let processors = Arc::new(ProcessorPool::new(2)?);
    let mut machines =
        MachineManager::start_with_processor_caches(Arc::clone(&processors), Arc::clone(&stacks))?;
    let executions = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(ROUTINE_COUNT);
    for _ in 0..ROUTINE_COUNT {
        let executions = Arc::clone(&executions);
        handles.push(machines.dispatch_new(RoutineMetadata::new(), move || {
            executions.fetch_add(1, Ordering::SeqCst);
        })?);
    }
    for handle in handles {
        handle.join()?;
    }
    machines.shutdown()?;

    let metrics = stacks.metrics()?;
    let cache_metrics = metrics
        .go_inspired
        .as_ref()
        .ok_or("missing Go-inspired metrics")?;
    assert_eq!(executions.load(Ordering::SeqCst), ROUTINE_COUNT);
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(cache_metrics.allocations, ROUTINE_COUNT as u64);
    assert_eq!(cache_metrics.releases, ROUTINE_COUNT as u64);
    assert!(cache_metrics.cache_hits >= (ROUTINE_COUNT - processors.len()) as u64);
    assert!(cache_metrics.cache_misses <= processors.len() as u64);
    assert_eq!(
        cache_metrics.cache_hits + cache_metrics.cache_misses,
        ROUTINE_COUNT as u64
    );
    assert_eq!(cache_metrics.local_free_slots, 0);
    assert_eq!(cache_metrics.active_stacks, 0);
    Ok(())
}

#[test]
fn worker_side_dispatch_requires_a_stack_manager_and_go_inspired_caches()
-> Result<(), Box<dyn Error>> {
    let processors = Arc::new(ProcessorPool::new(1)?);
    let mut machines = MachineManager::start(Arc::clone(&processors))?;
    assert!(matches!(
        machines.dispatch_new(RoutineMetadata::new(), || {}),
        Err(MachineError::StackManagerNotConfigured)
    ));
    machines.shutdown()?;

    let fixed = Arc::new(StackManager::new(StackConfig::new(32 * 1024)?)?);
    assert!(matches!(
        MachineManager::start_with_processor_caches(processors, fixed),
        Err(MachineError::ProcessorCachesRequireGoInspired)
    ));
    Ok(())
}
