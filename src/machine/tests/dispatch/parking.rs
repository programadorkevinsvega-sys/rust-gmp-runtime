use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::ThreadId;
use std::time::Duration;

use crate::machine::MachineManager;
use crate::processor::ProcessorPool;
use crate::stack::{StackConfig, StackManager};

use super::super::new_routine;

#[test]
fn single_worker_consumes_a_burst_and_returns_to_parking() -> Result<(), Box<dyn Error>> {
    const TASK_COUNT: usize = 300;

    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    let mut machines = MachineManager::start(Arc::new(ProcessorPool::new(1)?))?;
    let executions = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(TASK_COUNT);
    for _ in 0..TASK_COUNT {
        let routine = new_routine(&stack_manager)?;
        let executions_in_entry = Arc::clone(&executions);
        handles.push(machines.dispatch(routine, move || {
            executions_in_entry.fetch_add(1, Ordering::Relaxed);
        })?);
    }
    for handle in handles {
        handle.join()?;
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let stats = loop {
        let stats = machines.scheduler_stats()?;
        if stats.parked_workers == 1 {
            break stats;
        }
        if std::time::Instant::now() >= deadline {
            return Err("single worker did not park after consuming the burst".into());
        }
        std::thread::yield_now();
    };
    assert_eq!(executions.load(Ordering::Relaxed), TASK_COUNT);
    assert_eq!(stats.published_tasks, TASK_COUNT as u64);
    assert_eq!(stats.selected_tasks, TASK_COUNT as u64);
    assert_eq!(stats.completed_tasks, TASK_COUNT as u64);
    assert_eq!(stats.tasks_by_processor, vec![TASK_COUNT as u64]);
    assert_eq!(stats.stolen_tasks, 0);
    assert_eq!(stats.local_queue_lengths, vec![0]);
    assert_eq!(stats.injection_queue_len, 0);
    assert_eq!(stats.global_queue_len, 0);
    machines.shutdown()?;
    Ok(())
}

#[test]
fn idle_workers_park_and_publication_wakes_them_for_fair_parallel_progress()
-> Result<(), Box<dyn Error>> {
    const WORKER_COUNT: usize = 4;

    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    let mut machines = MachineManager::start(Arc::new(ProcessorPool::new(WORKER_COUNT)?))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        if machines.scheduler_stats()?.parked_workers == WORKER_COUNT {
            break;
        }
        if std::time::Instant::now() >= deadline {
            return Err("workers did not park with empty run queues".into());
        }
        std::thread::yield_now();
    }

    let (entered_tx, entered_rx) = mpsc::channel::<ThreadId>();
    let mut releases = Vec::with_capacity(WORKER_COUNT);
    let mut handles = Vec::with_capacity(WORKER_COUNT);
    for _ in 0..WORKER_COUNT {
        let routine = new_routine(&stack_manager)?;
        let (release_tx, release_rx) = mpsc::channel();
        releases.push(release_tx);
        let entered = entered_tx.clone();
        handles.push(machines.dispatch(routine, move || {
            assert!(entered.send(std::thread::current().id()).is_ok());
            assert!(release_rx.recv().is_ok());
        })?);
    }
    drop(entered_tx);

    let mut active_workers = std::collections::HashSet::with_capacity(WORKER_COUNT);
    for _ in 0..WORKER_COUNT {
        active_workers.insert(entered_rx.recv_timeout(Duration::from_secs(2))?);
    }
    assert_eq!(active_workers.len(), WORKER_COUNT);
    for release in releases {
        release.send(())?;
    }
    for handle in handles {
        handle.join()?;
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let stats = loop {
        let stats = machines.scheduler_stats()?;
        if stats.parked_workers == WORKER_COUNT {
            break stats;
        }
        if std::time::Instant::now() >= deadline {
            return Err("workers did not return to parking after completing work".into());
        }
        std::thread::yield_now();
    };
    assert_eq!(stats.published_tasks, WORKER_COUNT as u64);
    assert_eq!(stats.selected_tasks, WORKER_COUNT as u64);
    assert_eq!(stats.completed_tasks, WORKER_COUNT as u64);
    assert!(stats.worker_state_transitions >= (WORKER_COUNT * 3) as u64);
    assert_eq!(stats.wake_notifications, WORKER_COUNT as u64);
    assert!(stats.park_events >= WORKER_COUNT as u64);
    assert_eq!(stats.parked_workers, WORKER_COUNT);
    assert_eq!(stats.local_queue_lengths, vec![0; WORKER_COUNT]);
    assert_eq!(stats.injection_queue_len, 0);
    assert_eq!(stats.global_queue_len, 0);
    assert_eq!(
        stats.tasks_by_processor.iter().sum::<u64>(),
        WORKER_COUNT as u64
    );
    machines.shutdown()?;
    Ok(())
}
