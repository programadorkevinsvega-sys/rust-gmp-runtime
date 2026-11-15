use super::execution::run_batch;
use super::support::*;
use super::types::*;
use super::*;

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    let sample_count = match env::var("RUST_GMP_SCHEDULER_SAMPLES") {
        Ok(value) => value.parse()?,
        Err(env::VarError::NotPresent) => DEFAULT_SAMPLE_COUNT,
        Err(error) => return Err(error.into()),
    };
    if sample_count < 2 {
        return Err("RUST_GMP_SCHEDULER_SAMPLES must be at least 2".into());
    }
    let worker_counts = match env::var("RUST_GMP_SCHEDULER_WORKERS") {
        Ok(value) => parse_worker_counts(&value)?,
        Err(env::VarError::NotPresent) => DEFAULT_WORKER_COUNTS.to_vec(),
        Err(error) => return Err(error.into()),
    };
    let stack_manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    let mut worker_sets = Vec::with_capacity(worker_counts.len());
    for count in worker_counts {
        worker_sets.push(WorkerSet {
            count,
            manager: MachineManager::start(Arc::new(ProcessorPool::new(count)?))?,
        });
    }

    println!(
        "scheduler_f5 target={}-{} samples={sample_count} tasks_per_sample={TASK_COUNT} warmup_tasks={WARMUP_TASK_COUNT} stack_bytes={STACK_SIZE} workers={} workloads=noop,cpu64 modes=throughput_uninstrumented,latency_instrumented,parked_wake ordering=rotating",
        std::env::consts::OS,
        std::env::consts::ARCH,
        worker_sets
            .iter()
            .map(|set| set.count.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    for set in &mut worker_sets {
        for workload in [Workload::Noop, Workload::Cpu64] {
            run_batch(
                &set.manager,
                &stack_manager,
                WARMUP_TASK_COUNT,
                workload,
                true,
            )?;
            run_batch(
                &set.manager,
                &stack_manager,
                WARMUP_TASK_COUNT,
                workload,
                false,
            )?;
        }
    }

    let mut summaries = (0..worker_sets.len())
        .map(|_| {
            (0..2)
                .map(|_| [Summary::default(), Summary::default()])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let cases = (0..worker_sets.len())
        .flat_map(|worker_index| {
            [Workload::Noop, Workload::Cpu64]
                .into_iter()
                .flat_map(move |workload| {
                    [false, true]
                        .into_iter()
                        .map(move |instrument_latency| (worker_index, workload, instrument_latency))
                })
        })
        .collect::<Vec<_>>();
    for sample_index in 0..sample_count {
        for case_offset in 0..cases.len() {
            let case_index = (case_offset + sample_index) % cases.len();
            let (worker_index, workload, instrument_latency) = cases[case_index];
            let set = &worker_sets[worker_index];
            let sample = run_batch(
                &set.manager,
                &stack_manager,
                TASK_COUNT,
                workload,
                instrument_latency,
            )?;
            let workload_index = match workload {
                Workload::Noop => 0,
                Workload::Cpu64 => 1,
            };
            let mode_index = usize::from(instrument_latency);
            let summary = &mut summaries[worker_index][workload_index][mode_index];
            summary.submit_ns.push(sample.submit_ns);
            summary.drain_ns.push(sample.drain_ns);
            summary.end_to_end_ns.push(sample.end_to_end_ns);
            summary
                .throughput_per_second
                .push(sample.throughput_per_second);
            if let Some(value) = sample.schedule_p50_ns {
                summary.schedule_p50_ns.push(value);
            }
            if let Some(value) = sample.schedule_p95_ns {
                summary.schedule_p95_ns.push(value);
            }
            println!(
                "sample={sample_index} mode={} workers={} workload={} tasks={TASK_COUNT} submit_ns={} drain_ns={} end_to_end_ns={} throughput_tasks_s={:.1} schedule_p50_ns={} schedule_p95_ns={} stolen_tasks={} park_events={} wake_notifications={} worker_state_transitions={} tasks_by_processor={} peak_injection={} peak_global={} peak_local={}",
                if instrument_latency {
                    "latency_instrumented"
                } else {
                    "throughput_uninstrumented"
                },
                set.count,
                workload.name(),
                sample.submit_ns,
                sample.drain_ns,
                sample.end_to_end_ns,
                sample.throughput_per_second,
                sample
                    .schedule_p50_ns
                    .map_or_else(|| "na".to_owned(), |value| value.to_string()),
                sample
                    .schedule_p95_ns
                    .map_or_else(|| "na".to_owned(), |value| value.to_string()),
                sample.stolen_tasks,
                sample.park_events,
                sample.wake_notifications,
                sample.worker_state_transitions,
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
    }

    super::support::report_wake_summaries(&worker_sets, &stack_manager, sample_count)?;

    for (worker_index, set) in worker_sets.iter_mut().enumerate() {
        set.manager.shutdown()?;
        for (workload_index, workload) in [Workload::Noop, Workload::Cpu64].into_iter().enumerate()
        {
            for (mode_index, mode) in ["throughput_uninstrumented", "latency_instrumented"]
                .into_iter()
                .enumerate()
            {
                let summary = &mut summaries[worker_index][workload_index][mode_index];
                let schedule_p50 = if summary.schedule_p50_ns.is_empty() {
                    "na".to_owned()
                } else {
                    percentile_u64(&mut summary.schedule_p50_ns, 50).to_string()
                };
                let schedule_p95 = if summary.schedule_p95_ns.is_empty() {
                    "na".to_owned()
                } else {
                    percentile_u64(&mut summary.schedule_p95_ns, 50).to_string()
                };
                println!(
                    "summary mode={mode} workers={} workload={} samples={} median_submit_ns={} median_drain_ns={} median_end_to_end_ns={} mad_end_to_end_ns={} p95_end_to_end_ns={} median_throughput_tasks_s={:.1} mad_throughput_tasks_s={:.1} median_schedule_p50_ns={} median_schedule_p95_ns={}",
                    set.count,
                    workload.name(),
                    sample_count,
                    percentile_u128(&mut summary.submit_ns, 50),
                    percentile_u128(&mut summary.drain_ns, 50),
                    percentile_u128(&mut summary.end_to_end_ns, 50),
                    median_absolute_deviation_u128(&summary.end_to_end_ns),
                    percentile_u128(&mut summary.end_to_end_ns, 95),
                    percentile_f64(&mut summary.throughput_per_second, 50),
                    median_absolute_deviation_f64(&summary.throughput_per_second),
                    schedule_p50,
                    schedule_p95
                );
            }
        }
    }
    Ok(())
}
