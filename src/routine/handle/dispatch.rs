use super::super::error::RoutineError;
use super::super::handle::Routine;
use super::super::permit::ExecutionPermit;

impl Routine {
    pub(crate) fn reserve_for_dispatch(&self) -> Result<(), RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        lifecycle.reserve_dispatch()
    }

    pub(crate) fn try_start_dispatched(&self) -> Result<ExecutionPermit, RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        lifecycle.claim_dispatch()?;
        drop(lifecycle);
        Ok(ExecutionPermit {
            inner: std::sync::Arc::clone(&self.inner),
            active: true,
        })
    }

    pub(crate) fn abandon_dispatch(&self) -> Result<bool, RoutineError> {
        let mut lifecycle = self.inner.lock_lifecycle()?;
        lifecycle.abandon_dispatch()
    }
}
