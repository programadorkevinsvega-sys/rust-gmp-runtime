use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::StackManager;

use super::workload::Workload;

static VALIDATED_ENTRIES: AtomicUsize = AtomicUsize::new(0);

pub(super) struct Timing {
    pub(super) stack_allocation: Duration,
    pub(super) dispatch_to_join: Duration,
    pub(super) end_to_end: Duration,
}

pub(super) fn run_batch(
    machines: &MachineManager,
    stacks: &StackManager,
    count: usize,
    workload: Workload,
    validate: bool,
) -> Result<Timing, Box<dyn Error>> {
    let started = Instant::now();
    let allocation_started = Instant::now();
    let mut routines = Vec::with_capacity(count);
    for _ in 0..count {
        routines.push(Routine::new(stacks, RoutineMetadata::new())?);
    }
    let stack_allocation = allocation_started.elapsed();
    let dispatch_started = Instant::now();
    let mut handles = Vec::with_capacity(count);
    for (index, routine) in routines.into_iter().enumerate() {
        if validate {
            handles.push(machines.dispatch(routine, move || {
                workload.run(index as u64);
                VALIDATED_ENTRIES.fetch_add(1, Ordering::Relaxed);
            })?);
        } else {
            handles.push(machines.dispatch(routine, move || workload.run(index as u64))?);
        }
    }
    for handle in handles {
        handle.join()?;
    }
    Ok(Timing {
        stack_allocation,
        dispatch_to_join: dispatch_started.elapsed(),
        end_to_end: started.elapsed(),
    })
}

pub(super) fn validate_execution(
    machines: &MachineManager,
    stacks: &StackManager,
    workers: usize,
) -> Result<(), Box<dyn Error>> {
    for workload in Workload::ALL {
        let count = workers.max(32);
        let before = VALIDATED_ENTRIES.load(Ordering::Relaxed);
        run_batch(machines, stacks, count, workload, true)?;
        let observed = VALIDATED_ENTRIES.load(Ordering::Relaxed) - before;
        if observed != count {
            return Err(format!("expected {count} entries, got {observed}").into());
        }
        println!(
            "validation workers={workers} workload={} expected={count} observed={observed} exactly_once=true",
            workload.name()
        );
    }
    Ok(())
}
