use super::*;

impl GoInspiredStackManager {
    /// Reads a fresh VMA snapshot and returns allocator counters.
    ///
    /// # Errors
    ///
    /// Returns an error if procfs cannot be read or accounting is inconsistent.
    pub fn metrics(&self) -> Result<GoInspiredStackMetrics, StackError> {
        let snapshot = self.inner.snapshot()?;
        Ok(self.inner.metrics.snapshot(Some(snapshot)))
    }

    /// Returns and resets syscall and pool-mutex timing totals.
    ///
    /// Timing fields remain zero unless this manager was created with
    /// [`Self::new_benchmarked`].
    #[cfg(feature = "allocator-benchmark")]
    pub fn take_timing_snapshot(&self) -> GoInspiredTimingSnapshot {
        self.inner.metrics.timing_snapshot_and_reset()
    }

    /// Resets the maximum concurrent refill count at a quiescent benchmark boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if any refill is currently in progress. The caller
    /// must prevent new refill operations until this method returns.
    #[cfg(feature = "allocator-benchmark")]
    pub fn reset_refill_peak(&self) -> Result<(), StackError> {
        if self
            .inner
            .metrics
            .refills_in_progress
            .load(std::sync::atomic::Ordering::Acquire)
            != 0
        {
            return Err(StackError::MetricsInvariant {
                operation: "reset refill peak while refills are active",
                observed: "refill in progress".to_owned(),
            });
        }
        self.inner
            .metrics
            .max_concurrent_refills
            .store(0, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    /// Reads a fresh process VMA observation outside the pool lock.
    ///
    /// # Errors
    ///
    /// Returns an error if procfs cannot be read or parsed.
    pub fn vma_snapshot(&self) -> Result<VmaSnapshot, StackError> {
        self.inner.snapshot()
    }
}
