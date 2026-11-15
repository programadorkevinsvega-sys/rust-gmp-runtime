use std::error::Error;

use rust_gmp_runtime::stack::{StackManager, StackMetrics};

use super::SLOTS_PER_SPAN;

pub(super) fn validate_retained_state(metrics: &StackMetrics) -> Result<(), Box<dyn Error>> {
    let spans = active_span_count(metrics)?;
    let expected_free_slots = spans
        .checked_mul(u64::try_from(SLOTS_PER_SPAN)?)
        .ok_or("free slot count overflow")?;
    if metrics.active_stacks != 0
        || metrics.free_stacks != expected_free_slots
        || metrics.reserved_virtual_bytes == 0
        || metrics.accessible_virtual_bytes == 0
    {
        return Err(format!("unexpected post-execution stack state: {metrics:?}").into());
    }
    Ok(())
}

pub(super) fn validate_retained_span(
    metrics: &StackMetrics,
    stacks: &StackManager,
) -> Result<(), Box<dyn Error>> {
    let enabled_slot_bytes = u64::try_from(stacks.usable_size())?;
    let max_enabled_bytes = enabled_slot_bytes
        .checked_mul(u64::try_from(SLOTS_PER_SPAN)?)
        .ok_or("accessible byte count overflow")?;
    let expected_span_bytes = stacks
        .usable_size()
        .checked_add(stacks.page_size())
        .and_then(|stride| stride.checked_mul(SLOTS_PER_SPAN))
        .ok_or("span size overflow")?;
    if metrics.active_stacks != 0
        || metrics.free_stacks != u64::try_from(SLOTS_PER_SPAN)?
        || metrics.reserved_virtual_bytes != u64::try_from(expected_span_bytes)?
        || metrics.accessible_virtual_bytes == 0
        || metrics.accessible_virtual_bytes > max_enabled_bytes
        || enabled_slot_bytes == 0
        || metrics.accessible_virtual_bytes % enabled_slot_bytes != 0
    {
        return Err(
            format!("default trim did not retain exactly one clean span: {metrics:?}").into(),
        );
    }
    Ok(())
}

pub(super) fn active_span_count(metrics: &StackMetrics) -> Result<u64, Box<dyn Error>> {
    let details = metrics
        .go_inspired
        .as_ref()
        .ok_or("Go-inspired metrics missing")?;
    details
        .spans_created
        .checked_sub(details.spans_released)
        .ok_or_else(|| "released spans exceed created spans".into())
}
