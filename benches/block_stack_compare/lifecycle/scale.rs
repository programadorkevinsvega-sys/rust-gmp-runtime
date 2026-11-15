use super::*;

pub(super) fn run_scale(
    strategy: &str,
    count: usize,
    samples: usize,
    stack_bytes: usize,
    slots_per_arena: usize,
) -> Result<(), Box<dyn Error>> {
    for sample in 0..samples {
        let config = StackConfig::new(stack_bytes)?;
        let allocator = Allocator::create(strategy, config, slots_per_arena, false, false)?;
        let before = allocator.stats()?;
        if !supported_capacity(allocator.vma_snapshot()?, count)? {
            println!(
                "scale_skipped strategy={strategy} stacks={count} sample={sample} vmas_before={} reason=preflight_headroom",
                before.vmas
            );
            break;
        }
        let started = Instant::now();
        let mut stacks = Vec::with_capacity(count);
        for _ in 0..count {
            stacks.push(allocator.allocate()?);
        }
        let allocate_ns = started.elapsed().as_nanos();
        let touch_started = Instant::now();
        let touched = stacks
            .iter()
            .step_by(100)
            .inspect(|stack| stack.touch())
            .count();
        let touch_ns = touch_started.elapsed().as_nanos();
        let during = allocator.stats()?;
        let release_started = Instant::now();
        drop(stacks);
        let release_ns = release_started.elapsed().as_nanos();
        let after = allocator.stats()?;
        let recovery = if strategy == "go-inspired" {
            recover_empty_spans(&allocator)?.to_string()
        } else {
            "not_applicable".to_owned()
        };
        let manager_drop_started = Instant::now();
        drop(allocator);
        let manager_drop_ns = manager_drop_started.elapsed().as_nanos();
        let manager_drop_vmas = current_vma_count()?;
        let throughput = count as f64 / (allocate_ns as f64 / 1_000_000_000.0);
        println!(
            "scale strategy={strategy} stacks={count} sample={sample} instrumentation=off allocate_ns={allocate_ns} allocate_ops_s={throughput:.2} touched_slots={touched} touch_ns={touch_ns} release_ns={release_ns} manager_drop_ns={manager_drop_ns} manager_drop_vmas={manager_drop_vmas} before={before} during={during} after_release={after} recovery={recovery}"
        );
    }
    Ok(())
}
pub(super) fn recover_empty_spans(allocator: &Allocator) -> Result<SpanRecovery, Box<dyn Error>> {
    let mut caches = (0..RECOVERY_CACHE_COUNT)
        .map(|_| allocator.worker_cache())
        .collect::<Result<Vec<_>, _>>()?;
    for cache in &mut caches {
        let stack = allocator.allocate_for_worker(cache)?;
        allocator.release_for_worker(stack, cache)?;
    }
    let before = allocator.stats()?;
    let started = Instant::now();
    let released_spans = allocator.flush_all_caches_and_release_empty_spans(&mut caches)?;
    let elapsed_ns = started.elapsed().as_nanos();
    let after = allocator.stats()?;
    if after.active != 0
        || after.free_slots != 0
        || after.reserved_bytes != 0
        || after.accessible_bytes != 0
        || after.allocation_failures != before.allocation_failures
        || after.cleanup_failures != before.cleanup_failures
    {
        return Err(format!(
            "span recovery did not fully restore allocator accounting: before={before} after={after}"
        )
        .into());
    }
    Ok(SpanRecovery {
        cache_count: RECOVERY_CACHE_COUNT,
        local_slots_before_flush: before.local_free_slots,
        released_spans,
        elapsed_ns,
        before,
        after,
    })
}

#[derive(Debug)]
pub(super) struct SpanRecovery {
    cache_count: usize,
    local_slots_before_flush: u64,
    released_spans: usize,
    elapsed_ns: u128,
    before: crate::common::Stats,
    after: crate::common::Stats,
}
impl std::fmt::Display for SpanRecovery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cache_count={} local_slots_before_flush={} released_spans={} elapsed_ns={} before=[{}] after=[{}]",
            self.cache_count,
            self.local_slots_before_flush,
            self.released_spans,
            self.elapsed_ns,
            self.before,
            self.after,
        )
    }
}
