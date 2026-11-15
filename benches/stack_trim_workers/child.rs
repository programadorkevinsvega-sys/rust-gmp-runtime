use super::validation::{active_span_count, validate_retained_span, validate_retained_state};
use super::*;

pub(super) fn run_child(args: &[String]) -> Result<(), Box<dyn Error>> {
    let [_, workers, sample] = args else {
        return Err("isolated run requires worker count and sample index".into());
    };
    let workers = workers.parse::<usize>()?;
    if !WORKER_COUNTS.contains(&workers) {
        return Err("unsupported worker count".into());
    }
    let sample = sample.parse::<usize>()?;

    let config = StackConfig::new(STACK_SIZE)?.with_engine(StackEngine::GoInspired);
    let stacks = StackManager::with_go_inspired_options(
        config,
        GoInspiredOptions::default().with_slots_per_span(SLOTS_PER_SPAN),
    )?;
    let mut machines = MachineManager::start(Arc::new(ProcessorPool::new(workers)?))?;
    let baseline = stacks.metrics()?;

    let executions = Arc::new(AtomicUsize::new(0));
    let execution_started = Instant::now();
    let creation_started = Instant::now();
    let routines = (0..TASK_COUNT)
        .map(|_| Routine::new(&stacks, RoutineMetadata::new()))
        .collect::<Result<Vec<_>, _>>()?;
    let routine_creation_ns = creation_started.elapsed().as_nanos();

    let dispatch_started = Instant::now();
    let mut handles = Vec::with_capacity(TASK_COUNT);
    for routine in routines {
        let executions = Arc::clone(&executions);
        handles.push(machines.dispatch(routine, move || {
            executions.fetch_add(1, Ordering::Relaxed);
        })?);
    }
    let dispatch_ns = dispatch_started.elapsed().as_nanos();

    let join_started = Instant::now();
    for handle in handles {
        handle.join()?;
    }
    let join_ns = join_started.elapsed().as_nanos();
    let execution_ns = execution_started.elapsed().as_nanos();

    let observed = executions.load(Ordering::Relaxed);
    if observed != TASK_COUNT {
        return Err(format!("expected {TASK_COUNT} executions, observed {observed}").into());
    }

    let before_trim = stacks.metrics()?;
    validate_retained_state(&before_trim)?;
    let default_trim_started = Instant::now();
    let default_trimmed = stacks.trim()?;
    let default_trim_ns = default_trim_started.elapsed().as_nanos();
    let after_default_trim = stacks.metrics()?;
    let expected_default_trim = active_span_count(&before_trim)?.saturating_sub(1);
    if u64::try_from(default_trimmed)? != expected_default_trim {
        return Err(format!(
            "default trim count mismatch: expected={expected_default_trim} actual={default_trimmed}"
        )
        .into());
    }
    validate_retained_span(&after_default_trim, &stacks)?;

    let full_trim_started = Instant::now();
    let fully_trimmed = stacks.trim_to_retained_spans(0)?;
    let full_trim_ns = full_trim_started.elapsed().as_nanos();
    let after_full_trim = stacks.metrics()?;
    if after_full_trim.reserved_virtual_bytes != 0
        || after_full_trim.accessible_virtual_bytes != 0
        || after_full_trim.free_stacks != 0
        || after_full_trim.active_stacks != 0
    {
        return Err(format!("full trim left retained stack resources: {after_full_trim:?}").into());
    }

    println!(
        "trim_sample workers={workers} sample={sample} executions={observed} routine_creation_ns={routine_creation_ns} dispatch_ns={dispatch_ns} join_ns={join_ns} execution_wall_ns={execution_ns} spans_before={} free_slots_before={} reserved_before={} accessible_before={} process_vsize_baseline={:?} process_rss_baseline={:?} vmas_baseline={} process_vsize_before={:?} process_rss_before={:?} vmas_before={} default_trimmed_spans={default_trimmed} default_trim_ns={default_trim_ns} reserved_after_default={} accessible_after_default={} process_vsize_after_default={:?} process_rss_after_default={:?} vmas_after_default={} fully_trimmed_spans={fully_trimmed} full_trim_ns={full_trim_ns} reserved_after_full={} accessible_after_full={} process_vsize_after_full={:?} process_rss_after_full={:?} vmas_after_full={}",
        active_span_count(&before_trim)?,
        before_trim.free_stacks,
        before_trim.reserved_virtual_bytes,
        before_trim.accessible_virtual_bytes,
        baseline.process_virtual_bytes,
        baseline.process_resident_bytes,
        baseline.vma_current,
        before_trim.process_virtual_bytes,
        before_trim.process_resident_bytes,
        before_trim.vma_current,
        after_default_trim.reserved_virtual_bytes,
        after_default_trim.accessible_virtual_bytes,
        after_default_trim.process_virtual_bytes,
        after_default_trim.process_resident_bytes,
        after_default_trim.vma_current,
        after_full_trim.reserved_virtual_bytes,
        after_full_trim.accessible_virtual_bytes,
        after_full_trim.process_virtual_bytes,
        after_full_trim.process_resident_bytes,
        after_full_trim.vma_current,
    );
    machines.shutdown()?;
    Ok(())
}
