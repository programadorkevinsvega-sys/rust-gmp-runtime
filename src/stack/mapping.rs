use std::ptr::NonNull;
use std::sync::Arc;

use super::allocator::StackManagerInner;

#[path = "mapping/drop.rs"]
mod drop;

/// Bounds of a stack reservation; the guard region precedes usable pages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackBounds {
    /// First inaccessible guard-page address.
    pub guard_start: usize,
    /// Exclusive end of the guard region and start of usable pages.
    pub guard_end: usize,
    /// First address available for stack use.
    pub usable_start: usize,
    /// Exclusive end of usable stack pages.
    pub usable_end: usize,
}

/// RAII owner of a stable-address stack reservation and its guard page.
#[derive(Debug)]
pub struct Stack {
    base: NonNull<u8>,
    reservation_size: usize,
    guard_size: usize,
    usable_size: usize,
    manager: Arc<StackManagerInner>,
    two_phase_syscalls: bool,
    #[cfg(feature = "allocator-benchmark")]
    benchmark_metrics: Option<Arc<super::benchmark_metrics::StackBenchmarkMetrics>>,
}

impl Stack {
    pub(super) fn new(
        base: NonNull<u8>,
        reservation_size: usize,
        guard_size: usize,
        usable_size: usize,
        manager: Arc<StackManagerInner>,
        two_phase_syscalls: bool,
        #[cfg(feature = "allocator-benchmark")] benchmark_metrics: Option<
            Arc<super::benchmark_metrics::StackBenchmarkMetrics>,
        >,
    ) -> Self {
        Self {
            base,
            reservation_size,
            guard_size,
            usable_size,
            manager,
            two_phase_syscalls,
            #[cfg(feature = "allocator-benchmark")]
            benchmark_metrics,
        }
    }

    /// Returns stack and guard bounds as process virtual addresses.
    pub fn bounds(&self) -> StackBounds {
        let guard_start = self.base.as_ptr() as usize;
        let usable_start = guard_start + self.guard_size;
        let usable_end = usable_start + self.usable_size;
        StackBounds {
            guard_start,
            guard_end: usable_start,
            usable_start,
            usable_end,
        }
    }

    /// Returns the page-aligned usable stack capacity in bytes.
    pub fn usable_size(&self) -> usize {
        self.usable_size
    }

    /// Returns total reserved address space, including the guard page.
    pub fn reserved_size(&self) -> usize {
        self.reservation_size
    }
}

// SAFETY: Stack uniquely owns its mapping; moving the owner does not move the
// virtual address. Access to stack contents is reserved for the routine's
// exclusive execution permit, and public methods expose only integer bounds.
unsafe impl Send for Stack {}

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;
