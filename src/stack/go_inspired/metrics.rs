use crate::stack::VmaSnapshot;

/// Quiescent or best-effort counters for the Go-inspired experimental manager.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GoInspiredStackMetrics {
    /// Successfully issued stack leases.
    pub allocations: u64,
    /// Successfully returned leases.
    pub releases: u64,
    /// Leases currently owned by callers.
    pub active_stacks: u64,
    /// Assignments served directly from a Processor-owned local cache.
    pub cache_hits: u64,
    /// Assignments that had to consult the global pool.
    pub cache_misses: u64,
    /// Nonempty batch refills into local caches.
    pub cache_refills: u64,
    /// Calls that attempted to refill an empty local cache.
    pub cache_refill_requests: u64,
    /// Total slots transferred into local caches by refill.
    pub cache_refill_slots: u64,
    /// Largest number of slots transferred by one refill.
    pub cache_refill_max_batch: u64,
    /// Refill operations that overlapped another refill operation.
    pub overlapping_refills: u64,
    /// Maximum number of refill operations active at once.
    pub max_concurrent_refills: u64,
    /// Slots returned from local caches to the global pool.
    pub cache_releases: u64,
    /// Slots allocated after their first activation.
    pub reused_slots: u64,
    /// Spans reserved during this manager's lifetime.
    pub spans_created: u64,
    /// Spans released by `trim`.
    pub spans_released: u64,
    /// Slots quarantined after activation failures.
    pub quarantined_slots: u64,
    /// Failed stack allocation and cache-refill operations.
    pub allocation_failures: u64,
    /// Failed cache flush, span trim, or stack release operations.
    pub cleanup_failures: u64,
    /// Slots currently free in global pools.
    pub global_free_slots: u64,
    /// Slots currently retained in all live local caches.
    pub local_free_slots: u64,
    /// Virtual address space currently reserved by spans.
    pub reserved_virtual_bytes: u64,
    /// Usable pages made accessible; may include free reusable slots.
    pub accessible_virtual_bytes: u64,
    /// Time spent waiting to acquire a per-class pool mutex.
    pub lock_wait_ns: u64,
    /// Time spent holding a per-class pool mutex.
    pub lock_hold_ns: u64,
    /// Accumulated `mmap` time.
    pub mmap_ns: u64,
    /// Accumulated `mprotect` time.
    pub mprotect_ns: u64,
    /// Accumulated `munmap` time.
    pub munmap_ns: u64,
    /// Number of span `mmap` calls.
    pub mmap_calls: u64,
    /// Number of slot `mprotect` calls.
    pub mprotect_calls: u64,
    /// Number of span `munmap` calls.
    pub munmap_calls: u64,
    /// Most recent real process VMA observation.
    pub vma_snapshot: Option<VmaSnapshot>,
}

mod atomic;
pub(super) use atomic::{AtomicMetrics, RefillGuard};
