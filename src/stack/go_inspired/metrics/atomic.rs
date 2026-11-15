use crate::stack::VmaSnapshot;

use super::GoInspiredStackMetrics;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Debug, Default)]
pub(in crate::stack::go_inspired) struct AtomicMetrics {
    pub(in crate::stack::go_inspired) allocations: AtomicU64,
    pub(in crate::stack::go_inspired) releases: AtomicU64,
    pub(in crate::stack::go_inspired) active_stacks: AtomicU64,
    pub(in crate::stack::go_inspired) cache_hits: AtomicU64,
    pub(in crate::stack::go_inspired) cache_misses: AtomicU64,
    pub(in crate::stack::go_inspired) cache_refills: AtomicU64,
    pub(in crate::stack::go_inspired) cache_refill_requests: AtomicU64,
    pub(in crate::stack::go_inspired) cache_refill_slots: AtomicU64,
    pub(in crate::stack::go_inspired) cache_refill_max_batch: AtomicU64,
    pub(in crate::stack::go_inspired) overlapping_refills: AtomicU64,
    pub(in crate::stack::go_inspired) max_concurrent_refills: AtomicU64,
    pub(in crate::stack::go_inspired) refills_in_progress: AtomicU64,
    pub(in crate::stack::go_inspired) cache_releases: AtomicU64,
    pub(in crate::stack::go_inspired) reused_slots: AtomicU64,
    pub(in crate::stack::go_inspired) spans_created: AtomicU64,
    pub(in crate::stack::go_inspired) spans_released: AtomicU64,
    pub(in crate::stack::go_inspired) quarantined_slots: AtomicU64,
    pub(in crate::stack::go_inspired) allocation_failures: AtomicU64,
    pub(in crate::stack::go_inspired) vma_preflight_rejections: AtomicU64,
    pub(in crate::stack::go_inspired) cleanup_failures: AtomicU64,
    pub(in crate::stack::go_inspired) global_free_slots: AtomicU64,
    pub(in crate::stack::go_inspired) local_free_slots: AtomicU64,
    pub(in crate::stack::go_inspired) reserved_virtual_bytes: AtomicU64,
    pub(in crate::stack::go_inspired) accessible_virtual_bytes: AtomicU64,
    pub(in crate::stack::go_inspired) lock_wait_ns: AtomicU64,
    pub(in crate::stack::go_inspired) lock_hold_ns: AtomicU64,
    pub(in crate::stack::go_inspired) mmap_ns: AtomicU64,
    pub(in crate::stack::go_inspired) mprotect_ns: AtomicU64,
    pub(in crate::stack::go_inspired) munmap_ns: AtomicU64,
    pub(in crate::stack::go_inspired) mmap_calls: AtomicU64,
    pub(in crate::stack::go_inspired) mprotect_calls: AtomicU64,
    pub(in crate::stack::go_inspired) munmap_calls: AtomicU64,
}

impl AtomicMetrics {
    pub fn snapshot(&self, vma_snapshot: Option<VmaSnapshot>) -> GoInspiredStackMetrics {
        GoInspiredStackMetrics {
            allocations: load(&self.allocations),
            releases: load(&self.releases),
            active_stacks: load(&self.active_stacks),
            cache_hits: load(&self.cache_hits),
            cache_misses: load(&self.cache_misses),
            cache_refills: load(&self.cache_refills),
            cache_refill_requests: load(&self.cache_refill_requests),
            cache_refill_slots: load(&self.cache_refill_slots),
            cache_refill_max_batch: load(&self.cache_refill_max_batch),
            overlapping_refills: load(&self.overlapping_refills),
            max_concurrent_refills: load(&self.max_concurrent_refills),
            cache_releases: load(&self.cache_releases),
            reused_slots: load(&self.reused_slots),
            spans_created: load(&self.spans_created),
            spans_released: load(&self.spans_released),
            quarantined_slots: load(&self.quarantined_slots),
            allocation_failures: load(&self.allocation_failures),
            vma_preflight_rejections: load(&self.vma_preflight_rejections),
            cleanup_failures: load(&self.cleanup_failures),
            global_free_slots: load(&self.global_free_slots),
            local_free_slots: load(&self.local_free_slots),
            reserved_virtual_bytes: load(&self.reserved_virtual_bytes),
            accessible_virtual_bytes: load(&self.accessible_virtual_bytes),
            lock_wait_ns: load(&self.lock_wait_ns),
            lock_hold_ns: load(&self.lock_hold_ns),
            mmap_ns: load(&self.mmap_ns),
            mprotect_ns: load(&self.mprotect_ns),
            munmap_ns: load(&self.munmap_ns),
            mmap_calls: load(&self.mmap_calls),
            mprotect_calls: load(&self.mprotect_calls),
            munmap_calls: load(&self.munmap_calls),
            vma_snapshot,
        }
    }

