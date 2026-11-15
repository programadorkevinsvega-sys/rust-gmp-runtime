use std::sync::MutexGuard;

use super::admission::MAX_VMA_COST_PER_STACK;
use super::admission::VmaAdmission;
use super::allocator::FixedStackManager;
use super::error::StackError;
use super::metrics::StackMetrics;
use super::process_metrics::read_process_metrics;
use super::vma::VmaSnapshot;

impl FixedStackManager {
    /// Performs an advisory admission check for the requested number of stacks.
    ///
    /// The result is a snapshot, not a reservation; other process mappings may
    /// change before subsequent allocations.
    ///
    /// # Errors
    ///
    /// Returns an error if procfs is unavailable, preflight is disabled, or
    /// the conservative estimate exceeds the configured VMA budget.
    pub fn preflight_additional_stacks(&self, count: usize) -> Result<VmaSnapshot, StackError> {
        let config = self
            .inner
            .vma_preflight
            .ok_or(StackError::VmaPreflightDisabled)?;
        let (_admission, snapshot) =
            self.check_admission_with_reconciliation(count, config.safety_margin)?;
        drop(_admission);
        Ok(snapshot)
    }

    /// Reads the current Linux VMA limit and process mapping count.
    ///
    /// # Errors
    ///
    /// Returns an error if procfs is unavailable, malformed, or admission
    /// accounting is poisoned.
    pub fn vma_snapshot(&self) -> Result<VmaSnapshot, StackError> {
        let (_admission, snapshot) = self.admission_with_snapshot(true)?;
        Ok(snapshot)
    }

    /// Returns allocator counters and Linux process memory/VMA observations.
    ///
    /// # Errors
    ///
    /// Returns an error if accounting is poisoned or required procfs data cannot
    /// be read or parsed.
    pub fn metrics(&self) -> Result<StackMetrics, StackError> {
        let (_admission, snapshot) = self.admission_with_snapshot(true)?;
        let mut metrics = match self.inner.metrics.lock() {
            Ok(metrics) => metrics.clone(),
            Err(error) => {
                return Err(StackError::MetricsPoisoned {
                    message: error.to_string(),
                });
            }
        };
        drop(_admission);
        let process_metrics = read_process_metrics(self.inner.page_size)?;
        metrics.vma_limit = snapshot.limit;
        metrics.vma_current = snapshot.current;
        metrics.vma_remaining = snapshot.remaining;
        metrics.vma_utilization_percent = snapshot.utilization_percent;
        metrics.runtime_mapping_count = metrics.active_mappings;
        metrics.runtime_vma_estimate = metrics
            .active_mappings
            .checked_mul(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        metrics.free_stacks = 0;
        metrics.process_virtual_bytes = process_metrics.virtual_bytes;
        metrics.process_resident_bytes = process_metrics.resident_bytes;
        metrics.process_vma_count = Some(snapshot.current);
        Ok(metrics)
    }

    pub(super) fn lock_admission(&self) -> Result<MutexGuard<'_, VmaAdmission>, StackError> {
        #[cfg(feature = "allocator-benchmark")]
        let wait_started = self
            .benchmark_metrics
            .as_ref()
            .map(|_| std::time::Instant::now());
        match self.inner.admission.lock() {
            Ok(admission) => {
                #[cfg(feature = "allocator-benchmark")]
                if let (Some(metrics), Some(started)) = (&self.benchmark_metrics, wait_started) {
                    metrics.record_lock_wait(started);
                }
                Ok(admission)
            }
            Err(error) => Err(StackError::AdmissionPoisoned {
                message: error.to_string(),
            }),
        }
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
