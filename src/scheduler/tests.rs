use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;

use crate::machine::MachineId;
use crate::machine::worker::dispatch::DispatchTask;
use crate::routine::{Routine, RoutineMetadata};
use crate::stack::{StackConfig, StackManager};

use super::Scheduler;

#[test]
fn idle_processor_steals_a_bounded_batch_from_a_local_queue() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    let scheduler = Scheduler::new(2)?;
    let mut handles = Vec::new();
    let mut routines = Vec::new();
    for _ in 0..2 {
        let routine = Routine::new(&stack_manager, RoutineMetadata::new())?;
        routine.reserve_for_dispatch()?;
        let (task, handle) = DispatchTask::new(MachineId::new(0), routine.clone(), || {});
        scheduler.publish(task, 0).map_err(|(_, message)| message)?;
        handles.push(handle);
        routines.push(routine);
    }

    let first = scheduler
        .take_next(0)?
        .ok_or("first Processor failed to select published work")?;
    let stolen = scheduler
        .take_next(1)?
        .ok_or("idle Processor failed to steal available work")?;
    let mut selected = HashSet::new();
    for task in [first, stolen] {
        selected.insert(
            task.routine()
                .ok_or("selected task no longer owns its routine")?
                .id(),
        );
        drop(task);
    }
    assert_eq!(selected.len(), 2);
    assert!(
        routines
            .iter()
            .all(|routine| selected.contains(&routine.id()))
    );
    for handle in handles {
        assert!(handle.join().is_err());
    }
    Ok(())
}

#[test]
fn idle_processor_worker_steals_from_a_saturated_local_queue_without_losing_ownership()
-> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    let scheduler = Arc::new(Scheduler::new(2)?);
    let local_capacity = scheduler
        .state
        .lock()
        .map_err(|error| format!("scheduler lock poisoned in test: {error}"))?
        .local[0]
        .remaining_capacity();
    let mut handles = Vec::with_capacity(local_capacity);
    let mut expected = HashSet::with_capacity(local_capacity);
    {
        let mut state = scheduler
            .state
            .lock()
            .map_err(|error| format!("scheduler lock poisoned in test: {error}"))?;
        for _ in 0..local_capacity {
            let routine = Routine::new(&stack_manager, RoutineMetadata::new())?;
            routine.reserve_for_dispatch()?;
            expected.insert(routine.id());
            let (task, handle) = DispatchTask::new(MachineId::new(0), routine, || {});
            super::work_stealing::push_local(&mut state, 0, task);
            handles.push(handle);
        }
        assert_eq!(state.local[0].len(), local_capacity);
        assert_eq!(state.local[1].len(), 0);
    }

    let thief_scheduler = Arc::clone(&scheduler);
    let thief = std::thread::spawn(move || thief_scheduler.wait_next(1));
    let first_stolen = thief
        .join()
        .map_err(|payload| {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic payload");
            format!("idle Processor worker panicked: {message}")
        })??
        .ok_or("idle Processor worker did not steal available local work")?;
    let mut selected = HashSet::with_capacity(local_capacity);
    let first_stolen_id = first_stolen
        .routine()
        .ok_or("stolen task no longer owns its routine")?
        .id();
    assert!(selected.insert(first_stolen_id));
    drop(first_stolen);

    while let Some(task) = scheduler.take_next(1)? {
        let id = task
            .routine()
            .ok_or("stolen task no longer owns its routine")?
            .id();
        assert!(
            selected.insert(id),
            "routine {id:?} was selected more than once"
        );
        drop(task);
    }
    while let Some(task) = scheduler.take_next(0)? {
        let id = task
            .routine()
            .ok_or("remaining local task no longer owns its routine")?
            .id();
        assert!(
            selected.insert(id),
            "routine {id:?} was selected more than once"
        );
        drop(task);
    }

    assert_eq!(selected, expected);
    let stats = scheduler.stats()?;
    assert!(stats.stolen_tasks > 0);
    assert!(stats.steal_attempts > 0);
    assert!(stats.failed_steal_attempts > 0);
    assert!(stats.tasks_by_processor[1] > 0);
    assert_eq!(
        stats.tasks_by_processor.iter().sum::<u64>(),
        local_capacity as u64
    );
    assert_eq!(stats.local_queue_lengths, vec![0, 0]);
    for handle in handles {
        assert!(handle.join().is_err());
    }
    Ok(())
}

#[test]
fn publication_wakes_a_parked_worker_without_losing_the_task() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    let scheduler = std::sync::Arc::new(Scheduler::new(1)?);
    let waiter_scheduler = std::sync::Arc::clone(&scheduler);
    let waiter = std::thread::spawn(move || {
        waiter_scheduler
            .wait_next(0)
            .map(|task| task.map(|task| task.routine().map(Routine::id)))
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if scheduler.stats()?.parked_workers == 1 {
            break;
        }
        if std::time::Instant::now() >= deadline {
            return Err("worker did not park while its queue was empty".into());
        }
        std::thread::yield_now();
    }

    let routine = Routine::new(&stack_manager, RoutineMetadata::new())?;
    routine.reserve_for_dispatch()?;
    let expected_id = routine.id();
    let (task, handle) = DispatchTask::new(MachineId::new(0), routine, || {});
    scheduler.publish(task, 0).map_err(|(_, message)| message)?;
    let selected = waiter
        .join()
        .map_err(|payload| {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic payload");
            format!("parked worker thread panicked: {message}")
        })??
        .ok_or("worker woke without a published task")?
        .ok_or("selected task did not own a routine")?;
    assert_eq!(selected, expected_id);
    assert!(handle.join().is_err());

    let stats = scheduler.stats()?;
    assert_eq!(stats.park_events, 1);
    assert_eq!(stats.wake_notifications, 1);
    assert_eq!(stats.wake_returns, 1);
    assert_eq!(stats.published_tasks, 1);
    assert_eq!(stats.selected_tasks, 1);
    assert_eq!(stats.parked_workers, 0);
    Ok(())
}
