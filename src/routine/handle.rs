use std::sync::{Arc, Mutex};

use super::context_storage::RoutineContext;
use super::error::RoutineError;
use super::id::RoutineId;
use super::lifecycle::RoutineLifecycle;
use super::metadata::RoutineMetadata;
use super::permit::ExecutionPermit;
use super::scheduling::RoutineSchedulingInfo;
use super::state::{RoutineCompletion, RoutineState};
use crate::stack::{StackBounds, StackManager};

/// Shared handle to a routine's identity, lifecycle, metadata, and stack.
#[derive(Clone, Debug)]
pub struct Routine {
    inner: Arc<RoutineInner>,
}

#[derive(Debug)]
pub(super) struct RoutineInner {
    pub(super) id: RoutineId,
    pub(super) metadata: RoutineMetadata,
    pub(super) stack: Mutex<crate::stack::Stack>,
    pub(super) context: Mutex<RoutineContext>,
    pub(super) lifecycle: Mutex<RoutineLifecycle>,
}

impl Routine {
    /// Allocates a routine identity and its stack, initially in `Created`.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier cannot be allocated or the stack
    /// manager cannot reserve a stack.
    pub fn new(
        stack_manager: &StackManager,
        metadata: RoutineMetadata,
    ) -> Result<Self, RoutineError> {
        let id = RoutineId::allocate()?;
        let stack = stack_manager.allocate()?;
        let context = RoutineContext::new(stack.bounds())?;
        Ok(Self {
            inner: Arc::new(RoutineInner {
                id,
                metadata,
                stack: Mutex::new(stack),
                context: Mutex::new(context),
                lifecycle: Mutex::new(RoutineLifecycle::new()),
            }),
        })
    }

    /// Returns the process-unique routine identifier.
    pub fn id(&self) -> RoutineId {
        self.inner.id
    }

    /// Returns immutable creation metadata.
    pub fn metadata(&self) -> &RoutineMetadata {
        &self.inner.metadata
    }

    /// Returns the current state or a lock-poisoning error.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned.
    pub fn state(&self) -> Result<RoutineState, RoutineError> {
        let lifecycle = self.inner.lock_lifecycle()?;
        Ok(lifecycle.state())
    }

    /// Returns the terminal disposition, if the routine has completed.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned.
    pub fn completion(&self) -> Result<Option<RoutineCompletion>, RoutineError> {
        let lifecycle = self.inner.lock_lifecycle()?;
        Ok(lifecycle.completion())
    }

    /// Returns the owned stack's current virtual-address bounds.
    ///
    /// # Errors
    ///
    /// Returns an error if the stack lock is poisoned.
    pub fn stack_bounds(&self) -> Result<StackBounds, RoutineError> {
        let stack = self.inner.lock_stack()?;
        Ok(stack.bounds())
    }

    /// Returns context storage and stack bounds without claiming it is executable.
    ///
    /// # Errors
    ///
    /// Returns an error if the context lock is poisoned.
    pub fn context_info(&self) -> Result<super::RoutineContextInfo, RoutineError> {
        let context = self.inner.lock_context()?;
        Ok(context.info())
    }

    /// Returns accumulated scheduler and blocking observations.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned.
    pub fn scheduling_info(&self) -> Result<RoutineSchedulingInfo, RoutineError> {
        let lifecycle = self.inner.lock_lifecycle()?;
        Ok(lifecycle.scheduling())
    }

    /// Publishes a newly created routine as eligible for execution.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is not
    /// in `Created`.
    pub fn publish(&self) -> Result<(), RoutineError> {
        self.transition_from(RoutineState::Created, RoutineState::Runnable, None, None)
    }

    /// Makes a waiting routine eligible after its operation completes.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is not
    /// in `Waiting`.
    pub fn wake(&self) -> Result<(), RoutineError> {
        self.transition_from(RoutineState::Waiting, RoutineState::Runnable, None, None)
    }

    /// Returns a routine from a managed blocking system call to the runnable set.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is not
    /// in `Syscall`.
    pub fn return_from_syscall(&self) -> Result<(), RoutineError> {
        self.transition_from(RoutineState::Syscall, RoutineState::Runnable, None, None)
    }

    /// Discards an unstarted routine without making it runnable.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is not
    /// in `Created` or `Runnable`.
    pub fn discard(&self) -> Result<(), RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        let current = lifecycle.state();
        if !matches!(current, RoutineState::Created | RoutineState::Runnable) {
            return Err(RoutineError::InvalidTransition {
                current,
                requested: RoutineState::Dead,
            });
        }
        lifecycle.transition(RoutineState::Dead, Some(RoutineCompletion::Abandoned), None)
    }

    /// Claims a runnable routine for exclusive execution.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is not
    /// `Runnable`.
    pub fn try_start(&self) -> Result<ExecutionPermit, RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        lifecycle.transition_from(RoutineState::Runnable, RoutineState::Running, None, None)?;
        drop(lifecycle);
        Ok(ExecutionPermit {
            inner: Arc::clone(&self.inner),
            active: true,
        })
    }

    fn transition_from(
        &self,
        expected: RoutineState,
        next: RoutineState,
        completion: Option<RoutineCompletion>,
        blocking_reason: Option<super::BlockingReason>,
    ) -> Result<(), RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        lifecycle.transition_from(expected, next, completion, blocking_reason)
    }
}

#[cfg(test)]
#[path = "handle_tests.rs"]
mod tests;
