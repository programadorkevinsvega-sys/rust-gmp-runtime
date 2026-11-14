use std::sync::Arc;

use super::error::RoutineError;
use super::handle::RoutineInner;
use super::id::RoutineId;
use super::lifecycle::RoutineLifecycle;
use super::scheduling::{BlockingReason, WaitReason};
use super::state::{RoutineCompletion, RoutineState};

/// Exclusive capability to advance one routine from `Running`.
#[derive(Debug)]
pub struct ExecutionPermit {
    pub(super) inner: Arc<RoutineInner>,
    pub(super) active: bool,
}

impl ExecutionPermit {
    /// Returns the identifier of the exclusively claimed routine.
    pub fn routine_id(&self) -> RoutineId {
        self.inner.id
    }

    /// Yields the routine back to the runnable state.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is no
    /// longer running.
    pub fn yield_to_scheduler(self) -> Result<(), RoutineError> {
        self.transition(RoutineState::Runnable, None, None)
    }

    /// Suspends the routine while a runtime-managed operation is pending.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is no
    /// longer running.
    pub fn wait(self) -> Result<(), RoutineError> {
        self.wait_for(WaitReason::Synchronization)
    }

    /// Suspends the routine and records the kind of event it awaits.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is no
    /// longer running.
    pub fn wait_for(self, reason: WaitReason) -> Result<(), RoutineError> {
        let blocking_reason = match reason {
            WaitReason::Synchronization => BlockingReason::Synchronization,
            WaitReason::Io => BlockingReason::Io,
            WaitReason::Timer => BlockingReason::Timer,
            WaitReason::External => BlockingReason::External,
        };
        self.transition(RoutineState::Waiting, None, Some(blocking_reason))
    }

    /// Marks the routine as executing an explicitly managed blocking syscall.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is no
    /// longer running.
    pub fn begin_syscall(self) -> Result<(), RoutineError> {
        self.transition(
            RoutineState::Syscall,
            None,
            Some(BlockingReason::ManagedSyscall),
        )
    }

    /// Records normal completion and makes the routine terminal.
    ///
    /// # Errors
    ///
    /// Returns an error if the lifecycle lock is poisoned or the routine is no
    /// longer running.
    pub fn complete(self) -> Result<(), RoutineError> {
        self.finish(RoutineCompletion::Completed)
    }

    pub(crate) fn complete_after_panic(self) -> Result<(), RoutineError> {
        self.finish(RoutineCompletion::Panicked)
    }

    fn finish(self, completion: RoutineCompletion) -> Result<(), RoutineError> {
        self.transition(RoutineState::Dead, Some(completion), None)
    }

    fn transition(
        mut self,
        next: RoutineState,
        completion: Option<RoutineCompletion>,
        blocking_reason: Option<BlockingReason>,
    ) -> Result<(), RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        lifecycle.transition_from(RoutineState::Running, next, completion, blocking_reason)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for ExecutionPermit {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        match self.inner.lifecycle.lock() {
            Ok(mut lifecycle) => {
                if let Err(error) = lifecycle.abandon() {
                    abort_invalid_permit_drop(&self.inner.id, error);
                }
            }
            Err(error) => {
                abort_poisoned_permit_drop(&self.inner.id, error);
            }
        }
    }
}

fn abort_invalid_permit_drop(id: &RoutineId, error: RoutineError) -> ! {
    eprintln!(
        "fatal: could not safely abandon routine {}: {error}",
        id.get()
    );
    std::process::abort()
}

fn abort_poisoned_permit_drop(
    id: &RoutineId,
    error: std::sync::PoisonError<std::sync::MutexGuard<'_, RoutineLifecycle>>,
) -> ! {
    eprintln!(
        "fatal: routine {} lifecycle lock poisoned during permit drop: {error}",
        id.get()
    );
    std::process::abort()
}
