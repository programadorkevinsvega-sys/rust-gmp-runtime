use super::*;

#[test]
fn failed_span_release_restores_slots_and_counts_cleanup_failure() -> Result<(), Box<dyn Error>> {
    let manager = manager()?;
    let stack = manager.allocate()?;
    drop(stack);
    let before = manager.metrics()?;
    manager.fail_next_span_release_for_test()?;

    assert!(manager.release_empty_spans().is_err());
    let after_failure = manager.metrics()?;
    assert_eq!(
        after_failure.reserved_virtual_bytes,
        before.reserved_virtual_bytes
    );
    assert_eq!(after_failure.global_free_slots, before.global_free_slots);
    assert_eq!(after_failure.spans_released, before.spans_released);
    assert_eq!(after_failure.cleanup_failures, before.cleanup_failures + 1);

    assert_eq!(manager.release_empty_spans()?, 1);
    assert_eq!(manager.metrics()?.reserved_virtual_bytes, 0);
    Ok(())
}

#[test]
fn flush_all_caches_rejects_foreign_cache_without_flushing_it() -> Result<(), Box<dyn Error>> {
    let primary = manager()?;
    let foreign = manager()?;
    let mut cache = foreign.processor_cache()?;
    cache.refill()?;
    let before = foreign.metrics()?;
    {
        let mut caches = [&mut cache];
        assert!(primary.flush_all_caches(&mut caches).is_err());
    }
    assert_eq!(cache.len(), 1);
    assert_eq!(foreign.metrics()?.local_free_slots, before.local_free_slots);
    assert_eq!(primary.metrics()?.cleanup_failures, 1);
    Ok(())
}

#[test]
fn distinct_live_leases_never_overlap_across_workers() -> Result<(), Box<dyn Error>> {
    let manager = Arc::new(manager()?);
    let mut workers = Vec::new();
    for _ in 0..8 {
        let manager = Arc::clone(&manager);
        workers.push(thread::spawn(move || {
            let mut cache = manager.processor_cache().map_err(std::io::Error::other)?;
            let mut addresses = BTreeSet::new();
            let mut leases = Vec::new();
            for _ in 0..128 {
                let lease = manager
                    .allocate_cached(&mut cache)
                    .map_err(std::io::Error::other)?;
                let bounds = lease.bounds();
                if !addresses.insert(bounds.guard_start) {
                    return Err(std::io::Error::other("duplicate slot address"));
                }
                leases.push(lease);
            }
            for mut lease in leases {
                lease
                    .release_to_cache(&mut cache)
                    .map_err(std::io::Error::other)?;
            }
            Ok::<_, std::io::Error>(addresses)
        }));
    }
    let mut addresses = BTreeSet::new();
    for worker in workers {
        let worker_addresses = match worker.join() {
            Ok(result) => result?,
            Err(payload) => {
                return Err(std::io::Error::other(format!("worker panicked: {payload:?}")).into());
            }
        };
        addresses.extend(worker_addresses);
    }
    assert!(!addresses.is_empty());
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    Ok(())
}

#[test]
fn global_pool_keeps_live_leases_disjoint_across_workers() -> Result<(), Box<dyn Error>> {
    const WORKERS: usize = 8;
    const LEASES_PER_WORKER: usize = 8;
    let options = GoInspiredOptions::default()
        .with_slots_per_span(WORKERS * LEASES_PER_WORKER)
        .with_retained_spans(0);
    let manager = Arc::new(GoInspiredStackManager::with_options(
        StackConfig::new(8 * 1024)?,
        options,
    )?);
    let start = Arc::new(Barrier::new(WORKERS + 1));
    let mut workers = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let manager = Arc::clone(&manager);
        let start = Arc::clone(&start);
        workers.push(thread::spawn(move || {
            start.wait();
            let mut leases = Vec::with_capacity(LEASES_PER_WORKER);
            for _ in 0..LEASES_PER_WORKER {
                leases.push(manager.allocate().map_err(std::io::Error::other)?);
            }
            Ok::<_, std::io::Error>(leases)
        }));
    }

    start.wait();
    let expected_leases = WORKERS * LEASES_PER_WORKER;
    let expected_allocations = u64::try_from(expected_leases)?;
    let mut leases = Vec::with_capacity(expected_leases);
    for worker in workers {
        match worker.join() {
            Ok(result) => leases.extend(result?),
            Err(payload) => {
                return Err(std::io::Error::other(format!("worker panicked: {payload:?}")).into());
            }
        }
    }
    assert_eq!(leases.len(), expected_leases);
    let mut bounds = leases
        .iter()
        .map(|lease| lease.bounds())
        .collect::<Vec<_>>();
    bounds.sort_unstable_by_key(|stack| stack.guard_start);
    for adjacent in bounds.windows(2) {
        assert!(
            adjacent[0].usable_end <= adjacent[1].guard_start,
            "live stack bounds overlap: {:?} and {:?}",
            adjacent[0],
            adjacent[1]
        );
    }

    drop(leases);
    let metrics = manager.metrics()?;
    assert_eq!(metrics.allocations, expected_allocations);
    assert_eq!(metrics.releases, expected_allocations);
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.local_free_slots, 0);
    assert_eq!(metrics.allocation_failures, 0);
    assert_eq!(metrics.cleanup_failures, 0);
    Ok(())
}

#[test]
fn global_drop_fallback_reuses_a_stable_slot() -> Result<(), Box<dyn Error>> {
    let manager = manager()?;
    let first = manager.allocate()?;
    let address = first.bounds().guard_start;
    drop(first);
    let next = manager.allocate()?;
    assert_eq!(next.bounds().guard_start, address);
    assert_eq!(
        next.reserved_size(),
        next.usable_size() + manager.page_size()
    );
    Ok(())
}

#[test]
#[cfg(feature = "allocator-benchmark")]
fn benchmark_timing_covers_mapping_activation_pool_and_trim() -> Result<(), Box<dyn Error>> {
    let options = GoInspiredOptions::default()
        .with_slots_per_span(8)
        .with_retained_spans(0);
    let manager = GoInspiredStackManager::new_benchmarked(StackConfig::new(8 * 1024)?, options)?;
    let mut cache = manager.processor_cache()?;
    let mut lease = manager.allocate_cached(&mut cache)?;
    lease.release_to_cache(&mut cache)?;
    cache.clear()?;
    assert_eq!(manager.trim()?, 1);

    let timing = manager.take_timing_snapshot();
    assert_eq!(timing.mmap_calls, 1);
    assert_eq!(timing.mprotect_calls, 1);
    assert_eq!(timing.munmap_calls, 1);
    assert!(timing.lock_hold_ns > 0);
    let reset = manager.take_timing_snapshot();
    assert_eq!(reset.mmap_calls, 0);
    assert_eq!(reset.mprotect_calls, 0);
    assert_eq!(reset.munmap_calls, 0);
    Ok(())
}
