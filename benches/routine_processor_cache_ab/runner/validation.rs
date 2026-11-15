use std::error::Error;

use super::AllocationMode;

pub(super) fn validate_metrics(
    mode: AllocationMode,
    workers: usize,
    expected: u64,
    metrics: &rust_gmp_runtime::stack::GoInspiredStackMetrics,
) -> Result<(), Box<dyn Error>> {
    if metrics.allocations != expected
        || metrics.releases != expected
        || metrics.active_stacks != 0
        || metrics.local_free_slots != 0
        || metrics.allocation_failures != 0
        || metrics.cleanup_failures != 0
    {
        return Err(
            format!("stack accounting mismatch: expected={expected} metrics={metrics:?}").into(),
        );
    }
    let expected_workers = u64::try_from(workers)?;
    match mode {
        AllocationMode::Global => {
            if metrics.cache_hits != 0
                || metrics.cache_misses != 0
                || metrics.cache_refills != 0
                || metrics.cache_refill_requests != 0
                || metrics.cache_refill_slots != 0
            {
                return Err("global path unexpectedly used local cache counters".into());
            }
        }
        AllocationMode::ProcessorCache => {
            if metrics.cache_misses != expected_workers
                || metrics.cache_hits != expected - expected_workers
                || metrics.cache_refills != expected_workers
                || metrics.cache_refill_requests != expected_workers
                || metrics.cache_refill_slots == 0
            {
                return Err("Processor cache hit/refill accounting was inconsistent".into());
            }
        }
    }
    Ok(())
}
