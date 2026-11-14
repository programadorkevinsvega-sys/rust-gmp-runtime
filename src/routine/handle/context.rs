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
    /// The entry uses the C ABI, so unwinding across it is not supported in
    /// this subphase. Panic capture and an unwind-safe entry API are deferred
    /// to Phase 3.2.
    ///
    /// # Errors
    ///
    /// Returns an error if the routine is not runnable, the context is invalid,
    /// or a runtime lock is poisoned.
    pub fn run_once(&self, entry: extern "C" fn()) -> Result<(), RoutineError> {
        crate::context::switch::run_once(self, entry)
    }
}
