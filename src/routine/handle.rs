use std::sync::{Arc, Mutex};

use super::context_storage::RoutineContext;
use super::error::RoutineError;
use super::id::RoutineId;
use super::lifecycle::RoutineLifecycle;
use super::metadata::RoutineMetadata;
use super::permit::ExecutionPermit;
use super::scheduling::RoutineSchedulingInfo;
use super::state::{RoutineCompletion, RoutineState};
use crate::stack::StackBounds;
#[path = "handle/context.rs"]
mod context;
#[path = "handle/creation.rs"]
mod creation;
#[path = "handle/dispatch.rs"]
mod dispatch;

/// Shared handle to a routine's identity, lifecycle, metadata, and stack.
#[derive(Clone, Debug)]
pub struct Routine {
    pub(crate) inner: Arc<RoutineInner>,
}

#[derive(Debug)]
pub(crate) struct RoutineInner {
    pub(super) id: RoutineId,
    pub(super) metadata: RoutineMetadata,
    pub(super) stack: Mutex<crate::stack::Stack>,
    pub(crate) context: Mutex<RoutineContext>,
    pub(super) lifecycle: Mutex<RoutineLifecycle>,
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    pub(crate) benchmark: super::benchmark::RoutineBenchmark,
}

impl Routine {
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

    /// Returns a terminal routine's stack lease to its owning Processor cache.
    ///
    /// The routine must be terminal and the cache must belong to the same
    /// Go-inspired manager used to create its stack. On failure, the lease
    /// remains owned by the routine and will safely return to the global pool
    /// when dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if the routine is not `Dead`, a lock is poisoned, or
    /// the cache return fails.
    pub fn release_stack_to_processor_cache(
        &self,
        cache: &mut crate::stack::ProcessorStackCache,
    ) -> Result<(), RoutineError> {
        let lifecycle = self.inner.lock_lifecycle()?;
        if lifecycle.state() != RoutineState::Dead {
            return Err(RoutineError::StackReleaseBeforeCompletion {
                state: lifecycle.state(),
            });
        }
        drop(lifecycle);
        let mut context = self.inner.lock_context()?;
        let mut stack = self.inner.lock_stack()?;
        stack.release_to_processor_cache(cache)?;
        context.invalidate_stack();
        Ok(())
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
    /// Returns an error if the lifecycle lock is poisoned, the routine is not
    /// in `Created` or `Runnable`, or it is reserved for worker dispatch.
    pub fn discard(&self) -> Result<(), RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        if lifecycle.dispatch_reserved() {
            return Err(RoutineError::DispatchReserved);
        }
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
    /// Returns an error if the lifecycle lock is poisoned, the routine is not
    /// `Runnable`, or it is reserved for worker dispatch.
    pub fn try_start(&self) -> Result<ExecutionPermit, RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        if lifecycle.dispatch_reserved() {
            return Err(RoutineError::DispatchReserved);
        }
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
