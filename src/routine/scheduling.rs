use std::time::{Duration, Instant};

use super::error::RoutineError;

/// Cause for a routine to suspend while waiting for a runtime-managed event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitReason {
    /// Waiting for a synchronization primitive.
    Synchronization,
    /// Waiting for network or other readiness-based I/O.
    Io,
    /// Waiting for a timer deadline.
    Timer,
    /// Waiting for an event supplied by an external runtime component.
    External,
}

/// Reason reported for an active routine blocking interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockingReason {
    /// Waiting for a synchronization primitive.
    Synchronization,
    /// Waiting for network or other readiness-based I/O.
    Io,
    /// Waiting for a timer deadline.
    Timer,
    /// Waiting for an event supplied by an external runtime component.
    External,
    /// Executing an explicitly managed blocking system call.
    ManagedSyscall,
}

/// Current blocked state and the monotonic time at which it began.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoutineBlockingInfo {
    /// Why the routine is not currently runnable.
    pub reason: BlockingReason,
    /// Monotonic time at which the blocking interval began.
    pub since: Instant,
}

/// Consistent, read-only snapshot of routine scheduling and blocking data.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RoutineSchedulingInfo {
    /// Number of transitions into `Runnable`.
    pub runnable_entries: u64,
    /// Number of successful transitions into `Running`.
    pub execution_count: u64,
    /// Number of times execution yielded back to `Runnable`.
    pub yield_count: u64,
    /// Time accumulated while the routine was runnable but not running.
    pub runnable_time: Duration,
    /// Time accumulated while the routine held an execution permit.
    pub running_time: Duration,
    /// Start of the current runnable interval, if any.
    pub runnable_since: Option<Instant>,
    /// Start of the current execution interval, if any.
    pub running_since: Option<Instant>,
    /// Current wait/syscall information, if blocked.
    pub blocking: Option<RoutineBlockingInfo>,
    /// Time at which the routine became terminal.
    pub completed_at: Option<Instant>,
}

impl RoutineSchedulingInfo {
    pub(super) fn record_transition(
        &mut self,
        current: super::state::RoutineState,
        requested: super::state::RoutineState,
        blocking_reason: Option<BlockingReason>,
    ) -> Result<(), RoutineError> {
        use super::state::RoutineState;

        let now = Instant::now();
        if current == RoutineState::Runnable {
            if let Some(since) = self.runnable_since.take() {
                self.runnable_time = self
                    .runnable_time
                    .saturating_add(now.saturating_duration_since(since));
            }
        }
        if current == RoutineState::Running {
            if let Some(since) = self.running_since.take() {
                self.running_time = self
                    .running_time
                    .saturating_add(now.saturating_duration_since(since));
            }
        }

        match (current, requested) {
            (
                RoutineState::Created | RoutineState::Waiting | RoutineState::Syscall,
                RoutineState::Runnable,
            ) => {
                self.runnable_entries = self
                    .runnable_entries
                    .checked_add(1)
                    .ok_or(RoutineError::SchedulingCounterOverflow)?;
                self.runnable_since = Some(now);
                self.blocking = None;
            }
            (RoutineState::Running, RoutineState::Runnable) => {
                self.runnable_entries = self
                    .runnable_entries
                    .checked_add(1)
                    .ok_or(RoutineError::SchedulingCounterOverflow)?;
                self.yield_count = self
                    .yield_count
                    .checked_add(1)
                    .ok_or(RoutineError::SchedulingCounterOverflow)?;
                self.runnable_since = Some(now);
                self.blocking = None;
            }
            (RoutineState::Runnable, RoutineState::Running) => {
                self.execution_count = self
                    .execution_count
                    .checked_add(1)
                    .ok_or(RoutineError::SchedulingCounterOverflow)?;
                self.running_since = Some(now);
            }
            (RoutineState::Running, RoutineState::Waiting) => {
                self.blocking = Some(RoutineBlockingInfo {
                    reason: blocking_reason.unwrap_or(BlockingReason::Synchronization),
                    since: now,
                });
            }
            (RoutineState::Running, RoutineState::Syscall) => {
                self.blocking = Some(RoutineBlockingInfo {
                    reason: BlockingReason::ManagedSyscall,
                    since: now,
                });
            }
            (_, RoutineState::Dead) => {
                self.blocking = None;
                self.completed_at = Some(now);
            }
            _ => {}
        }
        Ok(())
    }
}
