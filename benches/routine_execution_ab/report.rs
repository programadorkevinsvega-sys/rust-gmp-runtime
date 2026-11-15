use rust_gmp_runtime::stack::StackEngine;

use super::batch::StageTotals;
use super::workload::Workload;

type MetricSelector = (&'static str, fn(&StageTotals) -> u128);

pub(super) fn print_sample(
    engine: StackEngine,
    workers: usize,
    phase: &str,
    workload: Workload,
    sample: usize,
    stack_manager_setup_ns: u128,
    timing: StageTotals,
) {
    println!(
        "sample engine={engine:?} workers={workers} phase={phase} workload={} sample={sample} instrumentation_enabled={} stack_manager_setup_ns={stack_manager_setup_ns} creation_wall_ns={} dispatch_wall_ns={} join_wall_ns={} end_to_end_wall_ns={}",
        workload.name(),
        cfg!(feature = "runtime-benchmark-instrumentation"),
        timing.creation_wall_ns,
        timing.dispatch_ns,
        timing.join_ns,
        timing.end_to_end_ns,
    );
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    println!(
        "stages engine={engine:?} workers={workers} phase={phase} workload={} sample={sample} creation_sum_ns={} stack_acquisition_sum_ns={} run_once_with_permit_sum_ns={} finalization_sum_ns={} finalization_is_subset_of_run_once=true",
        workload.name(),
        timing.creation_ns,
        timing.acquisition_ns,
        timing.run_once_ns,
        timing.finalization_ns,
    );
}

pub(super) fn report_summary(
    engine: StackEngine,
    workers: usize,
    phase: &str,
    workload: Workload,
    samples: &[StageTotals],
) {
    let metrics: [MetricSelector; 4] = [
        ("creation_wall", |sample| sample.creation_wall_ns),
        ("dispatch_wall", |sample| sample.dispatch_ns),
        ("join_wall", |sample| sample.join_ns),
        ("end_to_end_wall", |sample| sample.end_to_end_ns),
    ];
    for (metric, select) in metrics {
        let mut values = samples.iter().map(select).collect::<Vec<_>>();
        values.sort_unstable();
        let median = values[4] + (values[5] - values[4]) / 2;
        let mut deviations = values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>();
        deviations.sort_unstable();
        println!(
            "summary engine={engine:?} workers={workers} phase={phase} workload={} metric={metric} samples={} median_ns={median} mad_ns={} min_ns={} max_ns={} spread_ns={}",
            workload.name(),
            values.len(),
            deviations[5],
            values[0],
            values[9],
            values[9] - values[0],
        );
    }
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    report_instrumented_stages(engine, workers, phase, workload, samples);
}

#[cfg(feature = "runtime-benchmark-instrumentation")]
fn report_instrumented_stages(
    engine: StackEngine,
    workers: usize,
    phase: &str,
    workload: Workload,
    samples: &[StageTotals],
) {
    let metrics: [MetricSelector; 4] = [
        ("creation_sum", |sample| sample.creation_ns),
        ("stack_acquisition_sum", |sample| sample.acquisition_ns),
        ("run_once_with_permit_sum", |sample| sample.run_once_ns),
        ("finalization_sum", |sample| sample.finalization_ns),
    ];
    for (metric, select) in metrics {
        let mut values = samples.iter().map(select).collect::<Vec<_>>();
        values.sort_unstable();
        let median = values[4] + (values[5] - values[4]) / 2;
        let mut deviations = values
            .iter()
            .map(|value| value.abs_diff(median))
            .collect::<Vec<_>>();
        deviations.sort_unstable();
        println!(
            "summary engine={engine:?} workers={workers} phase={phase} workload={} metric={metric} samples={} median_ns={median} mad_ns={} min_ns={} max_ns={} spread_ns={}",
            workload.name(),
            values.len(),
            deviations[5],
            values[0],
            values[9],
            values[9] - values[0],
        );
    }
}
