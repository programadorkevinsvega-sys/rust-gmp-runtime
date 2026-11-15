use std::sync::{Arc, Condvar, Mutex};

use super::admission::VmaAdmission;
#[cfg(feature = "allocator-benchmark")]
use super::benchmark_metrics::StackBenchmarkMetrics;
use super::config::{StackConfig, VmaPreflightConfig};
use super::error::StackError;
use super::metrics::StackMetrics;
use super::protection;
use super::size::round_up_to_page;

#[path = "allocator/allocation.rs"]
mod allocation;
#[cfg(feature = "allocator-benchmark")]
#[path = "allocator_benchmarked.rs"]
mod benchmarked;
#[path = "allocator/two_phase.rs"]
mod two_phase;

/// Allocates individually mapped, stable-address stacks with a guard page.
#[derive(Clone, Debug)]
pub(super) struct FixedStackManager {
    pub(super) inner: Arc<StackManagerInner>,
    pub(super) two_phase_syscalls: bool,
    #[cfg(feature = "allocator-benchmark")]
    pub(super) benchmark_metrics: Option<Arc<StackBenchmarkMetrics>>,
}

#[derive(Debug)]
pub(super) struct StackManagerInner {
    pub(super) page_size: usize,
    pub(super) requested_usable_size: usize,
    pub(super) usable_size: usize,
    pub(super) reservation_size: usize,
    pub(super) vma_preflight: Option<VmaPreflightConfig>,
    pub(super) metrics: Mutex<StackMetrics>,
    pub(super) admission: Mutex<VmaAdmission>,
    pub(super) admission_changed: Condvar,
}

impl FixedStackManager {
    /// Creates a manager for Linux x86_64 and obtains the OS page size.
    ///
    /// # Errors
    ///
    /// Returns an error if the target is unsupported, the page size cannot be
    /// queried, or the requested mapping size overflows.
    pub fn new(config: StackConfig) -> Result<Self, StackError> {
        Self::new_inner(config, false)
    }

    fn new_inner(config: StackConfig, two_phase_syscalls: bool) -> Result<Self, StackError> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(StackError::UnsupportedPlatform);
        }

        let page_size = protection::dispatch::page_size()?;
        let requested_usable_size = config.requested_usable_size();
        let usable_size = round_up_to_page(requested_usable_size, page_size)?;
        let reservation_size = usable_size
            .checked_add(page_size)
            .ok_or(StackError::SizeOverflow)?;

        Ok(Self {
            inner: Arc::new(StackManagerInner {
                page_size,
                requested_usable_size,
                usable_size,
                reservation_size,
                vma_preflight: config.vma_preflight(),
                metrics: Mutex::new(StackMetrics::default()),
                admission: Mutex::new(VmaAdmission::default()),
                admission_changed: Condvar::new(),
            }),
            two_phase_syscalls,
            #[cfg(feature = "allocator-benchmark")]
            benchmark_metrics: None,
        })
    }

    /// Returns the operating system's page size in bytes.
    pub fn page_size(&self) -> usize {
        self.inner.page_size
    }

    /// Returns the configured usable size before page alignment.
    pub fn requested_usable_size(&self) -> usize {
        self.inner.requested_usable_size
    }

    /// Returns the page-aligned usable size of each stack.
    pub fn usable_size(&self) -> usize {
        self.inner.usable_size
    }

    fn record_failed_allocation(&self, original: StackError) -> StackError {
        match self.inner.record_failed_allocation() {
            Ok(()) => original,
            Err(accounting) => combine_failures(original, accounting),
        }
    }

    fn cleanup_failed_allocation(
        &self,
        base: std::ptr::NonNull<u8>,
        original: StackError,
    ) -> StackError {
        let original = match protection::dispatch::release(base, self.inner.reservation_size) {
            Ok(()) => original,
            Err(cleanup) => {
                let cleanup = match self.inner.record_cleanup_failure() {
                    Ok(()) => cleanup,
                    Err(metrics) => combine_failures(cleanup, metrics),
                };
                combine_failures(original, cleanup)
            }
        };
        self.record_failed_allocation(original)
    }
}

fn combine_failures(original: StackError, cleanup: StackError) -> StackError {
    StackError::CleanupAfterFailure {
        original: Box::new(original),
        cleanup: Box::new(cleanup),
    }
}

#[cfg(test)]
#[path = "allocator_tests.rs"]
mod tests;
