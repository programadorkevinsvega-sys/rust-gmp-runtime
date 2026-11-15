use rust_gmp_runtime::stack::StackEngine;

use super::batch::Timing;
use super::engine_name;
use super::workload::Workload;

pub(super) fn report_case(
    engine: StackEngine,
    workers: usize,
    workload: Workload,
    tasks: usize,
    samples: &[Timing],
) {
    let allocation = samples
        .iter()
        .map(|sample| sample.stack_allocation.as_nanos())
        .collect::<Vec<_>>();
    let dispatch = samples
        .iter()
        .map(|sample| sample.dispatch_to_join.as_nanos())
        .collect::<Vec<_>>();
    let end_to_end = samples
        .iter()
        .map(|sample| sample.end_to_end.as_nanos())
        .collect::<Vec<_>>();
    for (metric, values) in [
        ("stack_alloc", allocation),
        ("dispatch_to_join", dispatch),
        ("end_to_end", end_to_end),
    ] {
        report_metric(engine, workers, workload, tasks, metric, values);
    }
}

fn report_metric(
    engine: StackEngine,
    workers: usize,
    workload: Workload,
    tasks: usize,
    metric: &str,
    mut values: Vec<u128>,
) {
    values.sort_unstable();
    let median = values[4] + (values[5] - values[4]) / 2;
    let mut deviations = values
        .iter()
        .map(|value| value.abs_diff(median))
        .collect::<Vec<_>>();
    deviations.sort_unstable();
    println!(
        "summary engine={} workers={workers} workload={} tasks={tasks} metric={metric} samples={} median_ns={median} mad_ns={} p95_max_observed_ns={} min_ns={} max_ns={} spread_ns={}",
        engine_name(engine),
        workload.name(),
        values.len(),
        deviations[5],
        values[9],
        values[0],
        values[9],
        values[9] - values[0],
    );
}
