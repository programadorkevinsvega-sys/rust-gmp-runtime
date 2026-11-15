use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::StackManager;

use super::workload::Workload;

static VALIDATED_ENTRIES: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StageTotals {
    pub(super) creation_wall_ns: u128,
    pub(super) dispatch_ns: u128,
    pub(super) join_ns: u128,
    pub(super) end_to_end_ns: u128,
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    pub(super) creation_ns: u128,
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    pub(super) acquisition_ns: u128,
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    pub(super) run_once_ns: u128,
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    pub(super) finalization_ns: u128,
}

pub(super) struct CreatedBatch {
    routines: Vec<Routine>,
    started: Instant,
    creation_wall: Duration,
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    creation_ns: u128,
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    acquisition_ns: u128,
}

pub(super) fn create_batch(
    stacks: &StackManager,
    count: usize,
) -> Result<CreatedBatch, Box<dyn Error>> {
    let started = Instant::now();
    let mut routines = Vec::with_capacity(count);
    for _ in 0..count {
        routines.push(Routine::new(stacks, RoutineMetadata::new())?);
    }
    let creation_wall = started.elapsed();
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    let creation_ns = routines
        .iter()
        .map(|routine| u128::from(routine.benchmark_timing().creation_ns))
        .sum();
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    let acquisition_ns = routines
        .iter()
        .map(|routine| u128::from(routine.benchmark_timing().stack_acquisition_ns))
        .sum();
    Ok(CreatedBatch {
        routines,
        started,
        creation_wall,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        creation_ns,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        acquisition_ns,
    })
}

pub(super) fn execute_batch(
    machines: &MachineManager,
    created: CreatedBatch,
    workload: Workload,
    validate: bool,
    include_creation: bool,
) -> Result<StageTotals, Box<dyn Error>> {
    let CreatedBatch {
        routines,
        started: creation_started,
        creation_wall,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        creation_ns,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        acquisition_ns,
    } = created;
    let dispatch_started = Instant::now();
    let mut handles = Vec::with_capacity(routines.len());
    let mut measurements = Vec::with_capacity(routines.len());
    for (index, routine) in routines.into_iter().enumerate() {
        let routine_measurement = routine.clone();
        handles.push(machines.dispatch(routine, move || {
            workload.run(index as u64);
            if validate {
                VALIDATED_ENTRIES.fetch_add(1, Ordering::Relaxed);
            }
        })?);
        measurements.push(routine_measurement);
    }
    let dispatch_ns = dispatch_started.elapsed().as_nanos();
    let join_started = Instant::now();
    for handle in handles {
        handle.join()?;
    }
    let join_ns = join_started.elapsed().as_nanos();
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    let (run_once_ns, finalization_ns) = {
        let mut run_once_ns = 0_u128;
        let mut finalization_ns = 0_u128;
        for routine in &measurements {
            let timing = routine.benchmark_timing();
            run_once_ns += u128::from(timing.run_once_with_permit_ns);
            finalization_ns += u128::from(timing.finalization_ns);
        }
        (run_once_ns, finalization_ns)
    };
    drop(measurements);
    let end_to_end_ns = if include_creation {
        creation_started.elapsed().as_nanos()
    } else {
        dispatch_started.elapsed().as_nanos()
    };
    Ok(StageTotals {
        creation_wall_ns: creation_wall.as_nanos(),
        dispatch_ns,
        join_ns,
        end_to_end_ns,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        creation_ns,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        acquisition_ns,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        run_once_ns,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        finalization_ns,
    })
}

pub(super) fn validate(
    machines: &MachineManager,
    stacks: &StackManager,
    count: usize,
) -> Result<(), Box<dyn Error>> {
    let before = VALIDATED_ENTRIES.load(Ordering::Relaxed);
    let created = create_batch(stacks, count)?;
    execute_batch(machines, created, Workload::Noop, true, true)?;
    let observed = VALIDATED_ENTRIES.load(Ordering::Relaxed) - before;
    if observed != count {
        return Err(format!("expected {count} executions, observed {observed}").into());
    }
    println!("validation expected={count} observed={observed} exactly_once=true");
    Ok(())
}
