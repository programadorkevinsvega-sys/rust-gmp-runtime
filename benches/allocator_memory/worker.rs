#![allow(unsafe_code)]

use rust_gmp_runtime::stack::{
    GoInspiredOptions, GoInspiredStack, GoInspiredStackManager, ProcessorStackCache, StackBounds,
    StackConfig,
};
use std::error::Error;

pub const SLOTS_PER_SPAN: usize = 256;
pub const CACHE_COUNT: usize = 8;
pub const STACK_BYTES: usize = 8 * 1024;
type RssMeasurements = (u64, u64, u64, u64, usize, usize);

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let [scenario, retained, count, sample] = args else {
        return Err("worker requires scenario, retained spans, count, and sample".into());
    };
    let retained_spans = retained.parse::<usize>()?;
    let stack_count = count.parse::<usize>()?;
    let sample = sample.parse::<usize>()?;
    let manager = GoInspiredStackManager::with_options(
        StackConfig::new(STACK_BYTES)?,
        GoInspiredOptions::default()
            .with_slots_per_span(SLOTS_PER_SPAN)
            .with_retained_spans(retained_spans),
    )?;
    check_vma_headroom(&manager, stack_count)?;

    let baseline_rss = process_rss_bytes(manager.page_size())?;
    let (active_rss, released_rss, flushed_rss, trimmed_rss, flushed_slots, trimmed_spans) =
        match scenario.as_str() {
            "global-pool" => global_pool_sample(&manager, stack_count, retained_spans)?,
            "processor-cache" => processor_cache_sample(&manager, stack_count, retained_spans)?,
            _ => return Err(format!("unknown memory scenario: {scenario}").into()),
        };
    let span_count = stack_count.div_ceil(SLOTS_PER_SPAN);
    let expected_trimmed = span_count.saturating_sub(retained_spans.min(span_count));
    if trimmed_spans != expected_trimmed {
        return Err(format!(
            "unexpected trim count: expected={expected_trimmed} actual={trimmed_spans}"
        )
        .into());
    }

    let released_spans = manager.release_empty_spans()?;
    let fully_released_rss = process_rss_bytes(manager.page_size())?;
    let final_metrics = manager.metrics()?;
    validate_final_state(&final_metrics, released_spans, retained_spans, span_count)?;
    let stride = STACK_BYTES
        .checked_add(manager.page_size())
        .ok_or("stack stride overflow")?;
    let reserved_before_trim = u64::try_from(span_count)?
        .checked_mul(u64::try_from(SLOTS_PER_SPAN)?)
        .and_then(|slots| slots.checked_mul(u64::try_from(stride).ok()?))
        .ok_or("reserved byte calculation overflow")?;
    let reserved_after_trim = u64::try_from(retained_spans.min(span_count))?
        .checked_mul(u64::try_from(SLOTS_PER_SPAN)?)
        .and_then(|slots| slots.checked_mul(u64::try_from(stride).ok()?))
        .ok_or("retained byte calculation overflow")?;
    println!(
        "memory_sample scenario={scenario} retained_target={retained_spans} sample={sample} stacks={stack_count} spans_created={span_count} baseline_rss={baseline_rss} active_rss={active_rss} free_rss={released_rss} flushed_rss={flushed_rss} trimmed_rss={trimmed_rss} fully_released_rss={fully_released_rss} rss_active_delta={} rss_free_delta={} rss_trimmed_delta={} rss_fully_released_delta={} reserved_before_trim={reserved_before_trim} reserved_after_trim={} flushed_slots={flushed_slots} trimmed_spans={trimmed_spans} released_spans={released_spans} vmas_after_release={}",
        active_rss.saturating_sub(baseline_rss),
        released_rss.saturating_sub(baseline_rss),
        trimmed_rss.saturating_sub(baseline_rss),
        fully_released_rss.saturating_sub(baseline_rss),
        reserved_after_trim,
        final_metrics
            .vma_snapshot
            .map_or(0, |snapshot| snapshot.current)
    );
    Ok(())
}

fn global_pool_sample(
    manager: &GoInspiredStackManager,
    count: usize,
    retained: usize,
) -> Result<RssMeasurements, Box<dyn Error>> {
    let stacks = allocate_and_touch(manager, count)?;
    let active = process_rss_bytes(manager.page_size())?;
    drop(stacks);
    let free = process_rss_bytes(manager.page_size())?;
    let trimmed = manager.trim_to_retained_spans(retained)?;
    let rss_trimmed = process_rss_bytes(manager.page_size())?;
    Ok((active, free, free, rss_trimmed, 0, trimmed))
}

