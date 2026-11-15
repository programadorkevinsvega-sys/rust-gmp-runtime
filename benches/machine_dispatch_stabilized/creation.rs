use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackManager, StackMetrics};

use super::{SAMPLE_COUNT, TASK_COUNT, WORKER_COUNTS};

pub(super) fn measure(stack_manager: &StackManager) -> Result<(), Box<dyn Error>> {
    for workers in WORKER_COUNTS {
        let mut manager = MachineManager::start(Arc::new(ProcessorPool::new(workers)?))?;
        let before = stack_manager.metrics()?;
        let mut creation = Vec::with_capacity(SAMPLE_COUNT);
        let mut dispatch = Vec::with_capacity(SAMPLE_COUNT);
        let mut complete = Vec::with_capacity(SAMPLE_COUNT);
        let mut drops = Vec::with_capacity(SAMPLE_COUNT);

        for sample in 0..SAMPLE_COUNT {
            for offset in 0..3 {
                match (sample + offset) % 3 {
                    0 => {
                        let (created, elapsed, drop_elapsed) = measure_creation(stack_manager)?;
                        creation.push(elapsed);
                        drops.push(drop_elapsed);
                        println!(
                            "dispatch_creation sample={sample} workers={workers} phase=create_and_drop routines={TASK_COUNT} peak_active={} peak_reserved_bytes={} create_ns={} drop_ns={}",
                            created.active_stacks - before.active_stacks,
                            created.reserved_virtual_bytes - before.reserved_virtual_bytes,
                            elapsed.as_nanos(),
                            drop_elapsed.as_nanos()
                        );
                    }
                    1 => {
                        let elapsed = measure_dispatch(&manager, stack_manager)?;
                        dispatch.push(elapsed);
                        println!(
                            "dispatch_creation sample={sample} workers={workers} phase=dispatch_precreated routines={TASK_COUNT} dispatch_join_ns={}",
                            elapsed.as_nanos()
                        );
                    }
                    2 => {
                        let elapsed = measure_complete(&manager, stack_manager)?;
                        complete.push(elapsed);
                        println!(
                            "dispatch_creation sample={sample} workers={workers} phase=create_dispatch_join routines={TASK_COUNT} end_to_end_ns={}",
                            elapsed.as_nanos()
                        );
                    }
                    _ => return Err("invalid creation benchmark phase".into()),
                }
            }
        }

        manager.shutdown()?;
        let after = stack_manager.metrics()?;
        verify_accounting(&before, &after)?;
        println!(
            "dispatch_creation memory workers={workers} active_before={} active_after={} vma_before={} vma_after={} reserved_before={} reserved_after={}",
            before.active_stacks,
            after.active_stacks,
            before.vma_current,
            after.vma_current,
            before.reserved_virtual_bytes,
            after.reserved_virtual_bytes
        );
        report(workers, "create", &mut creation);
        report(workers, "dispatch_precreated", &mut dispatch);
        report(workers, "create_dispatch_join", &mut complete);
        report(workers, "drop", &mut drops);
    }
    Ok(())
}

fn measure_creation(
    stack_manager: &StackManager,
) -> Result<(StackMetrics, Duration, Duration), Box<dyn Error>> {
    let started = Instant::now();
    let routines = prepare_routines(stack_manager)?;
    let create_elapsed = started.elapsed();
    let live = stack_manager.metrics()?;
    let drop_started = Instant::now();
    drop(routines);
    Ok((live, create_elapsed, drop_started.elapsed()))
}

fn measure_dispatch(
    manager: &MachineManager,
    stack_manager: &StackManager,
) -> Result<Duration, Box<dyn Error>> {
    let routines = prepare_routines(stack_manager)?;
    let started = Instant::now();
    let (handles, owners) = dispatch_all(manager, routines)?;
    join_all(handles)?;
    let elapsed = started.elapsed();
    drop(owners);
    Ok(elapsed)
}

fn measure_complete(
    manager: &MachineManager,
    stack_manager: &StackManager,
) -> Result<Duration, Box<dyn Error>> {
    let started = Instant::now();
    let mut owners = Vec::with_capacity(TASK_COUNT);
    let mut handles = Vec::with_capacity(TASK_COUNT);
    for _ in 0..TASK_COUNT {
        let routine = Routine::new(stack_manager, RoutineMetadata::new())?;
        owners.push(routine.clone());
        handles.push(manager.dispatch(routine, || {})?);
    }
    join_all(handles)?;
    drop(owners);
    Ok(started.elapsed())
}

fn prepare_routines(stack_manager: &StackManager) -> Result<Vec<Routine>, Box<dyn Error>> {
    let mut routines = Vec::with_capacity(TASK_COUNT);
    for _ in 0..TASK_COUNT {
        routines.push(Routine::new(stack_manager, RoutineMetadata::new())?);
    }
    Ok(routines)
}

fn dispatch_all(
    manager: &MachineManager,
    routines: Vec<Routine>,
) -> Result<
    (
        Vec<rust_gmp_runtime::machine::RoutineJoinHandle>,
        Vec<Routine>,
    ),
    Box<dyn Error>,
> {
    let mut handles = Vec::with_capacity(routines.len());
    let mut owners = Vec::with_capacity(routines.len());
    for routine in routines {
        owners.push(routine.clone());
        handles.push(manager.dispatch(routine, || {})?);
    }
    Ok((handles, owners))
}

fn join_all(
    handles: Vec<rust_gmp_runtime::machine::RoutineJoinHandle>,
) -> Result<(), Box<dyn Error>> {
    for handle in handles {
        handle.join()?;
    }
    Ok(())
}

fn verify_accounting(before: &StackMetrics, after: &StackMetrics) -> Result<(), Box<dyn Error>> {
    let expected = (SAMPLE_COUNT * TASK_COUNT * 3) as u64;
    let allocated = after
        .successful_allocations
        .checked_sub(before.successful_allocations)
        .ok_or("stack allocation counter moved backwards")?;
    let released = after
        .released_stacks
        .checked_sub(before.released_stacks)
        .ok_or("stack release counter moved backwards")?;
    if allocated != expected || released != expected || after.active_stacks != before.active_stacks
    {
        return Err(format!(
            "expected {expected} allocated/released stacks and stable active count, observed {allocated}/{released} and {}/{}",
            before.active_stacks, after.active_stacks
        )
        .into());
    }
    println!(
        "dispatch_creation allocation_check stacks_allocated={allocated} stacks_released={released} active_stacks_before={} active_stacks_after={}",
        before.active_stacks, after.active_stacks
    );
    Ok(())
}

fn report(workers: usize, phase: &str, samples: &mut [Duration]) {
    samples.sort_unstable();
    let median = samples[samples.len() / 2].as_nanos();
    let mut deviations = samples
        .iter()
        .map(|sample| sample.as_nanos().abs_diff(median))
        .collect::<Vec<_>>();
    deviations.sort_unstable();
    let p95 = (samples.len() * 95).div_ceil(100).saturating_sub(1);
    let minimum = samples[0].as_nanos();
    let maximum = samples[samples.len() - 1].as_nanos();
    println!(
        "dispatch_creation summary workers={workers} phase={phase} samples={} routines_per_sample={TASK_COUNT} median_ns={median} mad_ns={} p95_ns={} max_observed_ns={maximum} range_ns={}",
        samples.len(),
        deviations[deviations.len() / 2],
        samples[p95].as_nanos(),
        maximum - minimum
    );
}
