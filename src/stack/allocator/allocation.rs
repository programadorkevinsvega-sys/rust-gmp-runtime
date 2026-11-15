use std::sync::Arc;

#[cfg(feature = "allocator-benchmark")]
use super::super::benchmark_metrics::TimingKind;
use super::super::error::StackError;
use super::super::mapping::FixedStack;
use super::super::protection;
use super::FixedStackManager;

impl FixedStackManager {
    /// Reserves a stack mapping and enables only its usable pages.
    ///
    /// # Errors
    ///
    /// Returns an error if admission, mapping, protection, cleanup, or accounting
    /// fails.
    pub fn allocate(&self) -> Result<FixedStack, StackError> {
        if self.two_phase_syscalls {
            self.allocate_two_phase()
        } else {
            self.allocate_serialized()
        }
    }

    fn allocate_serialized(&self) -> Result<FixedStack, StackError> {
        let mut admission = match self.inner.vma_preflight {
            Some(config) => match self.check_admission_with_reconciliation(1, config.safety_margin)
            {
                Ok((admission, _)) => admission,
                Err(error) => return Err(self.record_failed_allocation(error)),
            },
            None => self.lock_admission()?,
        };
        #[cfg(feature = "allocator-benchmark")]
        let _hold_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        #[cfg(feature = "allocator-benchmark")]
        let _mmap_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Mmap));
        let reserve_result = protection::dispatch::reserve(self.inner.reservation_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(_mmap_timer);
        let base = match reserve_result {
            Ok(base) => base,
            Err(error) => {
                if self.inner.vma_preflight.is_none() {
                    admission.note_operation();
                }
                return Err(self.record_failed_allocation(error));
            }
        };
        if let Err(error) = admission.note_allocation() {
            return Err(self.cleanup_failed_allocation(base, error));
        }
        #[cfg(feature = "allocator-benchmark")]
        let enable_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Mprotect));
        let enable_result =
            protection::dispatch::enable_stack(base, self.inner.page_size, self.inner.usable_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(enable_timer);
        if let Err(error) = enable_result {
            if self.inner.vma_preflight.is_none() {
                admission.note_operation();
            }
            let error = self.cleanup_failed_allocation(base, error);
            return if self.inner.vma_preflight.is_some() {
                match admission.note_release() {
                    Ok(()) => Err(error),
                    Err(accounting) => Err(StackError::CleanupAfterFailure {
                        original: Box::new(error),
                        cleanup: Box::new(accounting),
                    }),
                }
            } else {
                Err(error)
            };
        }
        if let Err(error) = self
            .inner
            .record_allocation(self.inner.reservation_size, self.inner.usable_size)
        {
            let error = match admission.note_release() {
                Ok(()) => error,
                Err(accounting) => StackError::CleanupAfterFailure {
                    original: Box::new(error),
                    cleanup: Box::new(accounting),
                },
            };
            return Err(self.cleanup_failed_allocation(base, error));
        }
        if self.inner.vma_preflight.is_none() {
            admission.note_operation();
        }
        let stack = FixedStack::new(
            base,
            self.inner.reservation_size,
            self.inner.page_size,
            self.inner.usable_size,
            Arc::clone(&self.inner),
            false,
            #[cfg(feature = "allocator-benchmark")]
            self.benchmark_metrics.clone(),
        );
        drop(admission);
        #[cfg(feature = "allocator-benchmark")]
        drop(_hold_timer);
        Ok(stack)
    }
}
