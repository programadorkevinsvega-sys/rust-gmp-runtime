use crate::stack::{GoInspiredOptions, GoInspiredStackManager, StackConfig, StackError};
use std::error::Error;

#[test]
fn rejected_preflight_does_not_create_or_account_a_span() -> Result<(), Box<dyn Error>> {
    let config = StackConfig::new(8 * 1024)?.with_vma_safety_margin(u64::MAX);
    let manager = GoInspiredStackManager::with_options(config, GoInspiredOptions::default())?;
    assert!(matches!(
        manager.allocate(),
        Err(StackError::VmaPreflightRejected { .. })
    ));

    let metrics = manager.metrics()?;
    assert_eq!(metrics.spans_created, 0);
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.reserved_virtual_bytes, 0);
    assert_eq!(metrics.global_free_slots, 0);
    assert_eq!(metrics.allocation_failures, 1);
    Ok(())
}
