use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use crate::machine::{MachineManager, MachineState};
use crate::processor::ProcessorPool;
use crate::routine::RoutineCompletion;
use crate::stack::{StackConfig, StackManager};

use super::super::new_routine;

#[test]
fn dispatch_runs_many_routines_concurrently_with_a_bounded_worker_count()
-> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let processors = Arc::new(ProcessorPool::new(2)?);
    let mut machines = MachineManager::start(Arc::clone(&processors))?;
    let (entered_tx, entered_rx) = mpsc::channel();
    let active = Arc::new(AtomicUsize::new(0));
    let maximum_active = Arc::new(AtomicUsize::new(0));
    let execution_count = Arc::new(AtomicUsize::new(0));
    let invalid_state = Arc::new(AtomicUsize::new(0));
    let mut releases = Vec::new();
    let mut handles = Vec::new();
    let mut routines = Vec::new();

    for index in 0..2 {
        let routine = new_routine(&stack_manager)?;
        routines.push(routine.clone());
        let (release_tx, release_rx) = mpsc::channel();
        releases.push(release_tx);
        let entered = entered_tx.clone();
        let active = Arc::clone(&active);
        let maximum_active = Arc::clone(&maximum_active);
        let execution_count = Arc::clone(&execution_count);
        let routine_state = routine.clone();
        let invalid_state_in_entry = Arc::clone(&invalid_state);
        handles.push(machines.dispatch(routine, move || {
            execution_count.fetch_add(1, Ordering::SeqCst);
            if !matches!(
                routine_state.state(),
                Ok(crate::routine::RoutineState::Running)
            ) {
                invalid_state_in_entry.fetch_add(1, Ordering::SeqCst);
            }
            let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
            maximum_active.fetch_max(now_active, Ordering::SeqCst);
            if entered.send(index).is_ok() {
                match release_rx.recv_timeout(Duration::from_secs(3)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {}
                }
            }
            active.fetch_sub(1, Ordering::SeqCst);
        })?);
    }

    let first = entered_rx.recv_timeout(Duration::from_secs(2))?;
    let second = entered_rx.recv_timeout(Duration::from_secs(2))?;
    assert_ne!(first, second);
    assert!(
        machines
            .machines()?
            .iter()
            .all(|machine| matches!(machine.state, MachineState::Running { .. }))
    );

    for _ in 2..20 {
        let routine = new_routine(&stack_manager)?;
        routines.push(routine.clone());
        let execution_count = Arc::clone(&execution_count);
        handles.push(machines.dispatch(routine, move || {
            execution_count.fetch_add(1, Ordering::SeqCst);
        })?);
    }
    let release_thread = std::thread::spawn(move || -> Result<(), mpsc::SendError<()>> {
        std::thread::sleep(Duration::from_millis(10));
        for release in releases {
            release.send(())?;
        }
        Ok(())
    });
    machines.shutdown()?;
    release_thread
        .join()
        .map_err(|payload| {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic payload");
            std::io::Error::other(format!("release helper thread panicked: {message}"))
        })?
        .map_err(std::io::Error::other)?;
    for handle in handles {
        if let Err(error) = handle.join() {
            return Err(Box::new(std::io::Error::other(error.to_string())));
        }
    }

    assert_eq!(machines.machines()?.len(), 2);
    assert_eq!(maximum_active.load(Ordering::SeqCst), 2);
    assert_eq!(execution_count.load(Ordering::SeqCst), 20);
    assert_eq!(invalid_state.load(Ordering::SeqCst), 0);
    assert!(
        machines
            .machines()?
            .iter()
            .all(|machine| machine.state == MachineState::Terminated)
    );
    for routine in routines {
        assert_eq!(routine.state()?, crate::routine::RoutineState::Dead);
        assert_eq!(routine.completion()?, Some(RoutineCompletion::Completed));
    }
    Ok(())
}
