use std::error::Error;

use super::Routine;
use crate::routine::{RoutineError, RoutineMetadata};
use crate::stack::{GoInspiredOptions, StackConfig, StackEngine, StackManager};

#[test]
fn context_lock_poisoning_is_reported() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;
    let poison_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _context_guard = match routine.inner.context.lock() {
            Ok(context) => context,
            Err(error) => std::panic::resume_unwind(Box::new(error.to_string())),
        };
        std::panic::resume_unwind(Box::new("poison context lock for regression test"));
    }));

    match poison_result {
        Err(payload) => drop(payload),
        Ok(()) => return Err("context lock was not poisoned".into()),
    }

    assert!(matches!(
        routine.context_info(),
        Err(RoutineError::ContextLockPoisoned { .. })
    ));
    let stack_bounds = routine.stack_bounds()?;
    assert!(stack_bounds.usable_start < stack_bounds.usable_end);
    Ok(())
}

#[test]
fn completed_routine_returns_its_stack_to_the_processor_cache() -> Result<(), Box<dyn Error>> {
    let options = GoInspiredOptions::default()
        .with_slots_per_span(8)
        .with_retained_spans(0);
    let manager = StackManager::with_go_inspired_options(
        StackConfig::new(8 * 1024)?.with_engine(StackEngine::GoInspired),
        options,
    )?;
    let mut cache = manager.processor_cache()?;
    let routine = Routine::new_with_processor_cache(&manager, &mut cache, RoutineMetadata::new())?;
    assert!(matches!(
        routine.release_stack_to_processor_cache(&mut cache),
        Err(RoutineError::StackReleaseBeforeCompletion { .. })
    ));

    routine.publish()?;
    routine.run_once(|| {})?;
    routine.release_stack_to_processor_cache(&mut cache)?;
    assert_eq!(routine.context_info()?.stack_upper_bound, 0);
    let bounds = routine.stack_bounds()?;
    assert_eq!(bounds.usable_start, bounds.usable_end);
    let metrics = manager.metrics()?;
    let go_metrics = metrics
        .go_inspired
        .ok_or("Go-inspired metrics missing from Go-inspired stack manager")?;
    assert_eq!(go_metrics.active_stacks, 0);
    assert_eq!(cache.len(), 1);
    Ok(())
}
