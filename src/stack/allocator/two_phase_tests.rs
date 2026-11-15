use std::ptr::NonNull;

use crate::stack::protection;
use crate::stack::{StackConfig, StackError, VmaSnapshot};

use super::FixedStackManager;

#[test]
fn successful_partial_mapping_cleanup_cancels_pending_admission() -> Result<(), StackError> {
    let (manager, _) = FixedStackManager::new_benchmarked_two_phase(StackConfig::new(4096)?)?;
    let base = protection::dispatch::reserve(manager.inner.reservation_size)?;
    {
        let mut admission = manager.lock_admission()?;
        admission.observe(VmaSnapshot::from_counts(21, 10)?, 0)?;
        admission.begin_allocation()?;
    }

    let failure = StackError::SystemCall {
        operation: "mprotect",
        source: std::io::Error::other("injected protection failure"),
    };
    let error = manager.cleanup_partial_mapping(base, failure);
    assert!(matches!(
        error,
        StackError::SystemCall {
            operation: "mprotect",
            ..
        }
    ));
    let mut admission = manager.lock_admission()?;
    assert!(admission.check_cached(5, 0).is_ok());
    Ok(())
}

#[test]
fn failed_partial_cleanup_keeps_pending_capacity_reserved() -> Result<(), StackError> {
    let (manager, _) = FixedStackManager::new_benchmarked_two_phase(StackConfig::new(4096)?)?;
    let mapping = protection::dispatch::reserve(manager.inner.reservation_size)?;
    {
        let mut admission = manager.lock_admission()?;
        admission.observe(VmaSnapshot::from_counts(21, 10)?, 0)?;
        admission.begin_allocation()?;
    }

    let failure = StackError::SystemCall {
        operation: "mprotect",
        source: std::io::Error::other("injected protection failure"),
    };
    let error = manager.cleanup_partial_mapping(NonNull::dangling(), failure);
    assert!(matches!(error, StackError::CleanupAfterFailure { .. }));
    let mut admission = manager.lock_admission()?;
    assert!(matches!(
        admission.check_cached(5, 0),
        Err(StackError::VmaPreflightRejected {
            current: 12,
            required: 10,
            ..
        })
    ));
    drop(admission);
    assert_eq!(manager.metrics()?.cleanup_failures, 1);
    protection::dispatch::release(mapping, manager.inner.reservation_size)?;
    Ok(())
}
