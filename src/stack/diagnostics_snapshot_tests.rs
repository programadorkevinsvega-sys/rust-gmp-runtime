use std::error::Error;
use std::panic::resume_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::Duration;

use super::super::super::config::StackConfig;
use super::super::super::error::StackError;
use super::super::super::vma::read_vma_snapshot;
use super::super::StackManager;
use super::{channel_error, wait_for_refresh_waiters};

#[test]
fn allocation_does_not_wait_for_periodic_snapshot_io() -> Result<(), Box<dyn Error>> {
    let manager = Arc::new(StackManager::new(StackConfig::new(4096)?)?);
    manager.vma_snapshot()?;
    manager.lock_admission()?.expire_snapshot();

    let snapshot_started = Arc::new(Barrier::new(2));
    let snapshot_continue = Arc::new(Barrier::new(2));
    let refresher = Arc::clone(&manager);
    let started = Arc::clone(&snapshot_started);
    let continue_read = Arc::clone(&snapshot_continue);
    let refresh_worker = thread::spawn(move || {
        refresher
            .admission_with_snapshot_using(false, || {
                started.wait();
                continue_read.wait();
                read_vma_snapshot()
            })
            .map(|(admission, _)| drop(admission))
    });

    snapshot_started.wait();
    let allocator = Arc::clone(&manager);
    let (result_sender, result_receiver) = mpsc::channel();
    let allocation_worker = thread::spawn(move || {
        let result = allocator.allocate();
        result_sender
            .send(result)
            .map_err(|error| error.to_string())
    });
    let allocation = result_receiver.recv_timeout(Duration::from_secs(5));

    snapshot_continue.wait();
    let worker_result = allocation_worker
        .join()
        .map_err(|payload| resume_unwind(payload))?;
    worker_result.map_err(std::io::Error::other)?;
    let refresh_result = match refresh_worker.join() {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    };
    refresh_result?;
    let stack = allocation??;
    drop(stack);
    Ok(())
}

#[test]
fn concurrent_snapshot_requests_share_one_inflight_read() -> Result<(), Box<dyn Error>> {
    const WAITERS: usize = 4;
    let manager = Arc::new(StackManager::new(StackConfig::new(4096)?)?);
    manager.vma_snapshot()?;

    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let refresher = Arc::clone(&manager);
    let leader = thread::spawn(move || {
        refresher
            .admission_with_snapshot_using(true, || {
                started_sender.send(()).map_err(channel_error)?;
                release_receiver
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

    let reads = Arc::new(AtomicUsize::new(0));
    let mut waiters = Vec::new();
    for _ in 0..WAITERS {
        let manager = Arc::clone(&manager);
        let reads = Arc::clone(&reads);
        waiters.push(thread::spawn(move || {
            manager
                .admission_with_snapshot_using(true, || {
                    reads.fetch_add(1, Ordering::SeqCst);
                    read_vma_snapshot()
                })
                .map(|(admission, snapshot)| {
                    drop(admission);
                    snapshot
                })
        }));
    }

    wait_for_refresh_waiters(&manager, WAITERS)?;
    release_sender.send(())?;
    leader.join().map_err(|payload| resume_unwind(payload))??;
    for waiter in waiters {
        waiter.join().map_err(|payload| resume_unwind(payload))??;
    }
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    assert_eq!(manager.lock_admission()?.refresh_waiters(), 0);
    Ok(())
}

#[test]
fn failed_snapshot_wakes_waiter_and_allows_refresh_recovery() -> Result<(), Box<dyn Error>> {
    let manager = Arc::new(StackManager::new(StackConfig::new(4096)?)?);
    manager.vma_snapshot()?;

    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let refresher = Arc::clone(&manager);
    let failed_reader = thread::spawn(move || {
        refresher
            .admission_with_snapshot_using(true, || {
                started_sender.send(()).map_err(channel_error)?;
                release_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(channel_error)?;
                Err(StackError::VmaProcfs {
                    path: "/proc/self/maps",
                    source: std::io::Error::other("injected procfs read failure"),
                })
            })
            .map(|(admission, snapshot)| {
                drop(admission);
                snapshot
            })
    });
    started_receiver.recv_timeout(Duration::from_secs(5))?;

    let recovering_manager = Arc::clone(&manager);
    let recovering_reader = thread::spawn(move || {
        recovering_manager
            .admission_with_snapshot_using(true, read_vma_snapshot)
            .map(|(admission, snapshot)| {
                drop(admission);
                snapshot
            })
    });
    wait_for_refresh_waiters(&manager, 1)?;
    release_sender.send(())?;

    assert!(matches!(
        failed_reader
            .join()
            .map_err(|payload| resume_unwind(payload))?,
        Err(StackError::VmaProcfs {
            path: "/proc/self/maps",
            ..
        })
    ));
    recovering_reader
        .join()
        .map_err(|payload| resume_unwind(payload))??;
    let stack = manager.allocate()?;
    drop(stack);
    assert_eq!(manager.lock_admission()?.refresh_waiters(), 0);
    Ok(())
}
