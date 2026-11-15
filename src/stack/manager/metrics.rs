use super::super::config::StackEngine;
use super::super::error::StackError;
use super::super::go_inspired::GoInspiredStackManager;
use super::super::metrics::StackMetrics;
use super::super::process_metrics::read_process_metrics;

pub(super) fn go_inspired(manager: &GoInspiredStackManager) -> Result<StackMetrics, StackError> {
    let details = manager.metrics()?;
    let snapshot = details.vma_snapshot.ok_or(StackError::MalformedVmaLimit)?;
    let process = read_process_metrics(manager.page_size())?;
    let span_count = details
        .spans_created
        .checked_sub(details.spans_released)
        .ok_or(StackError::MetricsOverflow)?;
    let free_stacks = details
        .global_free_slots
        .checked_add(details.local_free_slots)
        .ok_or(StackError::MetricsOverflow)?;
    let accessible_slots = details
        .accessible_virtual_bytes
        .checked_div(u64::try_from(manager.usable_size()).map_err(|source| {
            StackError::IntegerConversion {
                operation: "Go-inspired usable stack size",
                source,
            }
        })?)
        .ok_or(StackError::MetricsOverflow)?;
    let runtime_vma_estimate = accessible_slots
        .checked_mul(super::super::admission::MAX_VMA_COST_PER_STACK)
        .ok_or(StackError::MetricsOverflow)?;
    Ok(StackMetrics {
        engine: StackEngine::GoInspired,
        go_inspired: Some(details.clone()),
        successful_allocations: details.allocations,
        released_stacks: details.releases,
        failed_allocations: details.allocation_failures,
        cleanup_failures: details.cleanup_failures,
        allocation_failures: details.allocation_failures,
        vma_preflight_rejections: details.vma_preflight_rejections,
        active_stacks: details.active_stacks,
        active_mappings: span_count,
        runtime_mapping_count: span_count,
        runtime_vma_estimate,
        free_stacks,
        reserved_virtual_bytes: details.reserved_virtual_bytes,
        accessible_virtual_bytes: details.accessible_virtual_bytes,
        process_virtual_bytes: process.virtual_bytes,
        process_resident_bytes: process.resident_bytes,
        process_vma_count: Some(snapshot.current),
        vma_limit: snapshot.limit,
        vma_current: snapshot.current,
        vma_remaining: snapshot.remaining,
        vma_utilization_percent: snapshot.utilization_percent,
    })
}
