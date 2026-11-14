use super::metrics::AtomicMetrics;
use std::sync::atomic::Ordering;

/// Timing totals and syscall counts since the previous reset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoInspiredTimingSnapshot {
    /// Time spent waiting to acquire the global class mutex.
    pub lock_wait_ns: u64,
    /// Time holding the global class mutex.
    pub lock_hold_ns: u64,
    /// Time spent reserving spans.
    pub mmap_ns: u64,
    /// Time spent enabling stack slots.
    pub mprotect_ns: u64,
    /// Time spent releasing spans.
    pub munmap_ns: u64,
    /// Span reservation syscall count.
    pub mmap_calls: u64,
    /// Slot protection syscall count.
    pub mprotect_calls: u64,
    /// Span release syscall count.
    pub munmap_calls: u64,
}

impl AtomicMetrics {
    pub fn timing_snapshot_and_reset(&self) -> GoInspiredTimingSnapshot {
        GoInspiredTimingSnapshot {
            lock_wait_ns: self.lock_wait_ns.swap(0, Ordering::AcqRel),
            lock_hold_ns: self.lock_hold_ns.swap(0, Ordering::AcqRel),
            mmap_ns: self.mmap_ns.swap(0, Ordering::AcqRel),
            mprotect_ns: self.mprotect_ns.swap(0, Ordering::AcqRel),
            munmap_ns: self.munmap_ns.swap(0, Ordering::AcqRel),
            mmap_calls: self.mmap_calls.swap(0, Ordering::AcqRel),
            mprotect_calls: self.mprotect_calls.swap(0, Ordering::AcqRel),
            munmap_calls: self.munmap_calls.swap(0, Ordering::AcqRel),
        }
    }
}
