use super::*;

pub(super) fn run_refill_burst_scenario(
    allocator: &Arc<Allocator>,
    workers: usize,
    burst_multiplier: usize,
    cache_bytes: usize,
    baseline: &Stats,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    let refill_target = allocator
        .cache_refill_target()
        .ok_or("refill-burst scenario requires the Go-inspired allocator")?;
    let burst_size = refill_target
        .checked_mul(burst_multiplier)
        .ok_or("refill burst size overflow")?;
    allocator.reset_refill_peak()?;
    let before = allocator.stats()?;
    let mut timing_totals = RefillTimingTotals::default();
    let mut cache_flush_timing_totals = RefillTimingTotals::default();
    for sample in 0..config.samples {
        let mut result = run_refill_burst_sample(
            Arc::clone(allocator),
            workers,
            burst_size,
            config.bursts_per_worker,
        )?;
        let throughput = result.operations as f64 / (result.elapsed_ns as f64 / 1_000_000_000.0);
        let mut latencies = std::mem::take(&mut result.burst_latencies_ns);
        let timing = allocator.take_go_inspired_timing();
        if let Some(snapshot) = timing {
            timing_totals.add(snapshot)?;
        }
        let burst_count = latencies.len();
        println!(
            "cache_refill_burst_sample instrumented={} workers={workers} cache_bytes={cache_bytes} refill_target={refill_target} burst_multiplier={burst_multiplier} burst_size={burst_size} sample={sample} retained={} actual_ops={} bursts={burst_count} elapsed_ns={} ops_s={throughput:.2} burst_latency_samples={burst_count} burst_latencies_ns={:?} timing={timing:?}",
            allocator.is_instrumented(),
            config.retained_count,
            result.operations,
            result.elapsed_ns,
            distribution(&mut latencies),
        );
        for cache in &mut result.worker_caches {
            allocator.flush_worker_cache(cache)?;
        }
        if let Some(snapshot) = allocator.take_go_inspired_timing() {
            cache_flush_timing_totals.add(snapshot)?;
        }
    }
    let after = allocator.stats()?;
    let active_delta = after
        .active
        .checked_sub(before.active)
        .ok_or("active stack count decreased during refill-burst scenario")?;
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
    let refill_request_delta = after
        .cache_refill_requests
        .checked_sub(before.cache_refill_requests)
        .ok_or("cache refill request count decreased")?;
    let operations = config
        .samples
        .checked_mul(workers)
        .and_then(|count| count.checked_mul(burst_size))
        .and_then(|count| count.checked_mul(config.bursts_per_worker))
        .ok_or("refill-burst operation count overflow")?;
    let refill_slot_delta = delta(
        after.cache_refill_slots,
        before.cache_refill_slots,
        "refilled slots",
    )?;
    if active_delta != 0
        || allocation_failure_delta != 0
        || cleanup_failure_delta != 0
        || refill_request_delta == 0
    {
        return Err(format!(
            "refill-burst accounting failed: active_delta={active_delta} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta} refill_request_delta={refill_request_delta}"
        )
        .into());
    }
    let operations_per_refill = operations as f64 / refill_request_delta as f64;
    let instrumented = allocator.is_instrumented();
    let overlapping_refill_delta = delta(
        after.overlapping_refills,
        before.overlapping_refills,
        "overlapping refills",
    )?;
    println!(
        "cache_refill_burst_scenario instrumented={instrumented} workers={workers} cache_bytes={cache_bytes} refill_target={refill_target} burst_multiplier={burst_multiplier} burst_size={burst_size} bursts_per_worker={} total_ops={operations} refill_request_delta={refill_request_delta} refill_slot_delta={refill_slot_delta} operations_per_refill={operations_per_refill:.2} refill_max_batch={} overlapping_refill_delta={overlapping_refill_delta} max_concurrent_refills={} pool_lock_wait_ns_total={} pool_lock_hold_ns_total={} mmap_ns_total={} mprotect_ns_total={} munmap_ns_total={} mmap_calls_total={} mprotect_calls_total={} munmap_calls_total={} cache_flush_timing={cache_flush_timing_totals:?} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta} before={before} after={after} retained_baseline={baseline}",
        config.bursts_per_worker,
        after.cache_refill_max_batch,
        after.max_concurrent_refills,
        timing_totals.lock_wait_ns,
        timing_totals.lock_hold_ns,
        timing_totals.mmap_ns,
        timing_totals.mprotect_ns,
        timing_totals.munmap_ns,
        timing_totals.mmap_calls,
        timing_totals.mprotect_calls,
        timing_totals.munmap_calls,
    );
    Ok(())
}
