use super::*;

pub(super) fn run_demand_refill_scenario(
    allocator: &Arc<Allocator>,
    workers: usize,
    mode_name: &str,
    mode: DemandRefillMode,
    cache_bytes: usize,
    baseline: &Stats,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    let demand_schedule = [1, 4, 16, 16, 4, 1, 0, 0];
    let before = allocator.stats()?;
    let mut timing_totals = RefillTimingTotals::default();
    let mut flush_timing_totals = RefillTimingTotals::default();
    let mut operations = 0usize;
    let mut total_elapsed_ns = 0u128;
    let mut latencies = Vec::new();
    let mut target_changes = 0usize;
    let mut max_target = 0usize;
    for sample in 0..config.samples {
        let mut result = run_demand_refill_sample(
            Arc::clone(allocator),
            workers,
            config.demand_epochs,
            &demand_schedule,
            mode,
        )?;
        operations = operations
            .checked_add(result.operations)
            .ok_or("demand-refill operation count overflow")?;
        total_elapsed_ns = total_elapsed_ns
            .checked_add(result.elapsed_ns)
            .ok_or("demand-refill elapsed time overflow")?;
        target_changes = target_changes.saturating_add(result.target_changes);
        max_target = max_target.max(result.max_refill_target);
        latencies.extend(result.latencies_ns.iter().copied());
        let mut sample_latencies = result.latencies_ns;
        let timing = allocator.take_go_inspired_timing();
        if let Some(snapshot) = timing {
            timing_totals.add(snapshot)?;
        }
        let throughput = result.operations as f64 / (result.elapsed_ns as f64 / 1e9);
        println!(
            "demand_refill_sample mode={mode_name} instrumented={} workers={workers} cache_bytes={cache_bytes} sample={sample} epochs={} operations={} elapsed_ns={} ops_s={throughput:.2} target_changes={} max_target={} phase_latencies_ns={:?} timing={timing:?}",
            allocator.is_instrumented(),
            config.demand_epochs,
            result.operations,
            result.elapsed_ns,
            result.target_changes,
            result.max_refill_target,
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
        .ok_or("active stack count decreased during demand-refill scenario")?;
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
            "demand-refill accounting mismatch: active_delta={active_delta} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta}"
        )
        .into());
    }
    println!(
        "demand_refill_scenario mode={mode_name} instrumented={} workers={workers} cache_bytes={cache_bytes} epochs_per_sample={} samples={} total_operations={operations} elapsed_ns={total_elapsed_ns} ops_s={:.2} target_changes={target_changes} max_target={max_target} refill_request_delta={refill_request_delta} refill_slot_delta={refill_slot_delta} cache_hit_delta={} cache_miss_delta={} refill_max_batch={} phase_latency_ns={:?} lock_wait_ns_total={} lock_hold_ns_total={} cache_flush_timing={flush_timing_totals:?} allocation_failure_delta={allocation_failure_delta} cleanup_failure_delta={cleanup_failure_delta} before={before} after={after} retained_baseline={baseline}",
        allocator.is_instrumented(),
        config.demand_epochs,
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
