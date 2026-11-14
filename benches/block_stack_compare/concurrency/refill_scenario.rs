use super::*;

pub(super) fn run_refill_scenario(
    allocator: &Arc<Allocator>,
    workers: usize,
    force_refill: bool,
    cache_bytes: usize,
    baseline: &Stats,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    allocator.reset_refill_peak()?;
    let before = allocator.stats()?;
    let mut timing_totals = RefillTimingTotals::default();
    for sample in 0..config.samples {
        let result = run_controlled_sample(
            Arc::clone(allocator),
            workers,
            config.operations_per_worker,
            allocator.is_instrumented(),
            false,
            force_refill,
        )?;
        let throughput = result.operations as f64 / (result.elapsed_ns as f64 / 1_000_000_000.0);
        let mut latencies = result.latencies_ns;
        let timing = allocator.take_go_inspired_timing();
        if let Some(snapshot) = timing {
            timing_totals.add(snapshot)?;
        }
        println!(
            "cache_refill_sample instrumented={} workers={workers} cache_bytes={cache_bytes} force_global_refill={force_refill} sample={sample} retained={} actual_ops={} elapsed_ns={} ops_s={throughput:.2} latency_samples={} latencies={:?} timing={timing:?}",
            allocator.is_instrumented(),
            config.retained_count,
            result.operations,
            result.elapsed_ns,
            latencies.len(),
            distribution(&mut latencies),
        );
    }
    let after = allocator.stats()?;
    let active_delta = after
        .active
        .checked_sub(before.active)
        .ok_or("active stack count decreased during cache refill scenario")?;
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
    if active_delta != 0 || allocation_failure_delta != 0 || cleanup_failure_delta != 0 {
        return Err(format!(
            "cache refill scenario changed active stacks or failure counts: active_delta={active_delta} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta}"
        )
        .into());
    }
    let refill_request_delta = after
        .cache_refill_requests
        .checked_sub(before.cache_refill_requests)
        .ok_or("cache refill request count decreased")?;
    let minimum_refills = if force_refill {
        config
            .samples
            .checked_mul(workers)
            .and_then(|count| count.checked_mul(config.operations_per_worker))
            .ok_or("expected refill count overflow")?
    } else {
        0
    };
    if refill_request_delta < u64::try_from(minimum_refills)? {
        return Err(format!(
            "forced-refill scenario requested {refill_request_delta} refills; expected at least {minimum_refills}"
        )
        .into());
    }
    println!(
        "cache_refill_scenario instrumented={} workers={workers} cache_bytes={cache_bytes} force_global_refill={force_refill} cache_hit_delta={} cache_miss_delta={} refill_request_delta={} refill_batch_delta={} refill_slot_delta={} refill_max_batch={} overlapping_refill_delta={} max_concurrent_refills={} pool_lock_wait_ns_total={} pool_lock_hold_ns_total={} mmap_ns_total={} mprotect_ns_total={} munmap_ns_total={} mmap_calls_total={} mprotect_calls_total={} munmap_calls_total={} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta} before={before} after={after} retained_baseline={baseline}",
        allocator.is_instrumented(),
        delta(after.cache_hits, before.cache_hits, "cache hits")?,
        delta(after.cache_misses, before.cache_misses, "cache misses")?,
        refill_request_delta,
        delta(after.cache_refills, before.cache_refills, "cache refills")?,
        delta(
            after.cache_refill_slots,
            before.cache_refill_slots,
            "refilled slots"
        )?,
        after.cache_refill_max_batch,
        delta(
            after.overlapping_refills,
            before.overlapping_refills,
            "overlapping refills"
        )?,
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
