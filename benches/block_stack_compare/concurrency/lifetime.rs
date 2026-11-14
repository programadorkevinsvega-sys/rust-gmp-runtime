use super::*;

pub(super) fn run_lifetime_scenario(
    allocator: &Arc<Allocator>,
    workers: usize,
    cache_retention_operations: usize,
    cache_bytes: usize,
    baseline: &Stats,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    let lifetimes = [1, 4, 16];
    let before = allocator.stats()?;
    let mut timing_totals = RefillTimingTotals::default();
    let mut flush_timing_totals = RefillTimingTotals::default();
    let mut operations = 0usize;
    let mut total_elapsed_ns = 0u128;
    let mut latencies = Vec::new();
    for sample in 0..config.samples {
        let mut result = run_lifetime_sample(
            Arc::clone(allocator),
            workers,
            config.operations_per_worker,
            &lifetimes,
            cache_retention_operations,
        )?;
        operations = operations
            .checked_add(result.operations)
            .ok_or("lifecycle operation count overflow")?;
        total_elapsed_ns = total_elapsed_ns
            .checked_add(result.elapsed_ns)
            .ok_or("lifecycle elapsed time overflow")?;
        latencies.extend(result.latencies_ns.iter().copied());
        let mut sample_latencies = result.latencies_ns;
        let timing = allocator.take_go_inspired_timing();
        if let Some(snapshot) = timing {
            timing_totals.add(snapshot)?;
        }
        let throughput = result.operations as f64 / (result.elapsed_ns as f64 / 1e9);
        println!(
            "stack_lifetime_sample instrumented={} workers={workers} cache_bytes={cache_bytes} cache_retention_operations={cache_retention_operations} lifetime_schedule={lifetimes:?} sample={sample} operations={} elapsed_ns={} ops_s={throughput:.2} operation_latencies_ns={:?} timing={timing:?}",
            allocator.is_instrumented(),
            result.operations,
            result.elapsed_ns,
            distribution(&mut sample_latencies),
        );
        for cache in &mut result.worker_caches {
            allocator.flush_worker_cache(cache)?;
        }
        if let Some(snapshot) = allocator.take_go_inspired_timing() {
            flush_timing_totals.add(snapshot)?;
        }
    }
    let after = allocator.stats()?;
    let active_delta = after
        .active
        .checked_sub(before.active)
        .ok_or("active stack count decreased during lifetime scenario")?;
    let allocation_failure_delta = counter_delta(
        after.allocation_failures,
        before.allocation_failures,
        "allocation failures",
    )?;
    let cleanup_failure_delta = counter_delta(
        after.cleanup_failures,
        before.cleanup_failures,
        "cleanup failures",
    )?;
    let refill_request_delta = delta(
        after.cache_refill_requests,
        before.cache_refill_requests,
        "cache refill requests",
    )?;
    let refill_slot_delta = delta(
        after.cache_refill_slots,
        before.cache_refill_slots,
        "cache refill slots",
    )?;
    if active_delta != 0 || allocation_failure_delta != 0 || cleanup_failure_delta != 0 {
        return Err(format!(
            "lifetime-scenario accounting mismatch: active_delta={active_delta} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta}"
        )
        .into());
    }
    println!(
        "stack_lifetime_scenario instrumented={} workers={workers} cache_bytes={cache_bytes} cache_retention_operations={cache_retention_operations} lifetime_schedule={lifetimes:?} samples={} total_operations={operations} elapsed_ns={total_elapsed_ns} ops_s={:.2} refill_request_delta={refill_request_delta} refill_slot_delta={refill_slot_delta} cache_hit_delta={} cache_miss_delta={} refill_max_batch={} operation_latency_ns={:?} lock_wait_ns_total={} lock_hold_ns_total={} cache_flush_timing={flush_timing_totals:?} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta} before={before} after={after} retained_baseline={baseline}",
        allocator.is_instrumented(),
        config.samples,
        operations as f64 / (total_elapsed_ns as f64 / 1e9),
        delta(after.cache_hits, before.cache_hits, "cache hits")?,
        delta(after.cache_misses, before.cache_misses, "cache misses")?,
        after.cache_refill_max_batch,
        distribution(&mut latencies),
        timing_totals.lock_wait_ns,
        timing_totals.lock_hold_ns,
    );
    Ok(())
}
