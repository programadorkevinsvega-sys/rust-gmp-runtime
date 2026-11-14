use super::admission::MAX_VMA_COST_PER_STACK;
use super::allocator::StackManagerInner;
use super::error::StackError;
use std::sync::MutexGuard;

/// Cumulative allocation counters and current process memory observations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackMetrics {
    /// Successful stack reservations since manager creation.
    pub successful_allocations: u64,
    /// Successfully released stacks since manager creation.
    pub released_stacks: u64,
    /// `allocate` attempts that failed, including preventive rejections.
    pub failed_allocations: u64,
    /// Cleanup syscalls that failed after an allocation error.
    pub cleanup_failures: u64,
    /// Compatibility alias for `failed_allocations`.
    pub allocation_failures: u64,
    /// Allocation or advisory preflight requests rejected by the VMA budget.
    pub vma_preflight_rejections: u64,
    /// Currently live stack objects.
    pub active_stacks: u64,
    /// Currently live mmap reservations owned by the manager.
    pub active_mappings: u64,
    /// Active mappings known to this manager; not an exact Linux VMA count.
    pub runtime_mapping_count: u64,
    /// Upper-bound VMA estimate for active runtime mappings, not an observed count.
    pub runtime_vma_estimate: u64,
    /// Free stack slots; always zero because this allocator has no pool.
    pub free_stacks: u64,
    /// Virtual address space reserved by live stack mappings.
    pub reserved_virtual_bytes: u64,
    /// Stack pages enabled for read/write access; this is not resident RAM.
    pub accessible_virtual_bytes: u64,
    /// Total process virtual memory observed via procfs when available.
    pub process_virtual_bytes: Option<u64>,
    /// Approximate process resident memory observed via procfs when available.
    pub process_resident_bytes: Option<u64>,
    /// Process VMA count observed via procfs when available.
    pub process_vma_count: Option<u64>,
    /// Current kernel VMA ceiling from `/proc/sys/vm/max_map_count`.
    pub vma_limit: u64,
    /// Current process VMA count from `/proc/self/maps`.
    pub vma_current: u64,
    /// Theoretical remaining VMA capacity, clamped to zero.
    pub vma_remaining: u64,
    /// Current process VMA count as a percentage of the limit.
    pub vma_utilization_percent: f64,
}

impl StackManagerInner {
    pub(super) fn record_allocation(
        &self,
        reservation_size: usize,
        usable_size: usize,
    ) -> Result<(), StackError> {
        let reservation_size = usize_to_u64(reservation_size, "stack reservation size")?;
        let usable_size = usize_to_u64(usable_size, "usable stack size")?;
        let mut metrics = self.lock_metrics()?;

        let successful_allocations = metrics
            .successful_allocations
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        let active_stacks = metrics
            .active_stacks
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        let active_mappings = metrics
            .active_mappings
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        let runtime_vma_estimate = active_mappings
            .checked_mul(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        let reserved_virtual_bytes = metrics
            .reserved_virtual_bytes
            .checked_add(reservation_size)
            .ok_or(StackError::MetricsOverflow)?;
        let accessible_virtual_bytes = metrics
            .accessible_virtual_bytes
            .checked_add(usable_size)
            .ok_or(StackError::MetricsOverflow)?;

        metrics.successful_allocations = successful_allocations;
        metrics.active_stacks = active_stacks;
        metrics.active_mappings = active_mappings;
        metrics.runtime_mapping_count = active_mappings;
        metrics.runtime_vma_estimate = runtime_vma_estimate;
        metrics.reserved_virtual_bytes = reserved_virtual_bytes;
        metrics.accessible_virtual_bytes = accessible_virtual_bytes;
        Ok(())
    }

    pub(super) fn record_release(
        &self,
        reservation_size: usize,
        usable_size: usize,
    ) -> Result<(), StackError> {
        let reservation_size = usize_to_u64(reservation_size, "stack reservation size")?;
        let usable_size = usize_to_u64(usable_size, "usable stack size")?;
        let mut metrics = self.lock_metrics()?;

        let released_stacks = metrics
            .released_stacks
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        let active_stacks = metrics
            .active_stacks
            .checked_sub(1)
            .ok_or(StackError::MetricsOverflow)?;
        let active_mappings = metrics
            .active_mappings
            .checked_sub(1)
            .ok_or(StackError::MetricsOverflow)?;
        let runtime_vma_estimate = active_mappings
            .checked_mul(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        let reserved_virtual_bytes = metrics
            .reserved_virtual_bytes
            .checked_sub(reservation_size)
            .ok_or(StackError::MetricsOverflow)?;
        let accessible_virtual_bytes = metrics
            .accessible_virtual_bytes
            .checked_sub(usable_size)
            .ok_or(StackError::MetricsOverflow)?;

        metrics.released_stacks = released_stacks;
        metrics.active_stacks = active_stacks;
        metrics.active_mappings = active_mappings;
        metrics.runtime_mapping_count = active_mappings;
        metrics.runtime_vma_estimate = runtime_vma_estimate;
        metrics.reserved_virtual_bytes = reserved_virtual_bytes;
        metrics.accessible_virtual_bytes = accessible_virtual_bytes;
        Ok(())
    }

    pub(super) fn record_failed_allocation(&self) -> Result<(), StackError> {
        let mut metrics = self.lock_metrics()?;
        metrics.failed_allocations = metrics
            .failed_allocations
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        metrics.allocation_failures = metrics.failed_allocations;
        Ok(())
    }

    pub(super) fn record_vma_preflight_rejection(&self) -> Result<(), StackError> {
        let mut metrics = self.lock_metrics()?;
        metrics.vma_preflight_rejections = metrics
            .vma_preflight_rejections
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        Ok(())
    }

    pub(super) fn record_cleanup_failure(&self) -> Result<(), StackError> {
        let mut metrics = self.lock_metrics()?;
        metrics.cleanup_failures = metrics
            .cleanup_failures
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        Ok(())
    }

    fn lock_metrics(&self) -> Result<MutexGuard<'_, StackMetrics>, StackError> {
        match self.metrics.lock() {
            Ok(metrics) => Ok(metrics),
            Err(error) => Err(StackError::MetricsPoisoned {
                message: error.to_string(),
            }),
        }
    }
}

fn usize_to_u64(value: usize, operation: &'static str) -> Result<u64, StackError> {
    match u64::try_from(value) {
        Ok(value) => Ok(value),
        Err(source) => Err(StackError::IntegerConversion { operation, source }),
    }
}
