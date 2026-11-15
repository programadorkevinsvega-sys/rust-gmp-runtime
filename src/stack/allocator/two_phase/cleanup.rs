#[cfg(feature = "allocator-benchmark")]
use super::super::super::benchmark_metrics::TimingKind;
use super::super::super::error::StackError;
use super::super::super::protection;
use super::super::FixedStackManager;
use super::super::combine_failures;

impl FixedStackManager {
    pub(super) fn cancel_pending_allocation(&self) -> Result<(), StackError> {
        let mut admission = self.lock_admission()?;
        #[cfg(feature = "allocator-benchmark")]
        let hold_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        let result = admission.cancel_allocation();
        #[cfg(feature = "allocator-benchmark")]
        drop(hold_timer);
        result
    }

    pub(super) fn cleanup_partial_mapping(
        &self,
        base: std::ptr::NonNull<u8>,
        original: StackError,
    ) -> StackError {
        #[cfg(feature = "allocator-benchmark")]
        let munmap_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Munmap));
        let release = protection::dispatch::release(base, self.inner.reservation_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(munmap_timer);
        match release {
            Ok(()) => match self.cancel_pending_allocation() {
                Ok(()) => original,
                Err(cleanup) => combine_failures(original, cleanup),
            },
            Err(cleanup) => self.combine_cleanup_failure(original, cleanup),
        }
    }

    pub(super) fn cleanup_after_accounting_failure(
        &self,
        base: std::ptr::NonNull<u8>,
        original: StackError,
    ) -> StackError {
        #[cfg(feature = "allocator-benchmark")]
        let munmap_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Munmap));
        let cleanup = protection::dispatch::release(base, self.inner.reservation_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(munmap_timer);
        match cleanup {
            Ok(()) => match self.cancel_pending_allocation() {
                Ok(()) => original,
                Err(cleanup) => combine_failures(original, cleanup),
            },
            Err(cleanup) => self.combine_cleanup_failure(original, cleanup),
        }
    }

    pub(super) fn cleanup_committed_mapping(
        &self,
        base: std::ptr::NonNull<u8>,
        original: StackError,
    ) -> StackError {
        #[cfg(feature = "allocator-benchmark")]
        let munmap_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Munmap));
        let cleanup = protection::dispatch::release(base, self.inner.reservation_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(munmap_timer);
        match cleanup {
            Ok(()) => {
                let accounting = self
                    .lock_admission()
                    .and_then(|mut admission| admission.note_release());
                match accounting {
                    Ok(()) => original,
                    Err(cleanup) => combine_failures(original, cleanup),
                }
            }
            Err(cleanup) => self.combine_cleanup_failure(original, cleanup),
        }
    }

    fn combine_cleanup_failure(&self, original: StackError, cleanup: StackError) -> StackError {
        let cleanup = match self.inner.record_cleanup_failure() {
            Ok(()) => cleanup,
            Err(metrics) => combine_failures(cleanup, metrics),
        };
        combine_failures(original, cleanup)
    }
}
