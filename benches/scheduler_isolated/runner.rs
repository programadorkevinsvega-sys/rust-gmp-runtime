use super::execution::*;
use super::support::*;
use super::types::*;
use super::*;

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    let worker_count = parse_env("RUST_GMP_SCHEDULER_WORKERS", 1_usize)?;
    if !(1..=64).contains(&worker_count) {
        return Err("RUST_GMP_SCHEDULER_WORKERS must be between 1 and 64".into());
    }
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
        .any(|count| *count == 0 || *count > MAX_TASK_COUNT)
    {
        return Err(format!("task counts must be between 1 and {MAX_TASK_COUNT}").into());
    }
    let stack_manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    let mut manager = MachineManager::start(Arc::new(ProcessorPool::new(worker_count)?))?;
    println!(
        "scheduler_isolated target={}-{} workers={worker_count} samples={sample_count} task_counts={} workloads=noop,cpu64 scenarios=burst,flow,parked_wake stack_bytes={STACK_SIZE} one_manager_per_process=true",
        std::env::consts::OS,
        std::env::consts::ARCH,
        task_counts
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );

    for task_count in task_counts {
        for workload in [Workload::Noop, Workload::Cpu64] {
            for scenario in [Scenario::Burst, Scenario::Flow] {
                run_batch(
                    &manager,
                    &stack_manager,
                    32.min(task_count),
                    workload,
                    scenario,
                )?;
                let mut summary = Summary {
                    end_to_end_ns: Vec::with_capacity(sample_count),
                    throughput_per_second: Vec::with_capacity(sample_count),
                    stolen_tasks: 0,
                    steal_attempts: 0,
                    failed_steal_attempts: 0,
                    tasks_by_processor: vec![0; worker_count],
                    max_injection_queue_len: 0,
                    max_global_queue_len: 0,
                    max_local_queue_lengths: vec![0; worker_count],
                };
                for sample_index in 0..sample_count {
                    let sample =
                        run_batch(&manager, &stack_manager, task_count, workload, scenario)?;
                    summary.end_to_end_ns.push(sample.end_to_end_ns);
                    summary
                        .throughput_per_second
                        .push(sample.throughput_per_second);
                    summary.stolen_tasks = summary.stolen_tasks.saturating_add(sample.stolen_tasks);
                    summary.steal_attempts =
                        summary.steal_attempts.saturating_add(sample.steal_attempts);
                    summary.failed_steal_attempts = summary
                        .failed_steal_attempts
                        .saturating_add(sample.failed_steal_attempts);
                    for (total, sample) in summary
                        .tasks_by_processor
                        .iter_mut()
                        .zip(&sample.tasks_by_processor)
                    {
                        *total = total.saturating_add(*sample);
                    }
                    summary.max_injection_queue_len = summary
                        .max_injection_queue_len
                        .max(sample.peak_injection_queue_len);
                    summary.max_global_queue_len = summary
                        .max_global_queue_len
                        .max(sample.peak_global_queue_len);
                    for (peak, sample) in summary
                        .max_local_queue_lengths
                        .iter_mut()
                        .zip(&sample.peak_local_queue_lengths)
                    {
                        *peak = (*peak).max(*sample);
                    }
                    println!(
                        "sample={sample_index} workers={worker_count} scenario={} workload={} tasks={task_count} submit_ns={} drain_ns={} end_to_end_ns={} throughput_tasks_s={:.1} steal_attempts={} failed_steal_attempts={} stolen_tasks={} tasks_by_processor={} peak_injection_cumulative={} peak_global_cumulative={} peak_local_cumulative={}",
                        scenario.name(),
                        workload.name(),
                        sample.submit_ns,
                        sample.drain_ns,
                        sample.end_to_end_ns,
                        sample.throughput_per_second,
                        sample.steal_attempts,
                        sample.failed_steal_attempts,
                        sample.stolen_tasks,
                        sample
                            .tasks_by_processor
                            .iter()
                            .map(u64::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                        sample.peak_injection_queue_len,
                        sample.peak_global_queue_len,
                        sample
                            .peak_local_queue_lengths
                            .iter()
                            .map(usize::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                }
                println!(
                    "summary workers={worker_count} scenario={} workload={} tasks={task_count} samples={sample_count} median_end_to_end_ns={} mad_end_to_end_ns={} median_throughput_tasks_s={:.1} mad_throughput_tasks_s={:.1} total_steal_attempts={} total_failed_steal_attempts={} total_stolen={} tasks_by_processor={} max_injection_cumulative={} max_global_cumulative={} max_local_cumulative={}",
                    scenario.name(),
                    workload.name(),
                    percentile_u128(&mut summary.end_to_end_ns, 50),
                    median_absolute_deviation_u128(&summary.end_to_end_ns),
                    percentile_f64(&mut summary.throughput_per_second, 50),
                    median_absolute_deviation_f64(&summary.throughput_per_second),
                    summary.steal_attempts,
                    summary.failed_steal_attempts,
                    summary.stolen_tasks,
                    summary
                        .tasks_by_processor
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    summary.max_injection_queue_len,
                    summary.max_global_queue_len,
                    summary
                        .max_local_queue_lengths
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
    }
    let mut wake_latencies = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let latency_ns = run_parked_wake(&manager, &stack_manager, worker_count)?;
        wake_latencies.push(latency_ns);
        println!(
            "wake_sample={sample_index} workers={worker_count} dispatch_to_entry_ns={latency_ns}"
        );
    }
    println!(
        "wake_summary workers={worker_count} samples={sample_count} median_dispatch_to_entry_ns={} mad_dispatch_to_entry_ns={}",
        percentile_u64(&mut wake_latencies.clone(), 50),
        median_absolute_deviation_u64(&wake_latencies)
    );
    manager.shutdown()?;
    Ok(())
}