fn processor_cache_sample(
    manager: &GoInspiredStackManager,
    count: usize,
    retained: usize,
) -> Result<RssMeasurements, Box<dyn Error>> {
    let mut caches = (0..CACHE_COUNT)
        .map(|_| manager.processor_cache())
        .collect::<Result<Vec<_>, _>>()?;
    let mut stacks = allocate_cached_and_touch(manager, &mut caches, count)?;
    let active = process_rss_bytes(manager.page_size())?;
    for (mut stack, cache_index) in stacks.drain(..) {
        stack.release_to_cache(&mut caches[cache_index])?;
    }
    let free = process_rss_bytes(manager.page_size())?;
    let mut cache_refs = caches.iter_mut().collect::<Vec<_>>();
    let flushed = manager.flush_all_caches(&mut cache_refs)?;
    let rss_flushed = process_rss_bytes(manager.page_size())?;
    let trimmed = manager.trim_to_retained_spans(retained)?;
    let rss_trimmed = process_rss_bytes(manager.page_size())?;
    Ok((active, free, rss_flushed, rss_trimmed, flushed, trimmed))
}

fn allocate_and_touch(
    manager: &GoInspiredStackManager,
    count: usize,
) -> Result<Vec<GoInspiredStack>, Box<dyn Error>> {
    let mut stacks = Vec::with_capacity(count);
    for _ in 0..count {
        let stack = manager.allocate()?;
        touch_all_pages(stack.bounds(), manager.page_size());
        stacks.push(stack);
    }
    Ok(stacks)
}

fn allocate_cached_and_touch(
    manager: &GoInspiredStackManager,
    caches: &mut [ProcessorStackCache],
    count: usize,
) -> Result<Vec<(GoInspiredStack, usize)>, Box<dyn Error>> {
    let mut stacks = Vec::with_capacity(count);
    for index in 0..count {
        let cache_index = index % caches.len();
        let stack = manager.allocate_cached(&mut caches[cache_index])?;
        touch_all_pages(stack.bounds(), manager.page_size());
        stacks.push((stack, cache_index));
    }
    Ok(stacks)
}

fn touch_all_pages(bounds: StackBounds, page_size: usize) {
    for address in (bounds.usable_start..bounds.usable_end).step_by(page_size) {
        // SAFETY: The live stack lease owns every writable page in this range.
        unsafe { std::ptr::write_volatile(address as *mut u8, 1) };
    }
}

fn check_vma_headroom(
    manager: &GoInspiredStackManager,
    count: usize,
) -> Result<(), Box<dyn Error>> {
    let snapshot = manager.vma_snapshot()?;
    let needed = u64::try_from(count)?
        .checked_mul(2)
        .and_then(|value| value.checked_add(1_024))
        .ok_or("VMA preflight calculation overflow")?;
    if snapshot.current.saturating_add(needed) > snapshot.limit {
        return Err(format!(
            "insufficient VMA headroom: current={} needed={needed} limit={}",
            snapshot.current, snapshot.limit
        )
        .into());
    }
    Ok(())
}

fn process_rss_bytes(page_size: usize) -> Result<u64, Box<dyn Error>> {
    let statm = std::fs::read_to_string("/proc/self/statm")?;
    let resident_pages = statm
        .split_whitespace()
        .nth(1)
        .ok_or("/proc/self/statm resident page count missing")?
        .parse::<u64>()?;
    resident_pages
        .checked_mul(u64::try_from(page_size)?)
        .ok_or_else(|| "RSS byte count overflow".into())
}

fn validate_final_state(
    metrics: &rust_gmp_runtime::stack::GoInspiredStackMetrics,
    released_spans: usize,
    retained: usize,
    span_count: usize,
) -> Result<(), Box<dyn Error>> {
    if released_spans != retained.min(span_count)
        || metrics.active_stacks != 0
        || metrics.local_free_slots != 0
        || metrics.global_free_slots != 0
        || metrics.reserved_virtual_bytes != 0
        || metrics.accessible_virtual_bytes != 0
    {
        return Err(format!(
            "allocator did not fully release after sample: released_spans={released_spans} metrics={metrics:?}"
        )
        .into());
    }
    Ok(())
}
