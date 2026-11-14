use super::*;

pub(super) fn run_scenario(
    allocator: &Arc<Allocator>,
    strategy: &str,
    instrumented: bool,
    snapshots: bool,
    workers: usize,
    baseline: &Stats,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    let scenario_before = allocator.stats()?;
    allocator.take_timing();
    for sample in 0..config.samples {
        let result = run_controlled_sample(
            Arc::clone(allocator),
            workers,
            config.operations_per_worker,
            instrumented,
            snapshots,
            false,
        )?;
        let expected_snapshots = if snapshots { SNAPSHOTS_PER_SAMPLE } else { 0 };
        let observed_snapshots = result
            .snapshot
            .as_ref()
            .map_or(0, |report| report.durations_ns.len());
        if observed_snapshots != expected_snapshots
            || result
                .snapshot
                .as_ref()
                .is_some_and(|report| report.vma_counts.len() != expected_snapshots)
        {
            return Err(format!(
                "{strategy} performed {observed_snapshots} snapshots; expected {expected_snapshots}"
            )
            .into());
        }

        let throughput = result.operations as f64 / (result.elapsed_ns as f64 / 1_000_000_000.0);
        let mut latencies = result.latencies_ns;
        println!(
            "concurrent_sample strategy={strategy} instrumented={instrumented} snapshots={snapshots} snapshot_count={} workers={workers} sample={sample} retained={} minimum_ops_per_worker={} actual_ops={} elapsed_ns={} ops_s={throughput:.2} latency_samples={} latencies={:?} snapshotter={:?} timing={}",
            observed_snapshots,
            config.retained_count,
            config.operations_per_worker,
            result.operations,
            result.elapsed_ns,
            latencies.len(),
            distribution(&mut latencies),
            result.snapshot,
            allocator.take_timing(),
        );
    }
    let scenario_after = allocator.stats()?;
    let expected_active = u64::try_from(config.retained_count)?;
    if scenario_after.active != expected_active
        || scenario_after.allocation_failures != scenario_before.allocation_failures
        || scenario_after.cleanup_failures != scenario_before.cleanup_failures
    {
        return Err(format!(
            "allocator accounting changed during {strategy} concurrency: before={scenario_before:?} after={scenario_after:?}"
        )
        .into());
    }

    println!(
        "concurrent_scenario strategy={strategy} instrumented={instrumented} snapshots={snapshots} workers={workers} before={scenario_before} after={scenario_after} retained_baseline={baseline}"
    );
    Ok(())
}
