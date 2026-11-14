use super::*;

pub(super) fn run_synchronized_refill_scenario(
    allocator: &Arc<Allocator>,
    workers: usize,
    refill_target: usize,
    cache_bytes: usize,
    baseline: &Stats,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    allocator.reset_refill_peak()?;
    let before = allocator.stats()?;
    let mut timing_totals = RefillTimingTotals::default();
    let mut total_operations = 0usize;
    let mut total_elapsed_ns = 0u128;
    let mut all_latencies = Vec::new();
    for sample in 0..config.samples {
        let result = run_synchronized_refill_sample(
            Arc::clone(allocator),
            workers,
            refill_target,
            config.refill_waves,
        )?;
        let throughput = result.operations as f64 / (result.elapsed_ns as f64 / 1e9);
        total_operations = total_operations
            .checked_add(result.operations)
            .ok_or("synchronized refill operation count overflow")?;
        total_elapsed_ns = total_elapsed_ns
            .checked_add(result.elapsed_ns)
            .ok_or("synchronized refill elapsed time overflow")?;
        all_latencies.extend(result.wave_latencies_ns.iter().copied());
        let mut sample_latencies = result.wave_latencies_ns;
        let timing = allocator.take_go_inspired_timing();
        if let Some(snapshot) = timing {
            timing_totals.add(snapshot)?;
        }
        println!(
            "synchronized_refill_sample instrumented={} workers={workers} cache_bytes={cache_bytes} refill_target={refill_target} sample={sample} waves={} operations={} elapsed_ns={} ops_s={throughput:.2} wave_latencies_ns={:?} timing={timing:?}",
            allocator.is_instrumented(),
            config.refill_waves,
            result.operations,
            result.elapsed_ns,
            distribution(&mut sample_latencies),
        );
    }
    let after = allocator.stats()?;
    let expected_refills = config
        .samples
        .checked_mul(workers)
        .and_then(|count| count.checked_mul(config.refill_waves))
        .ok_or("synchronized refill request count overflow")?;
    let expected_slots = expected_refills
        .checked_mul(refill_target)
        .ok_or("synchronized refill slot count overflow")?;
    let refill_requests = delta(
        after.cache_refill_requests,
        before.cache_refill_requests,
        "cache refill requests",
    )?;
    let refill_slots = delta(
        after.cache_refill_slots,
        before.cache_refill_slots,
        "cache refill slots",
    )?;
    let active_delta = after
        .active
        .checked_sub(before.active)
        .ok_or("active stack count decreased during synchronized refill")?;
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
    if refill_requests != u64::try_from(expected_refills)?
        || refill_slots != u64::try_from(expected_slots)?
        || active_delta != 0
        || allocation_failure_delta != 0
        || cleanup_failure_delta != 0
    {
        return Err(format!(
            "synchronized refill accounting mismatch: requests={refill_requests}/{expected_refills} slots={refill_slots}/{expected_slots} active_delta={active_delta} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta}"
        )
        .into());
    }
    println!(
        "synchronized_refill_scenario instrumented={} workers={workers} cache_bytes={cache_bytes} refill_target={refill_target} samples={} waves_per_sample={} total_operations={total_operations} elapsed_ns={total_elapsed_ns} ops_s={:.2} refill_requests={refill_requests} refill_slots={refill_slots} max_concurrent_refills={} total_wave_latency_ns={:?} lock_wait_ns_total={} lock_hold_ns_total={} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta} before={before} after={after} retained_baseline={baseline}",
        allocator.is_instrumented(),
        config.samples,
        config.refill_waves,
        total_operations as f64 / (total_elapsed_ns as f64 / 1e9),
        after.max_concurrent_refills,
        distribution(&mut all_latencies),
        timing_totals.lock_wait_ns,
        timing_totals.lock_hold_ns,
    );
    Ok(())
}
