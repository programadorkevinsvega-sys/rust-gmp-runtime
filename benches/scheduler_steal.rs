//! Scheduler-only benchmark for a loaded P and idle stealing Processors.

use std::error::Error;

use rust_gmp_runtime::scheduler_benchmark::run_imbalanced_steal_configured;
use rust_gmp_runtime::stack::{StackConfig, StackManager};

use self::support::{median_absolute_deviation, parse_env, parse_list, percentile};

#[path = "scheduler_steal/support.rs"]
mod support;

const DEFAULT_TASK_COUNTS: [usize; 3] = [256, 1_024, 4_096];
const DEFAULT_SAMPLE_COUNT: usize = 5;
const DEFAULT_THIEVES: [usize; 5] = [1, 2, 3, 7, 15];
const DEFAULT_STEAL_BATCHES: [usize; 5] = [1, 4, 8, 16, 32];
const LOCAL_BATCH: usize = 256;

fn main() -> Result<(), Box<dyn Error>> {
    let sample_count = parse_env("RUST_GMP_SCHEDULER_SAMPLES", DEFAULT_SAMPLE_COUNT)?;
    if sample_count < 2 {
        return Err("RUST_GMP_SCHEDULER_SAMPLES must be at least 2".into());
    }
    let task_counts = parse_list(
        "RUST_GMP_SCHEDULER_TASK_COUNTS",
        &DEFAULT_TASK_COUNTS.map(|count| count.to_string()).join(","),
    )?;
    if task_counts
        .iter()
        .any(|count| *count == 0 || count % LOCAL_BATCH != 0)
    {
        return Err(format!("task counts must be positive multiples of {LOCAL_BATCH}").into());
    }
    let thieves = parse_list(
        "RUST_GMP_SCHEDULER_THIEVES",
        &DEFAULT_THIEVES.map(|count| count.to_string()).join(","),
    )?;
    if thieves.iter().any(|count| *count == 0 || *count >= 64) {
        return Err("thief counts must be between 1 and 63".into());
    }
    let steal_batches = parse_list(
        "RUST_GMP_SCHEDULER_STEAL_BATCHES",
        &DEFAULT_STEAL_BATCHES
            .map(|count| count.to_string())
            .join(","),
    )?;
    if steal_batches
        .iter()
        .any(|count| *count == 0 || *count > LOCAL_BATCH)
    {
        return Err(format!("steal batches must be between 1 and {LOCAL_BATCH}").into());
    }
    let trace_tasks = parse_env("RUST_GMP_SCHEDULER_TRACE", false)?;
    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    println!(
        "scheduler_steal target={}-{} thieves={} steal_batches={} samples={sample_count} task_counts={} trace_tasks={trace_tasks} arrangement=single_loaded_local_queue workers=idle_thieves",
        std::env::consts::OS,
        std::env::consts::ARCH,
        thieves
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        steal_batches
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        task_counts
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );

    for thief_count in thieves {
        let processor_count = thief_count + 1;
        for steal_batch in &steal_batches {
            for task_count in &task_counts {
                let mut elapsed = Vec::with_capacity(sample_count);
                let mut total_stolen = 0_u64;
                let mut total_attempts = 0_u64;
                let mut total_failures = 0_u64;
                let mut total_drain_ns = vec![0_u128; thief_count];
                let mut total_barrier_wait_ns = vec![0_u128; thief_count];
                for sample_index in 0..sample_count {
                    let sample = run_imbalanced_steal_configured(
                        processor_count,
                        *task_count,
                        *steal_batch,
                        trace_tasks,
                        &stack_manager,
                    )?;
                    elapsed.push(sample.elapsed_ns);
                    total_stolen = total_stolen.saturating_add(sample.stolen_tasks);
                    total_attempts = total_attempts.saturating_add(sample.steal_attempts);
                    total_failures = total_failures.saturating_add(sample.failed_steal_attempts);
                    for (total, drain_ns) in total_drain_ns.iter_mut().zip(&sample.thief_drain_ns) {
                        *total = total.saturating_add(*drain_ns);
                    }
                    for (total, wait_ns) in total_barrier_wait_ns
                        .iter_mut()
                        .zip(&sample.thief_barrier_wait_ns)
                    {
                        *total = total.saturating_add(*wait_ns);
                    }
                    println!(
                        "sample={sample_index} thieves={thief_count} processors={processor_count} tasks={task_count} steal_batch={steal_batch} elapsed_ns={} steal_attempts={} failed_steal_attempts={} transfers={} tasks_by_processor={} thief_drain_ns={} thief_barrier_wait_ns={}",
                        sample.elapsed_ns,
                        sample.steal_attempts,
                        sample.failed_steal_attempts,
                        sample.stolen_tasks,
                        sample
                            .tasks_by_processor
                            .iter()
                            .map(u64::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                        sample
                            .thief_drain_ns
                            .iter()
                            .map(u128::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                        sample
                            .thief_barrier_wait_ns
                            .iter()
                            .map(u128::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    if trace_tasks {
                        for trace in sample.task_traces {
                            println!(
                                "task_trace sample={sample_index} thieves={thief_count} processors={processor_count} tasks={task_count} steal_batch={steal_batch} routine_id={} initial_processor={} transfers={} selected_by={}",
                                trace.routine_id,
                                trace.initial_processor,
                                trace
                                    .transfers
                                    .iter()
                                    .map(|(from, to)| format!("{from}>{to}"))
                                    .collect::<Vec<_>>()
                                    .join(","),
                                trace
                                    .selected_by
                                    .map_or_else(|| "none".to_owned(), |index| index.to_string())
                            );
                        }
                    }
                }
                let attempts_per_task =
                    total_attempts as f64 / (task_count.saturating_mul(sample_count) as f64);
                let failures_per_task =
                    total_failures as f64 / (task_count.saturating_mul(sample_count) as f64);
                let transfers_per_task =
                    total_stolen as f64 / (task_count.saturating_mul(sample_count) as f64);
                println!(
                    "summary thieves={thief_count} processors={processor_count} tasks={task_count} steal_batch={steal_batch} samples={sample_count} elapsed_use=diagnostic_only_if_trace median_elapsed_ns={} mad_elapsed_ns={} transfers_per_task={transfers_per_task:.4} attempts_per_task={attempts_per_task:.4} failures_per_task={failures_per_task:.4} total_transfers={total_stolen} total_attempts={total_attempts} total_failures={total_failures} mean_thief_drain_ns={} mean_thief_barrier_wait_ns={}",
                    percentile(&mut elapsed, 50),
                    median_absolute_deviation(&elapsed),
                    total_drain_ns
                        .iter()
                        .map(|duration| (duration / sample_count as u128).to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                    total_barrier_wait_ns
                        .iter()
                        .map(|duration| (duration / sample_count as u128).to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
    }
    Ok(())
}
