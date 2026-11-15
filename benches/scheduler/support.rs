use super::execution::run_parked_wake;
use super::types::WorkerSet;
use super::*;

pub(super) fn parse_worker_counts(value: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    let mut counts = Vec::new();
    let mut seen = HashSet::new();
    for item in value.split(',') {
        let count = item.parse::<usize>()?;
        if count == 0 || count > 64 {
            return Err("worker counts must be between 1 and 64".into());
        }
        if !seen.insert(count) {
            return Err(format!("duplicate worker count: {count}").into());
        }
        counts.push(count);
    }
    if counts.is_empty() {
        return Err("RUST_GMP_SCHEDULER_WORKERS must not be empty".into());
    }
    Ok(counts)
}

pub(super) fn percentile_u64(values: &mut [u64], percentile: usize) -> u64 {
    values.sort_unstable();
    let index = values.len().saturating_mul(percentile).div_ceil(100);
    values[index.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn percentile_u128(values: &mut [u128], percentile: usize) -> u128 {
    values.sort_unstable();
    let index = values.len().saturating_mul(percentile).div_ceil(100);
    values[index.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn percentile_f64(values: &mut [f64], percentile: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    let index = values.len().saturating_mul(percentile).div_ceil(100);
    values[index.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn median_absolute_deviation_u64(values: &[u64]) -> u64 {
    let median = percentile_u64(&mut values.to_vec(), 50);
    percentile_u64(
        &mut values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>(),
        50,
    )
}

pub(super) fn median_absolute_deviation_u128(values: &[u128]) -> u128 {
    let median = percentile_u128(&mut values.to_vec(), 50);
    percentile_u128(
        &mut values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>(),
        50,
    )
}

pub(super) fn median_absolute_deviation_f64(values: &[f64]) -> f64 {
    let median = percentile_f64(&mut values.to_vec(), 50);
    percentile_f64(
        &mut values
            .iter()
            .map(|value| (value - median).abs())
            .collect::<Vec<_>>(),
        50,
    )
}

pub(super) fn report_wake_summaries(
    worker_sets: &[WorkerSet],
    stack_manager: &StackManager,
    sample_count: usize,
) -> Result<(), Box<dyn Error>> {
    for set in worker_sets {
        let mut wake_latencies = Vec::with_capacity(sample_count);
        for sample_index in 0..sample_count {
            let latency_ns = run_parked_wake(&set.manager, stack_manager, set.count)?;
            wake_latencies.push(latency_ns);
            println!(
                "wake_sample={sample_index} mode=parked_wake workers={} workload=noop dispatch_to_entry_ns={latency_ns}",
                set.count
            );
        }
        println!(
            "wake_summary mode=parked_wake workers={} samples={sample_count} median_dispatch_to_entry_ns={} mad_dispatch_to_entry_ns={} p95_dispatch_to_entry_ns={}",
            set.count,
            percentile_u64(&mut wake_latencies.clone(), 50),
            median_absolute_deviation_u64(&wake_latencies),
            percentile_u64(&mut wake_latencies, 95)
        );
    }
    Ok(())
}
