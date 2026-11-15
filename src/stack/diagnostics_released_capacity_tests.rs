use std::error::Error;
use std::panic::resume_unwind;
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::Duration;

use super::super::super::allocator::FixedStackManager as StackManager;
use super::super::super::config::StackConfig;
use super::super::super::vma::{VmaSnapshot, read_vma_snapshot};
use super::channel_error;

#[test]
fn rejection_reconciliation_observes_release_during_snapshot() -> Result<(), Box<dyn Error>> {
    let manager = Arc::new(StackManager::new(StackConfig::new(4096)?)?);
    let limit = manager.vma_snapshot()?.limit;
    let stack = manager.allocate()?;

    let mut admission = manager.lock_admission()?;
    admission.observe(VmaSnapshot::from_counts(limit, limit - 1)?, 0)?;
    drop(admission);

    let (started_sender, started_receiver) = mpsc::channel();
    let (released_sender, released_receiver) = mpsc::channel();
    let rejecting_manager = Arc::clone(&manager);
    let rejection = thread::spawn(move || {
        rejecting_manager
            .check_admission_with_reconciliation_using(1, 0, || {
                started_sender.send(()).map_err(channel_error)?;
                released_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(channel_error)?;
                read_vma_snapshot()
            })
            .map(|(admission, snapshot)| {
                drop(admission);
                snapshot
            })
    });

    started_receiver.recv_timeout(Duration::from_secs(5))?;
    drop(stack);
    released_sender.send(())?;

    let fresh = rejection
        .join()
        .map_err(|payload| std::panic::resume_unwind(payload))??;
    assert!(fresh.current < limit - 1);
    assert_eq!(manager.metrics()?.active_stacks, 0);

    let replacement = manager.allocate()?;
    drop(replacement);
    Ok(())
}

#[test]
fn concurrent_allocations_and_releases_stay_conservative_then_recover() -> Result<(), Box<dyn Error>>
{
    const WORKERS: usize = 8;
    const HELD_STACKS: usize = WORKERS / 2;
    let manager = Arc::new(StackManager::new(StackConfig::new(4096)?)?);
    manager.vma_snapshot()?;

    let (started_sender, started_receiver) = mpsc::channel();
    let (continue_sender, continue_receiver) = mpsc::channel();
    let refreshing_manager = Arc::clone(&manager);
    let refresh = thread::spawn(move || {
        refreshing_manager
            .admission_with_snapshot_using(true, || {
                started_sender.send(()).map_err(channel_error)?;
                continue_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(channel_error)?;
                VmaSnapshot::from_counts(20, 10)
            })
            .map(|(admission, snapshot)| {
                drop(admission);
                snapshot
            })
    });
    started_receiver.recv_timeout(Duration::from_secs(5))?;

    let start = Arc::new(Barrier::new(WORKERS + 1));
    let (stack_sender, stack_receiver) = mpsc::channel();
    let mut workers = Vec::new();
    for worker_id in 0..WORKERS {
        let manager = Arc::clone(&manager);
        let start = Arc::clone(&start);
        let stack_sender = stack_sender.clone();
        workers.push(thread::spawn(move || -> Result<(), String> {
            start.wait();
            let stack = manager.allocate().map_err(|error| error.to_string())?;
            if worker_id < HELD_STACKS {
                drop(stack);
            } else {
                stack_sender
                    .send(stack)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        }));
    }
    drop(stack_sender);
    start.wait();
    for worker in workers {
        let result = match worker.join() {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        };
        result.map_err(std::io::Error::other)?;
    }
    let held: Vec<_> = stack_receiver.into_iter().collect();
    assert_eq!(held.len(), HELD_STACKS);

    continue_sender.send(())?;
    let refresh_result = match refresh.join() {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    };
    refresh_result?;
    let mut admission = manager.lock_admission()?;
    let estimate = match admission.check_cached(0, 0) {
        Err(error) => error,
        Ok(_) => return Err("synthetic VMA budget unexpectedly admitted".into()),
    };
    assert!(matches!(
        estimate,
        super::super::super::error::StackError::VmaPreflightRejected { current: 26, .. }
    ));
    drop(admission);
    drop(held);
    let (admission, fresh) =
        manager.check_admission_with_reconciliation_using(1, 0, read_vma_snapshot)?;
    assert!(fresh.current < fresh.limit);
    drop(admission);
    drop(manager.allocate()?);
    Ok(())
}
