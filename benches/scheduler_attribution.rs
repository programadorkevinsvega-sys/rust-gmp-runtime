//! Controlled end-to-end scheduler experiment for attributing coordination cost.

use std::error::Error;
use std::sync::Arc;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::stack::{StackConfig, StackManager};

use self::sample::run_sample;
use self::summary::Summary;
use self::support::{format_counts, median, parse_env, parse_list};

#[path = "scheduler_attribution/sample.rs"]
mod sample;
#[path = "scheduler_attribution/summary.rs"]
mod summary;
#[path = "scheduler_attribution/support.rs"]
mod support;

const DEFAULT_TASKS: usize = 1_024;
const DEFAULT_SAMPLES: usize = 7;
const MAX_TASKS: usize = 4_096;
const STACK_BYTES: usize = 8 * 1024;
const DEFAULT_WORK_UNITS: [usize; 3] = [0, 64, 4_096];

fn main() -> Result<(), Box<dyn Error>> {
    let workers = parse_env("RUST_GMP_ATTRIBUTION_WORKERS", 4_usize)?;
    let publishers = parse_env("RUST_GMP_ATTRIBUTION_PUBLISHERS", 4_usize)?;
    let tasks = parse_env("RUST_GMP_ATTRIBUTION_TASKS", DEFAULT_TASKS)?;
    let samples = parse_env("RUST_GMP_ATTRIBUTION_SAMPLES", DEFAULT_SAMPLES)?;
    let work_units = parse_list(
        "RUST_GMP_ATTRIBUTION_WORK_UNITS",
        &DEFAULT_WORK_UNITS
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
    )?;
    let stealing = parse_env("RUST_GMP_ATTRIBUTION_STEALING", true)?;
    if !(1..=64).contains(&workers) {
        return Err("RUST_GMP_ATTRIBUTION_WORKERS must be between 1 and 64".into());
    }
    if publishers == 0 || publishers > tasks {
        return Err("RUST_GMP_ATTRIBUTION_PUBLISHERS must be between 1 and task count".into());
    }
    if tasks == 0 || tasks > MAX_TASKS {
        return Err(format!("task count must be between 1 and {MAX_TASKS}").into());
    }
    if samples < 3 {
        return Err("RUST_GMP_ATTRIBUTION_SAMPLES must be at least 3".into());
    }
    if work_units.iter().any(|units| *units > 1_000_000) {
        return Err("work units must not exceed 1,000,000".into());
    }

    let stack_manager = StackManager::new(StackConfig::new(STACK_BYTES)?)?;
    let manager = Arc::new(MachineManager::start(Arc::new(ProcessorPool::new(
        workers,
    )?))?);
    manager.benchmark_set_stealing_enabled(stealing)?;
    println!(
        "scheduler_attribution target={}-{} workers={workers} publishers={publishers} tasks={tasks} samples={samples} work_units={} stealing={} one_manager_per_process=true diagnostics=false benchmark_control=enabled",
        std::env::consts::OS,
        std::env::consts::ARCH,
        work_units
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        if stealing { "enabled" } else { "disabled" },
    );

    run_sample(&manager, &stack_manager, tasks, publishers, 64)?;
    let mut summaries = work_units
        .iter()
        .map(|work_units| Summary {
            work_units: *work_units,
            submit: Vec::with_capacity(samples),
            post_submit_drain: Vec::with_capacity(samples),
            end_to_end: Vec::with_capacity(samples),
            dispatch_sum: Vec::with_capacity(samples),
            dispatch_max: Vec::with_capacity(samples),
            selected_by_processor: vec![0_u64; workers],
            peak_local_queues: vec![0_usize; workers],
            stolen_tasks: 0,
            steal_attempts: 0,
            failed_steal_attempts: 0,
        })
        .collect::<Vec<_>>();
    for sample_index in 0..samples {
        for offset in 0..work_units.len() {
            let work_index = (sample_index + offset) % work_units.len();
            let units = work_units[work_index];
            let sample = run_sample(&manager, &stack_manager, tasks, publishers, units)?;
            let summary = &mut summaries[work_index];
            summary.submit.push(sample.submit_ns);
            summary.post_submit_drain.push(sample.post_submit_drain_ns);
            summary.end_to_end.push(sample.end_to_end_ns);
            summary
                .dispatch_sum
                .push(sample.publisher_dispatch_ns.iter().sum());
            summary.dispatch_max.push(
                sample
                    .publisher_dispatch_ns
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0),
            );
            summary.stolen_tasks = summary.stolen_tasks.saturating_add(sample.stolen_tasks);
            summary.steal_attempts = summary.steal_attempts.saturating_add(sample.steal_attempts);
            summary.failed_steal_attempts = summary
                .failed_steal_attempts
                .saturating_add(sample.failed_steal_attempts);
            for (total, count) in summary
                .selected_by_processor
                .iter_mut()
                .zip(&sample.selected_by_processor)
            {
                *total = total.saturating_add(*count);
            }
            for (peak, observed) in summary
                .peak_local_queues
                .iter_mut()
                .zip(&sample.peak_local_queues)
            {
                *peak = (*peak).max(*observed);
            }
            println!(
                "sample={sample_index} workers={workers} publishers={publishers} tasks={tasks} work_units={units} stealing={} submit_wall_ns={} post_submit_drain_ns={} end_to_end_ns={} publisher_dispatch_ns={} publisher_dispatch_sum_ns={} publisher_dispatch_max_ns={} steal_attempts={} failed_steals={} stolen_tasks={} selected_by_processor={} peak_local_queue_cumulative={}",
                if stealing { "enabled" } else { "disabled" },
                sample.submit_ns,
                sample.post_submit_drain_ns,
                sample.end_to_end_ns,
                format_counts(&sample.publisher_dispatch_ns),
                sample.publisher_dispatch_ns.iter().sum::<u128>(),
                sample
                    .publisher_dispatch_ns
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0),
                sample.steal_attempts,
                sample.failed_steal_attempts,
                sample.stolen_tasks,
                format_counts(&sample.selected_by_processor),
                format_counts(&sample.peak_local_queues),
            );
        }
    }
    for summary in &mut summaries {
        let Summary {
            work_units: units,
            submit,
            post_submit_drain,
            end_to_end,
            dispatch_sum,
            dispatch_max,
            selected_by_processor,
            peak_local_queues,
            stolen_tasks,
            steal_attempts,
            failed_steal_attempts,
        } = summary;
        println!(
            "summary workers={workers} publishers={publishers} tasks={tasks} work_units={units} stealing={} median_submit_wall_ns={} median_post_submit_drain_ns={} median_end_to_end_ns={} median_publisher_dispatch_sum_ns={} median_publisher_dispatch_max_ns={} total_steal_attempts={steal_attempts} total_failed_steals={failed_steal_attempts} total_stolen={stolen_tasks} selected_by_processor={} peak_local_queue_cumulative={}",
            if stealing { "enabled" } else { "disabled" },
            median(submit),
            median(post_submit_drain),
            median(end_to_end),
            median(dispatch_sum),
            median(dispatch_max),
            format_counts(selected_by_processor),
            format_counts(peak_local_queues),
        );
    }

    let mut manager = Arc::try_unwrap(manager).map_err(|manager| {
        format!(
            "attribution manager still has {} outstanding references",
            Arc::strong_count(&manager)
        )
    })?;
    manager.shutdown()?;
    Ok(())
}
