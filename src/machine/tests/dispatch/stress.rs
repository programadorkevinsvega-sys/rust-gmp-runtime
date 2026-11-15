use std::error::Error;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

use crate::machine::{MachineManager, MachineState, RoutineJoinHandle};
use crate::processor::{ProcessorPool, ProcessorState};
use crate::routine::{Routine, RoutineCompletion, RoutineMetadata, RoutineState};
use crate::stack::{StackConfig, StackManager};

#[test]
fn concurrent_dispatch_is_exactly_once_at_required_scales() -> Result<(), Box<dyn Error>> {
    let stack_manager = Arc::new(StackManager::new(StackConfig::new(8 * 1024)?)?);
    for machine_count in [1, 2, 4] {
        for task_count in [128, 1_000, 10_000] {
            run_concurrent_case(&stack_manager, machine_count, task_count)?;
        }
    }
    Ok(())
}

fn run_concurrent_case(
    stack_manager: &StackManager,
    machine_count: usize,
    task_count: usize,
) -> Result<(), Box<dyn Error>> {
    let processors = Arc::new(ProcessorPool::new(machine_count)?);
    let machines = Arc::new(MachineManager::start(Arc::clone(&processors))?);
    let execution_counts = Arc::new(
        (0..task_count)
            .map(|_| AtomicUsize::new(0))
            .collect::<Vec<_>>(),
    );
    let invalid_states = Arc::new(AtomicUsize::new(0));
    let (handle_tx, handle_rx) = mpsc::channel::<(RoutineJoinHandle, Routine)>();
    let producer_count = 4.min(task_count);
    let mut producers = Vec::with_capacity(producer_count);

    for producer_index in 0..producer_count {
        let start = task_count * producer_index / producer_count;
        let end = task_count * (producer_index + 1) / producer_count;
        let machines = Arc::clone(&machines);
        let stack_manager = stack_manager.clone();
        let execution_counts = Arc::clone(&execution_counts);
        let invalid_states = Arc::clone(&invalid_states);
        let handle_tx = handle_tx.clone();
        producers.push(std::thread::spawn(move || -> Result<(), String> {
            for index in start..end {
                let routine = Routine::new(&stack_manager, RoutineMetadata::new())
                    .map_err(|error| error.to_string())?;
                let routine_in_entry = routine.clone();
                let counts_in_entry = Arc::clone(&execution_counts);
                let invalid_in_entry = Arc::clone(&invalid_states);
                let handle = machines
                    .dispatch(routine.clone(), move || {
                        if !matches!(routine_in_entry.state(), Ok(RoutineState::Running)) {
                            invalid_in_entry.fetch_add(1, Ordering::SeqCst);
                        }
                        counts_in_entry[index].fetch_add(1, Ordering::SeqCst);
                    })
                    .map_err(|error| error.to_string())?;
                handle_tx
                    .send((handle, routine))
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        }));
    }
    drop(handle_tx);

    for producer in producers {
        producer
            .join()
            .map_err(|payload| io::Error::other(panic_message(payload)))?
            .map_err(io::Error::other)?;
    }
    let mut completions = Vec::with_capacity(task_count);
    for _ in 0..task_count {
        completions.push(handle_rx.recv().map_err(io::Error::other)?);
    }
    let mut machines = Arc::try_unwrap(machines).map_err(|manager| {
        io::Error::other(format!(
            "dispatch producers retained MachineManager (strong count {})",
            Arc::strong_count(&manager)
        ))
    })?;
    machines.shutdown()?;

    for (handle, routine) in completions {
        handle.join()?;
        assert_eq!(routine.state()?, RoutineState::Dead);
        assert_eq!(routine.completion()?, Some(RoutineCompletion::Completed));
    }
    assert_eq!(invalid_states.load(Ordering::SeqCst), 0);
    assert!(
        execution_counts
            .iter()
            .all(|count| count.load(Ordering::SeqCst) == 1)
    );
    let snapshots = machines.machines()?;
    assert_eq!(snapshots.len(), machine_count);
    assert!(
        snapshots
            .iter()
            .all(|machine| machine.state == MachineState::Terminated)
    );
    for processor in processors.iter() {
        assert_eq!(processor.state()?, ProcessorState::Unowned);
    }
    Ok(())
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload")
        .to_owned()
}
