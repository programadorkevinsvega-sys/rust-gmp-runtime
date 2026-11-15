use super::{AllocationMode, workload::Workload};

pub(super) fn summarize(mode: AllocationMode, workers: usize, samples: &[(Workload, u128)]) {
    for workload in Workload::ALL {
        let mut values = samples
            .iter()
            .filter_map(|(sample_workload, elapsed)| {
                (*sample_workload == workload).then_some(*elapsed)
            })
            .collect::<Vec<_>>();
        values.sort_unstable();
        let median = values[4] + (values[5] - values[4]) / 2;
        let mut deviations = values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>();
        deviations.sort_unstable();
        let median_ops_s = 256_000_000_000_f64 / median as f64;
        println!(
            "routine_processor_summary mode={} workers={workers} workload={} samples={} median_ns={median} mad_ns={} min_ns={} max_ns={} median_ops_s={median_ops_s:.2}",
            mode.name(),
            workload.name(),
            values.len(),
            deviations[5],
            values[0],
            values[9]
        );
    }
}
