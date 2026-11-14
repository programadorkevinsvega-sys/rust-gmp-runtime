use std::sync::MutexGuard;

use super::admission::VmaAdmission;
use super::allocator::StackManager;
#[cfg(feature = "allocator-benchmark")]
use super::benchmark_metrics::TimingKind;
use super::error::StackError;
use super::vma::{VmaSnapshot, read_vma_snapshot};

impl StackManager {
    pub(super) fn admission_with_snapshot(
        &self,
        force_refresh: bool,
    ) -> Result<(MutexGuard<'_, VmaAdmission>, VmaSnapshot), StackError> {
        self.admission_with_snapshot_using(force_refresh, read_vma_snapshot)
    }

    pub(super) fn check_admission_with_reconciliation(
        &self,
        count: usize,
        safety_margin: u64,
    ) -> Result<(MutexGuard<'_, VmaAdmission>, VmaSnapshot), StackError> {
        self.check_admission_with_reconciliation_using(count, safety_margin, read_vma_snapshot)
    }

    pub(super) fn check_admission_with_reconciliation_using(
        &self,
        count: usize,
        safety_margin: u64,
        read_snapshot: impl FnMut() -> Result<VmaSnapshot, StackError>,
    ) -> Result<(MutexGuard<'_, VmaAdmission>, VmaSnapshot), StackError> {
        let (mut admission, _) = self.admission_with_snapshot(false)?;
        #[cfg(feature = "allocator-benchmark")]
        let _hold = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        match admission.check_cached(count, safety_margin) {
            Ok(snapshot) => Ok((admission, snapshot)),
            Err(StackError::VmaPreflightRejected { .. }) => {
                let generation = admission.snapshot_generation();
                #[cfg(feature = "allocator-benchmark")]
                drop(_hold);
                drop(admission);
                admission = self.refresh_after_rejection_using(generation, read_snapshot)?;
                #[cfg(feature = "allocator-benchmark")]
                let _hold = self
                    .benchmark_metrics
                    .as_ref()
                    .map(|metrics| metrics.time(TimingKind::LockHold));
                match admission.check_cached(count, safety_margin) {
                    Ok(snapshot) => Ok((admission, snapshot)),
                    Err(retry_error @ StackError::VmaPreflightRejected { .. }) => {
                        match self.inner.record_vma_preflight_rejection() {
                            Ok(()) => Err(retry_error),
                            Err(counter_error) => Err(StackError::CleanupAfterFailure {
                                original: Box::new(retry_error),
                                cleanup: Box::new(counter_error),
                            }),
                        }
                    }
                    Err(retry_error) => Err(retry_error),
                }
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn refresh_after_rejection_using(
        &self,
        rejected_generation: u64,
        mut read_snapshot: impl FnMut() -> Result<VmaSnapshot, StackError>,
    ) -> Result<MutexGuard<'_, VmaAdmission>, StackError> {
        let mut admission = self.lock_admission()?;
        loop {
            #[cfg(feature = "allocator-benchmark")]
            let _hold = self
                .benchmark_metrics
                .as_ref()
                .map(|metrics| metrics.time(TimingKind::LockHold));
            if admission.snapshot_generation() != rejected_generation {
                return Ok(admission);
            }
            if admission.is_refreshing() {
                #[cfg(test)]
                admission.add_refresh_waiter()?;
                #[cfg(feature = "allocator-benchmark")]
                drop(_hold);
                admission = self
                    .inner
                    .admission_changed
                    .wait(admission)
                    .map_err(|error| StackError::AdmissionPoisoned {
                        message: error.to_string(),
                    })?;
                #[cfg(test)]
                admission.remove_refresh_waiter()?;
                continue;
            }

            let token = admission
                .begin_refresh()
                .ok_or_else(|| StackError::AdmissionPoisoned {
                    message: "VMA refresh state changed while admission was locked".to_owned(),
                })?;
            #[cfg(feature = "allocator-benchmark")]
            drop(_hold);
            drop(admission);
            let snapshot_result = read_snapshot();
            admission = self.lock_admission()?;
            let refresh_result = admission.finish_refresh(snapshot_result, token);
            self.inner.admission_changed.notify_all();
            refresh_result?;
        }
    }

    pub(super) fn admission_with_snapshot_using(
        &self,
        force_refresh: bool,
        mut read_snapshot: impl FnMut() -> Result<VmaSnapshot, StackError>,
    ) -> Result<(MutexGuard<'_, VmaAdmission>, VmaSnapshot), StackError> {
        let mut admission = self.lock_admission()?;
        let generation_at_request = admission.snapshot_generation();
        loop {
            #[cfg(feature = "allocator-benchmark")]
            let _hold = self
                .benchmark_metrics
                .as_ref()
                .map(|metrics| metrics.time(TimingKind::LockHold));
            let refresh_required = if force_refresh {
                admission.snapshot_generation() == generation_at_request
            } else {
                admission.refresh_due()
            };
            if !refresh_required {
                if let Some(snapshot) = admission.snapshot() {
                    return Ok((admission, snapshot));
                }
            }
            if admission.is_refreshing() {
                if !force_refresh {
                    if let Some(snapshot) = admission.snapshot() {
                        return Ok((admission, snapshot));
                    }
                }
                #[cfg(test)]
                admission.add_refresh_waiter()?;
                #[cfg(feature = "allocator-benchmark")]
                drop(_hold);
                admission = self
                    .inner
                    .admission_changed
                    .wait(admission)
                    .map_err(|error| StackError::AdmissionPoisoned {
                        message: error.to_string(),
                    })?;
                #[cfg(test)]
                admission.remove_refresh_waiter()?;
                continue;
            }

            let token = admission
                .begin_refresh()
                .ok_or_else(|| StackError::AdmissionPoisoned {
                    message: "VMA refresh state changed while admission was locked".to_owned(),
                })?;
            #[cfg(feature = "allocator-benchmark")]
            drop(_hold);
            drop(admission);
            let snapshot_result = read_snapshot();
            admission = self.lock_admission()?;
            let refresh_result = admission.finish_refresh(snapshot_result, token);
            self.inner.admission_changed.notify_all();
            refresh_result?;
        }
    }
}
