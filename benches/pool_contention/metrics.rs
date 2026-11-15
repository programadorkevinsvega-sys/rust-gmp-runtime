use std::error::Error;

use rust_gmp_runtime::stack::GoInspiredStackMetrics;

use super::{PoolPath, SLOTS_PER_SPAN};

#[derive(Debug)]
pub(super) struct MetricDelta {
    pub(super) allocations: u64,
    pub(super) releases: u64,
    pub(super) active_stacks: u64,
    pub(super) cache_hits: u64,
    pub(super) cache_misses: u64,
    pub(super) cache_refill_requests: u64,
    pub(super) cache_refills: u64,
    pub(super) cache_refill_slots: u64,
    pub(super) allocation_failures: u64,
    pub(super) cleanup_failures: u64,
}

pub(super) fn metric_delta(
    before: &GoInspiredStackMetrics,
    after: &GoInspiredStackMetrics,
) -> Result<MetricDelta, Box<dyn Error>> {
    Ok(MetricDelta {
        allocations: difference(after.allocations, before.allocations, "allocations")?,
        releases: difference(after.releases, before.releases, "releases")?,
        active_stacks: after.active_stacks,
        cache_hits: difference(after.cache_hits, before.cache_hits, "cache hits")?,
        cache_misses: difference(after.cache_misses, before.cache_misses, "cache misses")?,
        cache_refill_requests: difference(
            after.cache_refill_requests,
            before.cache_refill_requests,
            "cache refill requests",
        )?,
        cache_refills: difference(after.cache_refills, before.cache_refills, "cache refills")?,
        cache_refill_slots: difference(
            after.cache_refill_slots,
            before.cache_refill_slots,
            "cache refill slots",
        )?,
        allocation_failures: difference(
            after.allocation_failures,
            before.allocation_failures,
            "allocation failures",
        )?,
        cleanup_failures: difference(
            after.cleanup_failures,
            before.cleanup_failures,
            "cleanup failures",
        )?,
    })
}

fn difference(after: u64, before: u64, name: &'static str) -> Result<u64, Box<dyn Error>> {
    after
        .checked_sub(before)
        .ok_or_else(|| format!("{name} counter decreased").into())
}

pub(super) fn validate_operation_counts(
    path: PoolPath,
    expected: u64,
    delta: &MetricDelta,
) -> Result<(), Box<dyn Error>> {
    if delta.allocations != expected
        || delta.releases != expected
        || delta.active_stacks != 0
        || delta.allocation_failures != 0
        || delta.cleanup_failures != 0
    {
        return Err(format!(
            "{path:?} operation accounting mismatch: expected={expected} delta={delta:?}"
        )
        .into());
    }
    let cache_path_has_no_refills = path == PoolPath::WorkerCache
        && (delta.cache_misses != 0
            || delta.cache_refill_requests != 0
            || delta.cache_refills != 0
            || delta.cache_refill_slots != 0);
    let global_path_has_no_cache_activity = path == PoolPath::Global
        && (delta.cache_hits != 0
            || delta.cache_misses != 0
            || delta.cache_refill_requests != 0
            || delta.cache_refills != 0
            || delta.cache_refill_slots != 0);
    if cache_path_has_no_refills || global_path_has_no_cache_activity {
        return Err(format!(
            "{path:?} cache accounting mismatch: expected hot cache hits or no cache use; delta={delta:?}"
        )
        .into());
    }
    if path == PoolPath::WorkerCache && delta.cache_hits != expected {
        return Err(format!(
            "worker-cache hit count mismatch: expected={expected} actual={}",
            delta.cache_hits
        )
        .into());
    }
    Ok(())
}

pub(super) fn validate_memory_geometry(
    metrics: &GoInspiredStackMetrics,
    usable_bytes: usize,
    page_size: usize,
) -> Result<(), Box<dyn Error>> {
    let span_count = metrics
        .spans_created
        .checked_sub(metrics.spans_released)
        .ok_or("released spans exceed created spans")?;
    let expected_span_bytes = usable_bytes
        .checked_add(page_size)
        .and_then(|stride| stride.checked_mul(SLOTS_PER_SPAN))
        .ok_or("span size overflow")?;
    let expected_reserved = span_count
        .checked_mul(u64::try_from(expected_span_bytes)?)
        .ok_or("reserved byte count overflow")?;
    let max_accessible = span_count
        .checked_mul(u64::try_from(usable_bytes)?)
        .ok_or("accessible byte count overflow")?
        .checked_mul(u64::try_from(SLOTS_PER_SPAN)?)
        .ok_or("accessible byte count overflow")?;
    let usable_bytes = u64::try_from(usable_bytes)?;
    if usable_bytes == 0
        || metrics.reserved_virtual_bytes != expected_reserved
        || metrics.accessible_virtual_bytes > max_accessible
        || metrics.accessible_virtual_bytes % usable_bytes != 0
    {
        return Err(format!("pool memory geometry mismatch: {metrics:?}").into());
    }
    Ok(())
}

pub(super) fn validate_quiescent(metrics: &GoInspiredStackMetrics) -> Result<(), Box<dyn Error>> {
    if metrics.active_stacks != 0
        || metrics.local_free_slots != 0
        || metrics.allocation_failures != 0
        || metrics.cleanup_failures != 0
    {
        return Err(format!("pool did not reach a clean quiescent state: {metrics:?}").into());
    }
    Ok(())
}

pub(super) fn validate_trimmed(metrics: &GoInspiredStackMetrics) -> Result<(), Box<dyn Error>> {
    if metrics.active_stacks != 0
        || metrics.global_free_slots != 0
        || metrics.local_free_slots != 0
        || metrics.reserved_virtual_bytes != 0
        || metrics.accessible_virtual_bytes != 0
        || metrics.allocation_failures != 0
        || metrics.cleanup_failures != 0
    {
        return Err(format!("full trim left pool resources: {metrics:?}").into());
    }
    Ok(())
}
