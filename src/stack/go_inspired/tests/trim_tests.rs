use super::*;

#[test]
fn active_or_cached_slots_prevent_trim_and_clear_allows_it() -> Result<(), Box<dyn Error>> {
    let manager = manager()?;
    let mut cache = manager.processor_cache()?;
    let mut lease = manager.allocate_cached(&mut cache)?;
    assert_eq!(manager.trim()?, 0);
    lease.release_to_cache(&mut cache)?;
    assert_eq!(manager.trim()?, 0);
    cache.clear()?;
    assert_eq!(manager.trim()?, 1);
    let metrics = manager.metrics()?;
    assert_eq!(metrics.reserved_virtual_bytes, 0);
    assert_eq!(metrics.global_free_slots, 0);
    assert_eq!(metrics.spans_released, 1);
    Ok(())
}

#[test]
fn all_cache_flush_and_release_empty_spans_reclaims_virtual_reservations()
-> Result<(), Box<dyn Error>> {
    let manager = manager()?;
    let leases = (0..17)
        .map(|_| manager.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    drop(leases);
    let mut first = manager.processor_cache()?;
    let mut second = manager.processor_cache()?;
    first.refill()?;
    second.refill()?;

    let before = manager.metrics()?;
    assert_eq!(before.spans_created, 3);
    assert!(before.local_free_slots > 0);
    let before_vmas = manager.vma_snapshot()?.current;
    let released = {
        let mut caches = [&mut first, &mut second];
        manager.flush_all_caches_and_release_empty_spans(&mut caches)?
    };
    assert_eq!(released, 3);

    let after = manager.metrics()?;
    assert_eq!(after.active_stacks, 0);
    assert_eq!(after.local_free_slots, 0);
    assert_eq!(after.global_free_slots, 0);
    assert_eq!(after.reserved_virtual_bytes, 0);
    assert_eq!(after.accessible_virtual_bytes, 0);
    assert_eq!(after.spans_released, 3);
    assert!(manager.vma_snapshot()?.current < before_vmas);
    Ok(())
}

#[test]
fn trim_retains_configured_warm_span_but_explicit_release_does_not() -> Result<(), Box<dyn Error>> {
    let manager = GoInspiredStackManager::with_options(
        StackConfig::new(8 * 1024)?,
        GoInspiredOptions::default()
            .with_slots_per_span(8)
            .with_retained_spans(1),
    )?;
    let leases = (0..17)
        .map(|_| manager.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    drop(leases);

    assert_eq!(manager.trim()?, 2);
    assert_eq!(manager.metrics()?.reserved_virtual_bytes, 12 * 1024 * 8);
    assert_eq!(manager.release_empty_spans()?, 1);
    assert_eq!(manager.metrics()?.reserved_virtual_bytes, 0);
    Ok(())
}

#[test]
fn retention_target_can_be_adjusted_for_memory_pressure() -> Result<(), Box<dyn Error>> {
    let manager = GoInspiredStackManager::with_options(
        StackConfig::new(8 * 1024)?,
        GoInspiredOptions::default()
            .with_slots_per_span(8)
            .with_retained_spans(4),
    )?;
    let leases = (0..25)
        .map(|_| manager.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    drop(leases);

    assert_eq!(manager.trim()?, 0);
    assert_eq!(manager.metrics()?.spans_created, 4);
    assert_eq!(manager.trim_to_retained_spans(1)?, 3);
    assert_eq!(manager.metrics()?.reserved_virtual_bytes, 12 * 1024 * 8);
    assert_eq!(manager.trim_to_retained_spans(0)?, 1);
    assert_eq!(manager.metrics()?.reserved_virtual_bytes, 0);
    Ok(())
}
