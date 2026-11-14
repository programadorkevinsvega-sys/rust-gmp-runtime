//! Stack integration tests.

use std::error::Error;

use rust_gmp_runtime::stack::{StackConfig, StackError, StackManager};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn failed_mapping_is_reported_and_counted() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(1_usize << 60)?)?;
    let error = match manager.allocate() {
        Ok(stack) => {
            drop(stack);
            return Err("an impossible-sized mapping unexpectedly succeeded".into());
        }
        Err(error) => error,
    };

    assert!(matches!(
        error,
        StackError::SystemCall {
            operation: "mmap",
            ..
        }
    ));
    let metrics = manager.metrics()?;
    assert_eq!(metrics.failed_allocations, 1);
    assert_eq!(metrics.allocation_failures, 1);
    assert_eq!(metrics.vma_preflight_rejections, 0);
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.active_mappings, 0);
    Ok(())
}

#[test]
fn allocations_have_disjoint_stable_bounds_and_release_accounting() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(8192)?)?;
    let first = manager.allocate()?;
    let original_bounds = first.bounds();
    let second = manager.allocate()?;
    let second_bounds = second.bounds();

    assert_eq!(first.bounds(), original_bounds);
    assert!(
        original_bounds.usable_end <= second_bounds.guard_start
            || second_bounds.usable_end <= original_bounds.guard_start
    );
    assert_eq!(manager.metrics()?.active_stacks, 2);

    drop(second);
    assert_eq!(manager.metrics()?.active_stacks, 1);
    assert_eq!(first.bounds(), original_bounds);
    let replacement = manager.allocate()?;
    let replacement_bounds = replacement.bounds();
    assert_eq!(first.bounds(), original_bounds);
    assert!(
        original_bounds.usable_end <= replacement_bounds.guard_start
            || replacement_bounds.usable_end <= original_bounds.guard_start
    );
    drop(replacement);
    assert_eq!(first.bounds(), original_bounds);
    drop(first);
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.released_stacks, 3);
    Ok(())
}

#[test]
fn stack_capacity_is_page_aligned_and_separates_reserved_from_accessible()
-> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(1)?)?;
    let stack = manager.allocate()?;
    let metrics = manager.metrics()?;

    assert_eq!(stack.usable_size(), manager.page_size());
    assert_eq!(
        metrics.reserved_virtual_bytes,
        (manager.page_size() * 2) as u64
    );
    assert_eq!(metrics.accessible_virtual_bytes, manager.page_size() as u64);
    assert!(metrics.process_resident_bytes.is_some());
    Ok(())
}
