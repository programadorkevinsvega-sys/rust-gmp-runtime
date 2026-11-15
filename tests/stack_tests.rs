//! Stack integration tests.

use std::error::Error;

use rust_gmp_runtime::stack::{StackConfig, StackEngine, StackError, StackManager};

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
fn go_inspired_preflight_rejections_are_counted_without_faking_allocations()
-> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(
        StackConfig::new(4096)?
            .with_vma_safety_margin(u64::MAX)
            .with_engine(StackEngine::GoInspired),
    )?;
    assert!(matches!(
        manager.preflight_additional_stacks(1),
        Err(StackError::VmaPreflightRejected { .. })
    ));
    let preflight = manager.metrics()?;
    assert_eq!(preflight.vma_preflight_rejections, 1);
    assert_eq!(preflight.failed_allocations, 0);

    assert!(matches!(
        manager.allocate(),
        Err(StackError::VmaPreflightRejected { .. })
    ));
    let allocation = manager.metrics()?;
    assert_eq!(allocation.vma_preflight_rejections, 2);
    assert_eq!(allocation.failed_allocations, 1);
    assert_eq!(allocation.active_stacks, 0);
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

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn go_inspired_engine_uses_the_production_stack_manager_contract() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(8192)?.with_engine(StackEngine::GoInspired))?;
    assert_eq!(manager.engine(), StackEngine::GoInspired);
    let stack = manager.allocate()?;
    let bounds = stack.bounds();
    assert_eq!(bounds.guard_end, bounds.usable_start);
    assert_eq!(stack.usable_size(), manager.usable_size());

    let active = manager.metrics()?;
    assert_eq!(active.engine, StackEngine::GoInspired);
    assert_eq!(active.active_stacks, 1);
    assert_eq!(
        active
            .go_inspired
            .as_ref()
            .map(|details| details.allocations),
        Some(1)
    );
    drop(stack);

    let released = manager.metrics()?;
    assert_eq!(released.active_stacks, 0);
    assert_eq!(released.free_stacks, 64);
    assert!(manager.trim_to_retained_spans(0)? > 0);
    assert_eq!(manager.metrics()?.reserved_virtual_bytes, 0);
    Ok(())
}

#[test]
fn fixed_engine_remains_the_default_and_option_mismatch_is_explicit() -> Result<(), Box<dyn Error>>
{
    let default = StackManager::new(StackConfig::new(4096)?)?;
    assert_eq!(default.engine(), StackEngine::Fixed);
    assert_eq!(default.metrics()?.engine, StackEngine::Fixed);

    let error = StackManager::with_go_inspired_options(
        StackConfig::new(4096)?,
        rust_gmp_runtime::stack::GoInspiredOptions::default(),
    );
    assert!(matches!(
        error,
        Err(StackError::StackEngineMismatch {
            expected: StackEngine::GoInspired,
            selected: StackEngine::Fixed,
        })
    ));
    Ok(())
}
