use super::VmaAdmission;
use crate::stack::{StackError, VmaSnapshot};

#[test]
fn pending_allocations_consume_vma_budget_before_syscalls_complete() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(21, 10)?, 0)?;
    budget.begin_allocation()?;
    assert!(matches!(
        budget.check_cached(5, 0),
        Err(StackError::VmaPreflightRejected {
            current: 12,
            required: 10,
            ..
        })
    ));
    budget.cancel_allocation()?;
    assert!(budget.check_cached(5, 0).is_ok());
    Ok(())
}

#[test]
fn allocations_finishing_during_snapshot_are_counted_conservatively() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(40, 10)?, 0)?;
    let token = budget
        .begin_refresh()
        .ok_or(StackError::MalformedVmaLimit)?;
    budget.begin_allocation()?;
    budget.finish_allocation()?;
    budget.finish_refresh(Ok(VmaSnapshot::from_counts(40, 10)?), token)?;

    assert!(matches!(
        budget.check_cached(15, 0),
        Err(StackError::VmaPreflightRejected {
            current: 12,
            required: 30,
            ..
        })
    ));
    Ok(())
}

#[test]
fn pending_release_keeps_capacity_charged_until_snapshot_reconciles() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(20, 12)?, 2)?;
    budget.begin_release()?;
    assert!(matches!(
        budget.check_cached(5, 0),
        Err(StackError::VmaPreflightRejected {
            current: 14,
            required: 10,
            ..
        })
    ));
    budget.finish_release()?;
    assert!(matches!(
        budget.check_cached(5, 0),
        Err(StackError::VmaPreflightRejected {
            current: 12,
            required: 10,
            ..
        })
    ));
    Ok(())
}

#[test]
fn snapshot_during_release_keeps_mapping_charged_until_release_commit() -> Result<(), StackError> {
    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(20, 12)?, 2)?;
    let token = budget
        .begin_refresh()
        .ok_or(StackError::MalformedVmaLimit)?;
    budget.begin_release()?;
    budget.finish_refresh(Ok(VmaSnapshot::from_counts(20, 12)?), token)?;

    assert!(matches!(
        budget.check_cached(4, 0),
        Err(StackError::VmaPreflightRejected {
            current: 14,
            required: 8,
            ..
        })
    ));
    budget.finish_release()?;
    assert!(budget.check_cached(4, 0).is_ok());
    Ok(())
}
