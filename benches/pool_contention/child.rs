use super::metrics::{
    metric_delta, validate_memory_geometry, validate_operation_counts, validate_quiescent,
    validate_trimmed,
};
use super::worker::{new_manager, run_worker, warm_global_pool, warm_worker_caches};
use super::*;

pub(super) fn run_child(args: &[String]) -> Result<(), Box<dyn Error>> {
    let [_, path, workers, sample] = args else {
        return Err("isolated run requires path, workers, and sample".into());
    };
    let path = PoolPath::parse(path)?;
    let workers = workers.parse::<usize>()?;
    if !WORKER_COUNTS.contains(&workers) {
        return Err("unsupported worker count".into());
    }
    let sample = sample.parse::<usize>()?;

    let config = StackConfig::new(STACK_SIZE)?;
    let options = GoInspiredOptions::default()
        .with_slots_per_span(SLOTS_PER_SPAN)
        .with_cache_bytes(CACHE_BYTES);
    let manager = new_manager(config, options)?;
    let initial = manager.metrics()?;

    let caches = match path {
        PoolPath::Global => {
            warm_global_pool(&manager)?;
            Vec::new()
        }
        PoolPath::WorkerCache => warm_worker_caches(&manager, workers)?,
    };
    let setup = manager.metrics()?;
    let setup_delta = metric_delta(&initial, &setup)?;
    #[cfg(feature = "allocator-benchmark")]
    let setup_timing = manager.take_timing_snapshot();

    let start = Arc::new(Barrier::new(workers + 1));
    let mut threads = Vec::with_capacity(workers);
    let mut cache_slots = caches.into_iter();
    for _ in 0..workers {
        let manager = manager.clone();
        let start = Arc::clone(&start);
        let cache = if path == PoolPath::WorkerCache {
            Some(
                cache_slots
                    .next()
                    .flatten()
                    .ok_or("worker cache missing before thread start")?,
            )
        } else {
            None
        };
        threads.push(thread::spawn(move || {
            run_worker(&manager, path, OPERATIONS_PER_WORKER, cache, start)
        }));
    }

    let before = manager.metrics()?;
    #[cfg(feature = "allocator-benchmark")]
    let thread_start_timing = manager.take_timing_snapshot();
    let started = Instant::now();
    start.wait();
    let mut results = Vec::with_capacity(workers);
    for thread in threads {
        let result = thread
            .join()
            .map_err(|payload| io::Error::other(format!("worker panicked: {payload:?}")))??;
        results.push(result);
    }
    let wall_ns = started.elapsed().as_nanos();
    let after_operations = manager.metrics()?;
    let operation_delta = metric_delta(&before, &after_operations)?;
    validate_memory_geometry(
        &after_operations,
        manager.usable_size(),
        manager.page_size(),
    )?;
    #[cfg(feature = "allocator-benchmark")]
    let operation_timing = manager.take_timing_snapshot();

    let mut acquisition_ns = 0_u128;
    let mut return_ns = 0_u128;
    let mut completed_operations = 0_u64;
    let flush_started = Instant::now();
    for mut result in results {
        acquisition_ns = acquisition_ns.saturating_add(result.acquisition_ns);
        return_ns = return_ns.saturating_add(result.return_ns);
        completed_operations = completed_operations
            .checked_add(result.operations)
            .ok_or("worker operation count overflow")?;
        if let Some(cache) = result.cache.as_mut() {
            cache.clear()?;
        }
    }
    let cache_flush_ns = flush_started.elapsed().as_nanos();
    let after_flush = manager.metrics()?;
    #[cfg(feature = "allocator-benchmark")]
    let flush_timing = manager.take_timing_snapshot();

    validate_quiescent(&after_flush)?;
    let trim_started = Instant::now();
    let trimmed_spans = manager.trim_to_retained_spans(0)?;
    let trim_ns = trim_started.elapsed().as_nanos();
    let final_metrics = manager.metrics()?;
    validate_trimmed(&final_metrics)?;

    let operation_count = u64::try_from(workers)?
        .checked_mul(u64::try_from(OPERATIONS_PER_WORKER)?)
        .ok_or("operation count overflow")?;
    if completed_operations != operation_count {
        return Err(format!(
            "worker completion count mismatch: expected={operation_count} actual={completed_operations}"
        )
        .into());
    }
    validate_operation_counts(path, operation_count, &operation_delta)?;
    let throughput = operation_count as f64 / (wall_ns as f64 / 1_000_000_000.0);

    #[cfg(feature = "allocator-benchmark")]
    let (setup_lock_wait, setup_lock_hold) = (setup_timing.lock_wait_ns, setup_timing.lock_hold_ns);
    #[cfg(not(feature = "allocator-benchmark"))]
    let (setup_lock_wait, setup_lock_hold) = (0, 0);
    #[cfg(feature = "allocator-benchmark")]
    let (thread_start_lock_wait, thread_start_lock_hold) = (
        thread_start_timing.lock_wait_ns,
        thread_start_timing.lock_hold_ns,
    );
    #[cfg(not(feature = "allocator-benchmark"))]
    let (thread_start_lock_wait, thread_start_lock_hold) = (0, 0);
    #[cfg(feature = "allocator-benchmark")]
    let (operation_lock_wait, operation_lock_hold) =
        (operation_timing.lock_wait_ns, operation_timing.lock_hold_ns);
    #[cfg(not(feature = "allocator-benchmark"))]
    let (operation_lock_wait, operation_lock_hold) = (0, 0);
    #[cfg(feature = "allocator-benchmark")]
    let (flush_lock_wait, flush_lock_hold) = (flush_timing.lock_wait_ns, flush_timing.lock_hold_ns);
    #[cfg(not(feature = "allocator-benchmark"))]
    let (flush_lock_wait, flush_lock_hold) = (0, 0);
    println!(
        "pool_sample path={} workers={workers} sample={sample} instrumented={} operations={operation_count} completed_operations={completed_operations} elapsed_ns={wall_ns} ops_s={throughput:.2} acquisition_sum_ns={acquisition_ns} lease_return_sum_ns={return_ns} cache_flush_wall_ns={cache_flush_ns} setup_cache_hits={} setup_cache_misses={} setup_refill_requests={} setup_refills={} setup_refill_slots={} setup_lock_wait_ns={setup_lock_wait} setup_lock_hold_ns={setup_lock_hold} thread_start_lock_wait_ns={} thread_start_lock_hold_ns={} operation_cache_hits={} operation_cache_misses={} operation_refill_requests={} operation_refills={} operation_refill_slots={} operation_lock_wait_ns={operation_lock_wait} operation_lock_hold_ns={operation_lock_hold} flush_lock_wait_ns={flush_lock_wait} flush_lock_hold_ns={flush_lock_hold} active_after_ops={} free_after_ops={} reserved_after_ops={} accessible_after_ops={} vmas_after_ops={} reserved_after_flush={} local_free_after_flush={} global_free_after_flush={} trim_spans={trimmed_spans} trim_ns={trim_ns} reserved_final={} accessible_final={} active_final={} vmas_final={} failures={}",
        path.name(),
        cfg!(feature = "allocator-benchmark"),
        setup_delta.cache_hits,
        setup_delta.cache_misses,
        setup_delta.cache_refill_requests,
        setup_delta.cache_refills,
        setup_delta.cache_refill_slots,
        thread_start_lock_wait,
        thread_start_lock_hold,
        operation_delta.cache_hits,
        operation_delta.cache_misses,
        operation_delta.cache_refill_requests,
        operation_delta.cache_refills,
        operation_delta.cache_refill_slots,
        after_operations.active_stacks,
        after_operations.global_free_slots,
        after_operations.reserved_virtual_bytes,
        after_operations.accessible_virtual_bytes,
        after_operations
            .vma_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.current),
        after_flush.reserved_virtual_bytes,
        after_flush.local_free_slots,
        after_flush.global_free_slots,
        final_metrics.reserved_virtual_bytes,
        final_metrics.accessible_virtual_bytes,
        final_metrics.active_stacks,
        final_metrics
            .vma_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.current),
        final_metrics.allocation_failures + final_metrics.cleanup_failures,
    );
    Ok(())
}
