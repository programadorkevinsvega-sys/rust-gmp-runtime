use super::*;

impl Routine {
    /// Returns context storage and stack bounds without claiming it is executable.
    ///
    /// # Errors
    ///
    /// Returns an error if the context or lifecycle lock is poisoned, or if
    /// the routine is currently executing.
    pub fn context_info(&self) -> Result<super::super::RoutineContextInfo, RoutineError> {
        if self.inner.lock_lifecycle()?.state() == RoutineState::Running {
            return Err(RoutineError::ContextUnavailableWhileRunning);
        }
        let context = self.inner.try_lock_context()?;
        Ok(context.info())
    }

    /// Executes one published routine entry on its owned stack and returns to
    /// the calling scheduler stack when the entry returns.
    ///
    /// Panics are caught on the routine stack and returned with their original
    /// payload. The entry must be `UnwindSafe`; unwind does not cross Assembly.
    ///
    /// # Errors
    ///
    /// Returns a panic payload if the entry panicked, or a runtime error if the
    /// routine could not be safely started.
    pub fn run_once<F>(&self, entry: F) -> Result<(), super::super::RoutineRunError>
    where
        F: FnOnce() + std::panic::UnwindSafe,
    {
        crate::context::switch::run_once(self, entry)
    }
}
