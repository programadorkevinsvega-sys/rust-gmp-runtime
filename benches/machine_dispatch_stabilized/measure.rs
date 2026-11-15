use std::error::Error;
use std::time::{Duration, Instant};

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::StackManager;

use super::{
    ENTRY_COUNT, SAMPLE_COUNT, TASK_COUNT, TimingSample, WARMUP_COUNT, WORKER_COUNTS, WorkerSet,
    Workload,
};

pub(super) fn warm_up(
    worker_sets: &[WorkerSet],
    stack_manager: &StackManager,
) -> Result<(), Box<dyn Error>> {
    for set in worker_sets {
        for workload in Workload::ALL {
            run_batch(&set.manager, stack_manager, WARMUP_COUNT, workload)?;
        }
    }
    Ok(())
}

pub(super) fn run_round(
    sample_index: usize,
    worker_sets: &mut [WorkerSet],
    stack_manager: &StackManager,
) -> Result<(), Box<dyn Error>> {
    for worker_offset in 0..worker_sets.len() {
        let worker_index = (sample_index + worker_offset) % worker_sets.len();
        for workload_offset in 0..Workload::ALL.len() {
            let workload_index =
                (sample_index + worker_offset + workload_offset) % Workload::ALL.len();
            let workload = Workload::ALL[workload_index];
            let set = &mut worker_sets[worker_index];
            let timing = run_batch(&set.manager, stack_manager, TASK_COUNT, workload)?;
            set.samples[workload.index()].push(TimingSample {
                submit: timing.submit,
                completion_drain: timing.completion_drain,
                end_to_end: timing.end_to_end,
            });
            println!(
                "machine_dispatch_stabilized sample={sample_index} workers={} workload={} submit_batch_ns={} completion_drain_ns={} end_to_end_batch_ns={}",
                set.count,
                workload.label(),
                timing.submit.as_nanos(),
                timing.completion_drain.as_nanos(),
                timing.end_to_end.as_nanos()
            );
        }
    }
    Ok(())
}

struct BatchTiming {
    submit: Duration,
    completion_drain: Duration,
    end_to_end: Duration,
}

fn run_batch(
    manager: &MachineManager,
    stack_manager: &StackManager,
    count: usize,
    workload: Workload,
) -> Result<BatchTiming, Box<dyn Error>> {
    let routines = prepare_routines(stack_manager, count)?;
    let started = Instant::now();
    let mut handles = Vec::with_capacity(count);
    for routine in routines {
        handles.push(manager.dispatch(routine, move || workload.execute())?);
    }
    let submit = started.elapsed();
    let drain_started = Instant::now();
    for handle in handles {
        handle.join()?;
    }
    Ok(BatchTiming {
        submit,
        completion_drain: drain_started.elapsed(),
        end_to_end: started.elapsed(),
    })
}

fn prepare_routines(
    stack_manager: &StackManager,
    count: usize,
) -> Result<Vec<Routine>, Box<dyn Error>> {
    let mut routines = Vec::with_capacity(count);
    for _ in 0..count {
        routines.push(Routine::new(stack_manager, RoutineMetadata::new())?);
    }
    Ok(routines)
}

pub(super) fn report_set(set: &WorkerSet) {
    for workload in Workload::ALL {
        let samples = &set.samples[workload.index()];
        report_metric(set.count, workload.label(), "submit", samples, |sample| {
            sample.submit
        });
        report_metric(
            set.count,
            workload.label(),
            "completion_drain",
            samples,
            |sample| sample.completion_drain,
        );
        report_metric(
            set.count,
            workload.label(),
            "end_to_end",
            samples,
            |sample| sample.end_to_end,
        );
    }
}

fn report_metric(
    workers: usize,
    workload: &str,
    metric: &str,
    samples: &[TimingSample],
    select: impl Fn(&TimingSample) -> Duration,
) {
    let mut values = samples
        .iter()
        .map(|sample| select(sample).as_nanos())
        .collect::<Vec<_>>();
    values.sort_unstable();
    let median = values[values.len() / 2];
    let mut deviations = values
        .iter()
        .map(|value| value.abs_diff(median))
        .collect::<Vec<_>>();
    deviations.sort_unstable();
    println!(
        "machine_dispatch_stabilized summary workers={workers} workload={workload} metric={metric} samples={} median_ns={median} mad_ns={} p95_ns={} max_observed_ns={max} range_ns={range}",
        values.len(),
        deviations[deviations.len() / 2],
        percentile(&values, 95),
        max = values[values.len() - 1],
        range = values[values.len() - 1] - values[0]
    );
}

fn percentile(sorted: &[u128], percentile: usize) -> u128 {
    let rank = (sorted.len() * percentile).div_ceil(100);
    sorted[rank.saturating_sub(1)]
}

pub(super) fn verify_entries() -> Result<(), Box<dyn Error>> {
    let expected =
        WORKER_COUNTS.len() * Workload::ALL.len() * (WARMUP_COUNT + SAMPLE_COUNT * TASK_COUNT);
    let observed = ENTRY_COUNT.load(std::sync::atomic::Ordering::Relaxed);
    if observed != expected {
        return Err(format!("expected {expected} benchmark entries, observed {observed}").into());
    }
    println!("machine_dispatch_stabilized entries_verified={observed}");
    Ok(())
}
