use std::sync::Arc;

#[cfg(feature = "allocator-benchmark")]
use super::super::benchmark_metrics::TimingKind;
use super::super::error::StackError;
use super::super::mapping::Stack;
use super::super::protection;
use super::StackManager;
use super::combine_failures;

#[path = "two_phase/cleanup.rs"]
mod cleanup;

impl StackManager {
    pub(super) fn allocate_two_phase(&self) -> Result<Stack, StackError> {
        let mut admission = match self.inner.vma_preflight {
            Some(config) => match self.check_admission_with_reconciliation(1, config.safety_margin)
            {
                Ok((admission, _)) => admission,
                Err(error) => return Err(self.record_failed_allocation(error)),
            },
            None => self.lock_admission()?,
        };
        #[cfg(feature = "allocator-benchmark")]
        let hold_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        if let Err(error) = admission.begin_allocation() {
            drop(admission);
            return Err(self.record_failed_allocation(error));
        }

        drop(admission);
        #[cfg(feature = "allocator-benchmark")]
        drop(hold_timer);

        #[cfg(feature = "allocator-benchmark")]
        let mmap_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Mmap));
        let reservation = protection::dispatch::reserve(self.inner.reservation_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(mmap_timer);
        let base = match reservation {
            Ok(base) => base,
            Err(error) => {
                let error = match self.cancel_pending_allocation() {
                    Ok(()) => error,
                    Err(cleanup) => combine_failures(error, cleanup),
                };
                return Err(self.record_failed_allocation(error));
            }
        };

        #[cfg(feature = "allocator-benchmark")]
        let mprotect_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Mprotect));
        let protection =
            protection::dispatch::enable_stack(base, self.inner.page_size, self.inner.usable_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(mprotect_timer);
        if let Err(error) = protection {
            let error = self.cleanup_partial_mapping(base, error);
            return Err(self.record_failed_allocation(error));
        }

        let mut admission = match self.lock_admission() {
            Ok(admission) => admission,
            Err(error) => {
                let error = self.cleanup_after_accounting_failure(base, error);
                return Err(self.record_failed_allocation(error));
            }
        };
        #[cfg(feature = "allocator-benchmark")]
        let hold_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        if let Err(error) = admission.finish_allocation() {
            drop(admission);
            #[cfg(feature = "allocator-benchmark")]
            drop(hold_timer);
            let error = self.cleanup_after_accounting_failure(base, error);
            return Err(self.record_failed_allocation(error));
        }
        drop(admission);
        #[cfg(feature = "allocator-benchmark")]
        drop(hold_timer);

        if let Err(error) = self
            .inner
            .record_allocation(self.inner.reservation_size, self.inner.usable_size)
        {
            let error = self.cleanup_committed_mapping(base, error);
            return Err(self.record_failed_allocation(error));
        }

        Ok(Stack::new(
            base,
            self.inner.reservation_size,
            self.inner.page_size,
            self.inner.usable_size,
            Arc::clone(&self.inner),
            true,
            #[cfg(feature = "allocator-benchmark")]
            self.benchmark_metrics.clone(),
        ))
    }
}

#[cfg(all(test, feature = "allocator-benchmark"))]
#[path = "two_phase_tests.rs"]
mod tests;
