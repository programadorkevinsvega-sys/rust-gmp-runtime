use std::error::Error;
use std::panic::resume_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use super::super::config::StackConfig;
use super::super::vma::VmaSnapshot;
use super::super::vma::read_vma_snapshot;
use super::StackManager;

#[path = "diagnostics_released_capacity_tests.rs"]
mod released_capacity_tests;
#[path = "diagnostics_snapshot_tests.rs"]
mod snapshot_tests;

#[test]
fn rejected_admission_refreshes_once_and_recovers_released_capacity() -> Result<(), Box<dyn Error>>
{
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    let limit = read_vma_snapshot()?.limit;
    let mut admission = manager.lock_admission()?;
    admission.observe(VmaSnapshot::from_counts(limit, limit - 1)?, 0)?;
    let generation = admission.snapshot_generation();
    drop(admission);

    let reads = AtomicUsize::new(0);
    let (admission, fresh) = manager.check_admission_with_reconciliation_using(1, 0, || {
        reads.fetch_add(1, Ordering::SeqCst);
        read_vma_snapshot()
    })?;
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_ne!(admission.snapshot_generation(), generation);
    assert!(fresh.current < limit - 1);
    drop(admission);
    Ok(())
}

#[test]
fn concurrent_rejection_reconciliation_coalesces_to_one_refresh() -> Result<(), Box<dyn Error>> {
    const REQUESTERS: usize = 4;
    let manager = Arc::new(StackManager::new(StackConfig::new(4096)?)?);
    let initial = manager.vma_snapshot()?;
    let mut admission = manager.lock_admission()?;
    admission.observe(VmaSnapshot::from_counts(initial.limit, initial.limit)?, 0)?;
    let generation = admission.snapshot_generation();
    drop(admission);

    let reads = Arc::new(AtomicUsize::new(0));
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let leader_manager = Arc::clone(&manager);
    let leader_reads = Arc::clone(&reads);
    let leader = thread::spawn(move || {
        leader_manager
            .refresh_after_rejection_using(generation, || {
                leader_reads.fetch_add(1, Ordering::SeqCst);
                started_sender.send(()).map_err(channel_error)?;
                release_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(channel_error)?;
                read_vma_snapshot()
            })
            .map(drop)
    });
    started_receiver.recv_timeout(Duration::from_secs(5))?;

    let mut requesters = Vec::new();
    for _ in 0..REQUESTERS {
        let manager = Arc::clone(&manager);
        let reads = Arc::clone(&reads);
        requesters.push(thread::spawn(move || {
            manager
                .refresh_after_rejection_using(generation, || {
                    reads.fetch_add(1, Ordering::SeqCst);
                    read_vma_snapshot()
                })
                .map(drop)
        }));
    }
    wait_for_refresh_waiters(&manager, REQUESTERS)?;
    release_sender.send(())?;
    leader.join().map_err(|payload| resume_unwind(payload))??;
    for requester in requesters {
        requester
            .join()
            .map_err(|payload| resume_unwind(payload))??;
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    Ok(())
}

fn wait_for_refresh_waiters(manager: &StackManager, expected: usize) -> Result<(), Box<dyn Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if manager.lock_admission()?.refresh_waiters() == expected {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("timed out waiting for {expected} VMA refresh waiters").into());
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn channel_error(error: impl std::fmt::Display) -> super::super::error::StackError {
    super::super::error::StackError::VmaProcfs {
        path: "VMA refresh test synchronization channel",
        source: std::io::Error::other(error.to_string()),
    }
}
