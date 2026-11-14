use rust_gmp_runtime::stack::{BlockStackMetrics, GoInspiredStackMetrics, StackMetrics};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Stats {
    pub active: u64,
    pub arenas: u64,
    pub free_slots: u64,
    pub occupied_slots: u64,
    pub reused_slots: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_refills: u64,
    pub cache_refill_requests: u64,
    pub cache_refill_slots: u64,
    pub cache_refill_max_batch: u64,
    pub overlapping_refills: u64,
    pub max_concurrent_refills: u64,
    pub local_free_slots: u64,
    pub global_free_slots: u64,
    pub pool_lock_wait_ns: u64,
    pub pool_lock_hold_ns: u64,
    pub allocation_failures: Option<u64>,
    pub cleanup_failures: Option<u64>,
    pub reserved_bytes: u64,
    pub accessible_bytes: u64,
    pub process_virtual_bytes: Option<u64>,
    pub rss_bytes: Option<u64>,
    pub vmas: u64,
}

impl Display for Stats {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "active={} arenas={} free_slots={} occupied_slots={} reused_slots={} cache_hits={} cache_misses={} cache_refills={} cache_refill_requests={} cache_refill_slots={} cache_refill_max_batch={} overlapping_refills={} max_concurrent_refills={} local_free_slots={} global_free_slots={} pool_lock_wait_ns={} pool_lock_hold_ns={} allocation_failures={:?} cleanup_failures={:?} reserved_bytes={} accessible_bytes={} process_virtual_bytes={:?} rss_bytes={:?} vmas={}",
            self.active,
            self.arenas,
            self.free_slots,
            self.occupied_slots,
            self.reused_slots,
            self.cache_hits,
            self.cache_misses,
            self.cache_refills,
            self.cache_refill_requests,
            self.cache_refill_slots,
            self.cache_refill_max_batch,
            self.overlapping_refills,
            self.max_concurrent_refills,
            self.local_free_slots,
            self.global_free_slots,
            self.pool_lock_wait_ns,
            self.pool_lock_hold_ns,
            self.allocation_failures,
            self.cleanup_failures,
            self.reserved_bytes,
            self.accessible_bytes,
            self.process_virtual_bytes,
            self.rss_bytes,
            self.vmas,
        )
    }
}

pub fn fixed(metrics: StackMetrics, page_size: usize) -> Result<Stats, Box<dyn Error>> {
    let (virtual_bytes, rss_bytes) = process_memory(page_size)?;
    Ok(Stats {
        active: metrics.active_stacks,
        arenas: 0,
        free_slots: 0,
        occupied_slots: metrics.active_stacks,
        reused_slots: 0,
        cache_hits: 0,
        cache_misses: 0,
        cache_refills: 0,
        cache_refill_requests: 0,
        cache_refill_slots: 0,
        cache_refill_max_batch: 0,
        overlapping_refills: 0,
        max_concurrent_refills: 0,
        local_free_slots: 0,
        global_free_slots: 0,
        pool_lock_wait_ns: 0,
        pool_lock_hold_ns: 0,
        allocation_failures: Some(metrics.failed_allocations),
        cleanup_failures: Some(metrics.cleanup_failures),
        reserved_bytes: metrics.reserved_virtual_bytes,
        accessible_bytes: metrics.accessible_virtual_bytes,
        process_virtual_bytes: virtual_bytes.or(metrics.process_virtual_bytes),
        rss_bytes: rss_bytes.or(metrics.process_resident_bytes),
        vmas: metrics.vma_current,
    })
}

pub fn block(metrics: BlockStackMetrics, page_size: usize) -> Result<Stats, Box<dyn Error>> {
    let snapshot = metrics
        .vma_snapshot
        .ok_or("block allocator did not report a VMA snapshot")?;
    let (virtual_bytes, rss_bytes) = process_memory(page_size)?;
    Ok(Stats {
        active: metrics.active_stacks,
        arenas: metrics.arenas,
        free_slots: metrics.free_slots,
        occupied_slots: metrics.occupied_slots,
        reused_slots: metrics.reused_slots,
        cache_hits: 0,
        cache_misses: 0,
        cache_refills: 0,
        cache_refill_requests: 0,
        cache_refill_slots: 0,
        cache_refill_max_batch: 0,
        overlapping_refills: 0,
        max_concurrent_refills: 0,
        local_free_slots: 0,
        global_free_slots: 0,
        pool_lock_wait_ns: 0,
        pool_lock_hold_ns: 0,
        allocation_failures: Some(metrics.allocation_failures),
        cleanup_failures: Some(metrics.cleanup_failures),
        reserved_bytes: metrics.reserved_virtual_bytes,
        accessible_bytes: metrics.accessible_virtual_bytes,
        process_virtual_bytes: virtual_bytes,
        rss_bytes,
        vmas: snapshot.current,
    })
}

pub fn go_inspired(
    metrics: GoInspiredStackMetrics,
    page_size: usize,
) -> Result<Stats, Box<dyn Error>> {
    let snapshot = metrics
        .vma_snapshot
        .ok_or("Go-inspired allocator did not report a VMA snapshot")?;
    let (virtual_bytes, rss_bytes) = process_memory(page_size)?;
    let free_slots = metrics
        .global_free_slots
        .checked_add(metrics.local_free_slots)
        .ok_or("Go-inspired free-slot count overflow")?;
    let occupied_slots = metrics
        .active_stacks
        .checked_add(metrics.quarantined_slots)
        .ok_or("Go-inspired occupied-slot count overflow")?;
    let arenas = metrics
        .spans_created
        .checked_sub(metrics.spans_released)
        .ok_or("Go-inspired span count underflow")?;
    Ok(Stats {
        active: metrics.active_stacks,
        arenas,
        free_slots,
        occupied_slots,
        reused_slots: metrics.reused_slots,
        cache_hits: metrics.cache_hits,
        cache_misses: metrics.cache_misses,
        cache_refills: metrics.cache_refills,
        cache_refill_requests: metrics.cache_refill_requests,
        cache_refill_slots: metrics.cache_refill_slots,
        cache_refill_max_batch: metrics.cache_refill_max_batch,
        overlapping_refills: metrics.overlapping_refills,
        max_concurrent_refills: metrics.max_concurrent_refills,
        local_free_slots: metrics.local_free_slots,
        global_free_slots: metrics.global_free_slots,
        pool_lock_wait_ns: metrics.lock_wait_ns,
        pool_lock_hold_ns: metrics.lock_hold_ns,
        allocation_failures: Some(metrics.allocation_failures),
        cleanup_failures: Some(metrics.cleanup_failures),
        reserved_bytes: metrics.reserved_virtual_bytes,
        accessible_bytes: metrics.accessible_virtual_bytes,
        process_virtual_bytes: virtual_bytes,
        rss_bytes,
        vmas: snapshot.current,
    })
}

#[path = "stats/process.rs"]
mod process;
#[path = "stats/vma.rs"]
mod vma;
pub(super) use process::process_memory;
pub use vma::current_vma_count;
