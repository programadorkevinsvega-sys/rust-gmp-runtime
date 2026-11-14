use super::{MAX_VMA_COST_PER_STACK, VmaAdmission};
use crate::stack::{StackError, VmaSnapshot};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

#[test]
fn admission_reserves_internal_cost_and_safety_margin() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(20, 10)?, 0)?;
    assert!(budget.check_cached(4, 2).is_ok());
    for _ in 0..4 {
        budget.note_allocation()?;
    }
    assert!(matches!(
        budget.check_cached(5, 2),
        Err(StackError::VmaPreflightRejected { .. })
    ));
    assert_eq!(MAX_VMA_COST_PER_STACK, 2);
    Ok(())
}

#[test]
fn admission_counts_allocations_after_snapshot() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(15, 10)?, 4)?;
    budget.note_allocation()?;
    assert!(matches!(
        budget.check_cached(2, 0),
        Err(StackError::VmaPreflightRejected {
            current: 12,
            required: 4,
            ..
        })
    ));
    Ok(())
}

#[test]
fn refresh_accounts_for_allocations_started_during_procfs_read() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(18, 10)?, 0)?;
    budget.expire_snapshot();
    let token = budget
        .begin_refresh()
        .ok_or(StackError::MalformedVmaLimit)?;
    budget.note_allocation()?;
    budget.finish_refresh(Ok(VmaSnapshot::from_counts(18, 10)?), token)?;

    assert!(matches!(
        budget.check_cached(4, 0),
        Err(StackError::VmaPreflightRejected {
            current: 12,
            required: 8,
            ..
        })
    ));
    Ok(())
}

#[test]
fn release_cannot_cancel_allocation_from_same_refresh_interval() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(17, 10)?, 2)?;
    budget.expire_snapshot();
    let token = budget
        .begin_refresh()
        .ok_or(StackError::MalformedVmaLimit)?;
    budget.note_allocation()?;
    budget.note_release()?;
    budget.finish_refresh(Ok(VmaSnapshot::from_counts(17, 10)?), token)?;

    assert!(matches!(
        budget.check_cached(3, 0),
        Err(StackError::VmaPreflightRejected {
            current: 12,
            required: 6,
            ..
        })
    ));
    Ok(())
}

#[test]
fn concurrent_admissions_do_not_overbook_synthetic_budget() -> Result<(), std::io::Error> {
    let admission = Arc::new(Mutex::new(VmaAdmission::default()));
    if let Ok(mut state) = admission.lock() {
        state
            .observe(
                VmaSnapshot::from_counts(10, 0)
                    .map_err(|error| std::io::Error::other(error.to_string()))?,
                0,
            )
            .map_err(|error| std::io::Error::other(error.to_string()))?;
    } else {
        return Err(std::io::Error::other("admission mutex was poisoned"));
    }

    let barrier = Arc::new(Barrier::new(8));
    let mut workers = Vec::new();
    for _ in 0..8 {
        let admission = Arc::clone(&admission);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<bool, std::io::Error> {
            barrier.wait();
            let mut state = admission.lock().map_err(|error| {
                std::io::Error::other(format!("admission mutex was poisoned: {error}"))
            })?;
            if state.check_cached(1, 0).is_ok() {
                state.note_allocation().map_err(|error| {
                    std::io::Error::other(format!("admission accounting failed: {error}"))
                })?;
                Ok(true)
            } else {
                Ok(false)
            }
        }));
    }
    let mut admitted = 0;
    for worker in workers {
        let result = match worker.join() {
            Ok(result) => result?,
            Err(payload) => {
                let message = match payload.downcast_ref::<String>() {
                    Some(message) => message.as_str(),
                    None => match payload.downcast_ref::<&str>() {
                        Some(message) => message,
                        None => "admission worker panicked with a non-string payload",
                    },
                };
                return Err(std::io::Error::other(message));
            }
        };
        if result {
            admitted += 1;
        }
    }
    assert_eq!(admitted, 5);
    Ok(())
}

#[path = "admission_pending_tests.rs"]
mod pending;
