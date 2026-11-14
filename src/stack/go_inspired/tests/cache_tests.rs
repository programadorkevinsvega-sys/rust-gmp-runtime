use super::*;

#[test]
fn local_cache_refills_reuses_and_clears_slots() -> Result<(), Box<dyn Error>> {
    let manager = manager()?;
    let mut cache = manager.processor_cache()?;
    assert_eq!(manager.class_count(), 1);
    assert_eq!(cache.refill()?, 1);
    let mut first = manager.allocate_cached(&mut cache)?;
    let address = first.bounds().guard_start;
    first.release_to_cache(&mut cache)?;
    assert_eq!(cache.len(), 1);

    let reused = manager.allocate_cached(&mut cache)?;
    assert_eq!(reused.bounds().guard_start, address);
    drop(reused);
    cache.clear()?;
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.local_free_slots, 0);
    assert!(metrics.global_free_slots > 0);
    assert!(metrics.reused_slots > 0);
    assert_eq!(metrics.cache_refill_requests, 1);
    assert_eq!(metrics.cache_refills, 1);
    assert_eq!(metrics.cache_refill_slots, 1);
    assert_eq!(metrics.cache_refill_max_batch, 1);
    assert_eq!(metrics.cache_hits, 2);
    assert_eq!(metrics.cache_misses, 0);
    assert_eq!(metrics.allocation_failures, 0);
    assert_eq!(metrics.cleanup_failures, 0);
    Ok(())
}

#[test]
fn local_cache_refill_target_is_bounded_and_only_affects_future_refills()
-> Result<(), Box<dyn Error>> {
    let manager = GoInspiredStackManager::with_options(
        StackConfig::new(8 * 1024)?,
        GoInspiredOptions::default()
            .with_cache_bytes(64 * 1024)
            .with_slots_per_span(8),
    )?;
    let mut cache = manager.processor_cache()?;
    assert_eq!(cache.refill_target(), 2);
    assert_eq!(cache.refill()?, 2);
    cache.set_refill_target(1)?;
    assert_eq!(cache.refill_target(), 1);
    assert_eq!(cache.len(), 2);
    cache.clear()?;
    assert_eq!(cache.refill()?, 1);
    assert_eq!(cache.len(), 1);
    assert!(matches!(
        cache.set_refill_target(0),
        Err(StackError::InvalidCacheRefillTarget {
            requested_slots: 0,
            capacity_slots: 5,
        })
    ));
    assert!(matches!(
        cache.set_refill_target(6),
        Err(StackError::InvalidCacheRefillTarget {
            requested_slots: 6,
            capacity_slots: 5,
        })
    ));
    assert_eq!(manager.cache_refill_target(), 2);
    let metrics = manager.metrics()?;
    assert_eq!(metrics.cache_refill_slots, 3);
    assert_eq!(metrics.allocation_failures, 0);
    assert_eq!(metrics.cleanup_failures, 0);
    Ok(())
}

#[test]
fn resetting_refill_peak_preserves_cumulative_counters() -> Result<(), Box<dyn Error>> {
    let manager = manager()?;
    let mut cache = manager.processor_cache()?;
    cache.refill()?;
    let before = manager.metrics()?;
    assert_eq!(before.max_concurrent_refills, 1);
    manager.reset_refill_peak()?;
    let after = manager.metrics()?;
    assert_eq!(after.max_concurrent_refills, 0);
    assert_eq!(after.cache_refill_requests, before.cache_refill_requests);
    assert_eq!(after.cache_refills, before.cache_refills);
    Ok(())
}

#[test]
fn leases_can_migrate_between_explicit_caches() -> Result<(), Box<dyn Error>> {
    let manager = manager()?;
    let mut origin = manager.processor_cache()?;
    let mut current = manager.processor_cache()?;
    let mut lease = manager.allocate_cached(&mut origin)?;
    let start = lease.bounds().usable_start;
    lease.release_to_cache(&mut current)?;
    assert_eq!(origin.len(), 0);
    assert_eq!(current.len(), 1);
    let migrated = manager.allocate_cached(&mut current)?;
    assert_eq!(migrated.bounds().usable_start, start);
    drop(migrated);
    Ok(())
}
