use crate::stack::{BlockStackManager, StackConfig};
use std::collections::BTreeSet;
use std::error::Error;
use std::sync::Arc;

#[path = "tests/concurrency.rs"]
mod concurrency;
#[path = "tests/failures.rs"]
mod failures;
#[path = "tests/layout.rs"]
mod layout;

fn manager(slots: usize) -> Result<BlockStackManager, Box<dyn Error>> {
    Ok(BlockStackManager::new(StackConfig::new(8192)?, slots)?)
}

#[test]
fn allocates_unique_non_overlapping_slots_with_stable_bounds() -> Result<(), Box<dyn Error>> {
    let manager = manager(16)?;
    let stacks = (0..8)
        .map(|_| manager.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    let mut bounds = stacks
        .iter()
        .map(|stack| stack.bounds())
        .collect::<Vec<_>>();
    bounds.sort_unstable();
    for pair in bounds.windows(2) {
        assert!(pair[0].2 <= pair[1].0);
    }
    let stable = stacks[0].bounds();
    assert_eq!(stacks[0].bounds(), stable);
    for (index, stack) in stacks.iter().enumerate() {
        let (_, usable_start, usable_end) = stack.bounds();
        assert_eq!(usable_end - usable_start, stack.usable_size());
        // SAFETY: The handle owns this accessible range for the duration of the write.
        unsafe { std::ptr::write_volatile(usable_start as *mut u8, index as u8) };
        // SAFETY: The same live handle keeps the initialized address accessible.
        let value = unsafe { std::ptr::read_volatile(usable_start as *const u8) };
        assert_eq!(value, index as u8);
    }
    Ok(())
}

#[test]
fn slots_are_reused_and_manager_outlives_its_handles() -> Result<(), Box<dyn Error>> {
    let manager = manager(2)?;
    let first = manager.allocate()?;
    let first_bounds = first.bounds();
    drop(first);
    let reused = manager.allocate()?;
    assert_eq!(reused.bounds(), first_bounds);
    let stats = manager.metrics()?;
    assert_eq!(stats.arenas, 1);
    assert_eq!(stats.reused_slots, 1);
    drop(manager);
    let (_, usable_start, _) = reused.bounds();
    // SAFETY: The handle retains the arena mapping after manager drop.
    unsafe { std::ptr::write_volatile(usable_start as *mut u8, 7) };
    drop(reused);
    Ok(())
}

#[test]
fn allocates_across_multiple_arenas_without_duplicate_slots() -> Result<(), Box<dyn Error>> {
    let manager = manager(2)?;
    let stacks = (0..5)
        .map(|_| manager.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    let addresses: BTreeSet<_> = stacks.iter().map(|stack| stack.bounds().0).collect();
    assert_eq!(addresses.len(), stacks.len());
    assert_eq!(manager.metrics()?.arenas, 3);
    Ok(())
}

#[test]
fn last_handle_drop_unmaps_its_arena() -> Result<(), Box<dyn Error>> {
    let manager = manager(1)?;
    let stack = manager.allocate()?;
    let weak = Arc::downgrade(&manager.inner);
    drop(manager);
    assert!(weak.upgrade().is_some());
    drop(stack);
    assert!(weak.upgrade().is_none());
    Ok(())
}
