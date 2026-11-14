use super::*;

pub(super) fn run_retention_policies(
    stack_bytes: usize,
    slots_per_span: usize,
) -> Result<(), Box<dyn Error>> {
    let stack_count = env_usize("RUST_GMP_TOURNAMENT_RETENTION_STACKS", 100_000)?;
    let policies = env_usize_list("RUST_GMP_TOURNAMENT_RETAIN_SPANS", &[0, 1, 4, 16])?;
    if stack_count == 0 || policies.is_empty() {
        return Err("retention stack count and policy list must be nonzero".into());
    }

    for retained_spans in policies {
        let options = GoInspiredOptions::default()
            .with_slots_per_span(slots_per_span)
            .with_retained_spans(retained_spans);
        let manager =
            GoInspiredStackManager::with_options(StackConfig::new(stack_bytes)?, options)?;
        let before_snapshot = manager.vma_snapshot()?;
        if !supported_capacity(before_snapshot, stack_count)? {
            println!(
                "retention_policy_skipped stacks={stack_count} retained_spans={retained_spans} vmas_before={} reason=preflight_headroom",
                before_snapshot.current
            );
            continue;
        }
        let leases = (0..stack_count)
            .map(|_| manager.allocate())
            .collect::<Result<Vec<_>, _>>()?;
        drop(leases);

        let mut caches = (0..RECOVERY_CACHE_COUNT)
            .map(|_| manager.processor_cache())
            .collect::<Result<Vec<_>, _>>()?;
        for cache in &mut caches {
            let mut lease = manager.allocate_cached(cache)?;
            lease.release_to_cache(cache)?;
        }

        let before_trim = manager.metrics()?;
        let started = Instant::now();
        let flushed_slots = {
            let mut cache_refs = caches.iter_mut().collect::<Vec<_>>();
            manager.flush_all_caches(&mut cache_refs)?
        };
        let trimmed_spans = manager.trim()?;
        let trim_elapsed_ns = started.elapsed().as_nanos();
        let after_trim = manager.metrics()?;
        let expected_spans = u64::try_from(retained_spans)?
            .min(before_trim.spans_created - before_trim.spans_released);
        validate_retention_metrics(
            &after_trim,
            expected_spans,
            slots_per_span,
            stack_bytes,
            manager.page_size(),
        )?;
        let pressure_trim_started = Instant::now();
        let pressure_trimmed_spans = manager.trim_to_retained_spans(1)?;
        let pressure_trim_elapsed_ns = pressure_trim_started.elapsed().as_nanos();
        let after_pressure = manager.metrics()?;
        let pressure_retained_spans = 1_u64.min(expected_spans);
        validate_retention_metrics(
            &after_pressure,
            pressure_retained_spans,
            slots_per_span,
            stack_bytes,
            manager.page_size(),
        )?;
        let released_remaining = manager.release_empty_spans()?;
        let after_release = manager.metrics()?;
        if after_release.reserved_virtual_bytes != 0
            || after_release.global_free_slots != 0
            || after_release.active_stacks != 0
        {
            return Err(format!(
                "explicit release failed after retention policy {retained_spans}: {after_release:?}"
            )
            .into());
        }
        println!(
            "retention_policy stacks={stack_count} configured_retained_spans={retained_spans} flushed_slots={flushed_slots} trimmed_spans={trimmed_spans} trim_elapsed_ns={trim_elapsed_ns} retained_after_trim={} vmas_after_trim={} reserved_after_trim={} rss_after_trim={} pressure_target_spans=1 pressure_trimmed_spans={pressure_trimmed_spans} pressure_trim_elapsed_ns={pressure_trim_elapsed_ns} retained_after_pressure={} reserved_after_pressure={} rss_after_pressure={} explicitly_released_spans={released_remaining} vmas_after_full_release={} reserved_after_full_release={} rss_after_full_release={}",
            after_trim.spans_created - after_trim.spans_released,
            before_trim
                .vma_snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.current),
            after_trim.reserved_virtual_bytes,
            process_rss_bytes(manager.page_size())?,
            after_pressure.spans_created - after_pressure.spans_released,
            after_pressure.reserved_virtual_bytes,
            process_rss_bytes(manager.page_size())?,
            after_release
                .vma_snapshot
                .as_ref()
                .map_or(0, |snapshot| snapshot.current),
            after_release.reserved_virtual_bytes,
            process_rss_bytes(manager.page_size())?,
        );
    }
    Ok(())
}
pub(super) fn validate_retention_metrics(
    metrics: &rust_gmp_runtime::stack::GoInspiredStackMetrics,
    expected_spans: u64,
    slots_per_span: usize,
    stack_bytes: usize,
    page_size: usize,
) -> Result<(), Box<dyn Error>> {
    let expected_slots = expected_spans
        .checked_mul(u64::try_from(slots_per_span)?)
        .ok_or("expected retained slots overflow")?;
    let stride = stack_bytes
        .checked_add(page_size)
        .ok_or("stack stride overflow")?;
    let expected_bytes = expected_slots
        .checked_mul(u64::try_from(stride)?)
        .ok_or("expected retained bytes overflow")?;
    if metrics.active_stacks != 0
        || metrics.local_free_slots != 0
        || metrics.global_free_slots != expected_slots
        || metrics.reserved_virtual_bytes != expected_bytes
    {
        return Err(format!(
            "retention accounting mismatch for {expected_spans} spans: {metrics:?}"
        )
        .into());
    }
    Ok(())
}
pub(super) fn process_rss_bytes(page_size: usize) -> Result<u64, Box<dyn Error>> {
    let statm = fs::read_to_string("/proc/self/statm")?;
    let resident_pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .ok_or("statm resident page count missing")?
        .parse()?;
    resident_pages
        .checked_mul(u64::try_from(page_size)?)
        .ok_or_else(|| "RSS byte count overflow".into())
}
