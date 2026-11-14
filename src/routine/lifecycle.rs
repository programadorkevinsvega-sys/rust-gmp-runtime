use std::sync::MutexGuard;

use crate::stack::Stack;

use super::context_storage::RoutineContext;
use super::error::RoutineError;
use super::handle::RoutineInner;
use super::scheduling::{BlockingReason, RoutineSchedulingInfo};
use super::state::{RoutineCompletion, RoutineState};

#[derive(Debug)]
pub(super) struct RoutineLifecycle {
    state: RoutineState,
    completion: Option<RoutineCompletion>,
    scheduling: RoutineSchedulingInfo,
}

impl RoutineLifecycle {
    pub(super) fn new() -> Self {
        Self {
            state: RoutineState::Created,
            completion: None,
            scheduling: RoutineSchedulingInfo::default(),
        }
    }

    pub(super) fn state(&self) -> RoutineState {
        self.state
    }

    pub(super) fn completion(&self) -> Option<RoutineCompletion> {
        self.completion
    }

    pub(super) fn scheduling(&self) -> RoutineSchedulingInfo {
        self.scheduling
    }

    pub(super) fn transition(
        &mut self,
        requested: RoutineState,
        completion: Option<RoutineCompletion>,
        blocking_reason: Option<BlockingReason>,
    ) -> Result<(), RoutineError> {
        if !self.state.can_transition_to(requested) {
            return Err(RoutineError::InvalidTransition {
                current: self.state,
                requested,
            });
        }

        let mut scheduling = self.scheduling;
        scheduling.record_transition(self.state, requested, blocking_reason)?;
        self.state = requested;
        self.scheduling = scheduling;
        if requested == RoutineState::Dead {
            self.completion = completion;
        }
        Ok(())
    }

    pub(super) fn transition_from(
        &mut self,
        expected: RoutineState,
        requested: RoutineState,
        completion: Option<RoutineCompletion>,
        blocking_reason: Option<BlockingReason>,
    ) -> Result<(), RoutineError> {
        if self.state != expected {
            return Err(RoutineError::InvalidTransition {
                current: self.state,
                requested,
            });
        }
        self.transition(requested, completion, blocking_reason)
    }

    pub(super) fn abandon(&mut self) -> Result<(), RoutineError> {
        self.transition(RoutineState::Dead, Some(RoutineCompletion::Abandoned), None)
    }
}

impl RoutineInner {
    pub(super) fn lock_lifecycle(&self) -> Result<MutexGuard<'_, RoutineLifecycle>, RoutineError> {
        match self.lifecycle.lock() {
            Ok(lifecycle) => Ok(lifecycle),
            Err(error) => Err(RoutineError::StateLockPoisoned {
                message: error.to_string(),
            }),
        }
    }

    pub(super) fn lock_stack(&self) -> Result<MutexGuard<'_, Stack>, RoutineError> {
        match self.stack.lock() {
            Ok(stack) => Ok(stack),
            Err(error) => Err(RoutineError::StackLockPoisoned {
                message: error.to_string(),
            }),
        }
    }

    pub(super) fn lock_context(&self) -> Result<MutexGuard<'_, RoutineContext>, RoutineError> {
        match self.context.lock() {
            Ok(context) => Ok(context),
            Err(error) => Err(RoutineError::ContextLockPoisoned {
                message: error.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RoutineLifecycle;
    use crate::routine::{RoutineError, RoutineState};

    #[test]
    fn scheduling_counter_overflow_preserves_lifecycle_and_snapshot() {
        let mut lifecycle = RoutineLifecycle::new();
        lifecycle.scheduling.runnable_entries = u64::MAX;

        assert!(matches!(
            lifecycle.transition(RoutineState::Runnable, None, None),
            Err(RoutineError::SchedulingCounterOverflow)
        ));
        assert_eq!(lifecycle.state(), RoutineState::Created);
        assert_eq!(lifecycle.scheduling.runnable_entries, u64::MAX);
        assert_eq!(lifecycle.scheduling.runnable_since, None);
    }
}
