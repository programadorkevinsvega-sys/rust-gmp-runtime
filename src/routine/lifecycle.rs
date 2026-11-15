use std::sync::{MutexGuard, TryLockError};

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
    dispatch_reserved: bool,
}

impl RoutineLifecycle {
    pub(super) fn new() -> Self {
        Self {
            state: RoutineState::Created,
            completion: None,
            scheduling: RoutineSchedulingInfo::default(),
            dispatch_reserved: false,
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

    pub(super) fn dispatch_reserved(&self) -> bool {
        self.dispatch_reserved
    }

    pub(super) fn reserve_dispatch(&mut self) -> Result<(), RoutineError> {
        self.transition_from(RoutineState::Created, RoutineState::Runnable, None, None)?;
        self.dispatch_reserved = true;
        Ok(())
    }

    pub(super) fn claim_dispatch(&mut self) -> Result<(), RoutineError> {
        if !self.dispatch_reserved {
            return Err(RoutineError::InvalidTransition {
                current: self.state,
                requested: RoutineState::Running,
            });
        }
        self.transition_from(RoutineState::Runnable, RoutineState::Running, None, None)?;
        self.dispatch_reserved = false;
        Ok(())
    }

    pub(super) fn abandon_dispatch(&mut self) -> Result<bool, RoutineError> {
        if !self.dispatch_reserved {
            return Ok(false);
        }
        self.transition_from(
            RoutineState::Runnable,
            RoutineState::Dead,
            Some(RoutineCompletion::Abandoned),
            None,
        )?;
        self.dispatch_reserved = false;
        Ok(true)
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

    pub(crate) fn lock_context(&self) -> Result<MutexGuard<'_, RoutineContext>, RoutineError> {
        match self.context.lock() {
            Ok(context) => Ok(context),
            Err(error) => Err(RoutineError::ContextLockPoisoned {
                message: error.to_string(),
            }),
        }
    }

    pub(crate) fn try_lock_context(&self) -> Result<MutexGuard<'_, RoutineContext>, RoutineError> {
        match self.context.try_lock() {
            Ok(context) => Ok(context),
            Err(TryLockError::WouldBlock) => Err(RoutineError::ContextUnavailableWhileRunning),
            Err(TryLockError::Poisoned(error)) => Err(RoutineError::ContextLockPoisoned {
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
    fn dispatch_reservation_is_claimed_or_abandoned_only_once() -> Result<(), RoutineError> {
        let mut claimed = RoutineLifecycle::new();
        claimed.reserve_dispatch()?;
        assert!(claimed.dispatch_reserved());
        claimed.claim_dispatch()?;
        assert_eq!(claimed.state(), RoutineState::Running);
        assert!(!claimed.dispatch_reserved());
        assert!(matches!(
            claimed.claim_dispatch(),
            Err(RoutineError::InvalidTransition { .. })
        ));

        let mut abandoned = RoutineLifecycle::new();
        abandoned.reserve_dispatch()?;
        assert!(abandoned.abandon_dispatch()?);
        assert_eq!(abandoned.state(), RoutineState::Dead);
        assert!(!abandoned.dispatch_reserved());
        assert!(!abandoned.abandon_dispatch()?);
        Ok(())
    }

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
