use super::manager;
use std::error::Error;
use std::sync::atomic::Ordering;

#[test]
fn failed_arena_reservation_rolls_back_creation_and_admission() -> Result<(), Box<dyn Error>> {
    let manager = manager(2)?;
    manager.inner.faults.reserve.store(true, Ordering::SeqCst);
    assert!(manager.allocate().is_err());
    let failed = manager.metrics()?;
    assert_eq!(failed.arenas, 0);
    assert_eq!(failed.active_stacks, 0);
    assert_eq!(failed.allocation_failures, 1);
    let recovered = manager.allocate()?;
    drop(recovered);
    assert_eq!(manager.metrics()?.active_stacks, 0);
    Ok(())
}

#[test]
fn failed_slot_protection_returns_slot_to_free_state() -> Result<(), Box<dyn Error>> {
    let manager = manager(2)?;
    manager.inner.faults.enable.store(true, Ordering::SeqCst);
    assert!(manager.allocate().is_err());
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.free_slots, 2);
    assert_eq!(metrics.poisoned_slots, 0);
    assert_eq!(metrics.allocation_failures, 1);
    Ok(())
}

#[test]
fn failed_protection_rollback_quarantines_the_slot() -> Result<(), Box<dyn Error>> {
    let manager = manager(1)?;
    manager.inner.faults.enable.store(true, Ordering::SeqCst);
    manager.inner.faults.disable.store(true, Ordering::SeqCst);
    assert!(manager.allocate().is_err());
    let failed = manager.metrics()?;
    assert_eq!(failed.poisoned_slots, 1);
    assert_eq!(failed.free_slots, 0);
    assert_eq!(failed.active_stacks, 0);
    let next = manager.allocate()?;
    assert_eq!(manager.metrics()?.arenas, 2);
    drop(next);
    Ok(())
}

#[test]
fn failed_release_protection_quarantines_and_accounts_slot() -> Result<(), Box<dyn Error>> {
    let manager = manager(1)?;
    let stack = manager.allocate()?;
    manager.inner.faults.disable.store(true, Ordering::SeqCst);
    assert!(stack.release().is_err());
    let metrics = manager.metrics()?;
    assert_eq!(metrics.poisoned_slots, 1);
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.occupied_slots, 0);
    assert_eq!(metrics.accessible_virtual_bytes, 8192);
    Ok(())
}

#[test]
fn zero_capacity_and_arithmetic_overflow_are_rejected() -> Result<(), Box<dyn Error>> {
    let config = crate::stack::StackConfig::new(8192)?;
    assert!(crate::stack::BlockStackManager::new(config, 0).is_err());
    assert!(crate::stack::BlockStackManager::new(config, usize::MAX).is_err());
    Ok(())
}

#[test]
fn madvise_is_opt_in_and_counted_separately() -> Result<(), Box<dyn Error>> {
    let manager = crate::stack::BlockStackManager::new_benchmarked(
        crate::stack::StackConfig::new(8192)?,
        2,
        true,
    )?;
    let stack = manager.allocate()?;
    let (_, usable_start, _) = stack.bounds();
    // SAFETY: The handle owns a writable slot until it is released.
    unsafe { std::ptr::write_volatile(usable_start as *mut u8, 9) };
    drop(stack);
    let metrics = manager.metrics()?;
    let timing = manager.timing_snapshot_and_reset();
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.accessible_virtual_bytes, 0);
    assert_eq!(timing.madvise_calls, 1);
    assert_eq!(timing.mmap_calls, 1);
    assert_eq!(timing.mprotect_calls, 2);
    Ok(())
}

#[test]
fn uninstrumented_benchmark_manager_can_enable_madvise() -> Result<(), Box<dyn Error>> {
    let manager = crate::stack::BlockStackManager::new_benchmark_uninstrumented(
        crate::stack::StackConfig::new(8192)?,
        2,
        true,
    )?;
    assert!(manager.inner.madvise_dontneed);
    let stack = manager.allocate()?;
    drop(stack);
    assert_eq!(manager.metrics()?.active_stacks, 0);
    Ok(())
}
