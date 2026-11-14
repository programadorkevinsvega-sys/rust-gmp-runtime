use std::error::Error;

use super::Routine;
use crate::routine::{RoutineError, RoutineMetadata};
use crate::stack::{StackConfig, StackManager};

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