    pub fn record_elapsed(counter: &AtomicU64, started: Instant) {
        let duration = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let mut current = counter.load(Ordering::Acquire);
        loop {
            let next = current.saturating_add(duration);
            let updated = counter
                .compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok();
            if updated {
                break;
            }
            current = counter.load(Ordering::Acquire);
        }
    }

    pub fn record_call(counter: &AtomicU64) {
        let mut current = counter.load(Ordering::Acquire);
        loop {
            let updated = counter
                .compare_exchange_weak(
                    current,
                    current.saturating_add(1),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok();
            if updated {
                break;
            }
            current = counter.load(Ordering::Acquire);
        }
    }

    pub fn add(counter: &AtomicU64, amount: u64) -> Result<(), crate::stack::StackError> {
        counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(amount)
            })
            .map(|_| ())
            .map_err(|current| crate::stack::StackError::MetricsInvariant {
                operation: "increment Go-inspired metrics counter",
                observed: current.to_string(),
            })
    }

    pub fn subtract(counter: &AtomicU64, amount: u64) -> Result<(), crate::stack::StackError> {
        counter
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_sub(amount)
            })
            .map(|_| ())
            .map_err(|current| crate::stack::StackError::MetricsInvariant {
                operation: "decrement Go-inspired metrics counter",
                observed: current.to_string(),
            })
    }

    pub fn record_max(counter: &AtomicU64, value: u64) {
        let mut current = counter.load(Ordering::Acquire);
        while value > current {
            match counter.compare_exchange_weak(current, value, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    pub fn begin_refill(&self) -> Result<RefillGuard<'_>, crate::stack::StackError> {
        let current = self.refills_in_progress.fetch_add(1, Ordering::AcqRel);
        let Some(concurrent) = current.checked_add(1) else {
            self.refills_in_progress.fetch_sub(1, Ordering::AcqRel);
            return Err(crate::stack::StackError::MetricsInvariant {
                operation: "increment concurrent stack refills",
                observed: current.to_string(),
            });
        };
        if current > 0 {
            if let Err(error) = Self::add(&self.overlapping_refills, 1) {
                self.refills_in_progress.fetch_sub(1, Ordering::AcqRel);
                return Err(error);
            }
        }
        Self::record_max(&self.max_concurrent_refills, concurrent);
        Ok(RefillGuard { metrics: self })
    }
}

pub(in crate::stack::go_inspired) struct RefillGuard<'metrics> {
    metrics: &'metrics AtomicMetrics,
}

impl Drop for RefillGuard<'_> {
    fn drop(&mut self) {
        let previous = self
            .metrics
            .refills_in_progress
            .fetch_sub(1, Ordering::AcqRel);
        if previous == 0 {
            eprintln!("fatal: concurrent stack refill counter underflow");
            std::process::abort();
        }
    }
}

fn load(counter: &AtomicU64) -> u64 {
    counter.load(Ordering::Acquire)
}
