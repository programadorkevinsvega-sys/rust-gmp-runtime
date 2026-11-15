use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::routine::RoutineMetadata;
use rust_gmp_runtime::stack::StackManager;

use super::AllocationMode;

pub(super) fn verify(manager: &MachineManager, count: usize) -> Result<usize, Box<dyn Error>> {
    let executions = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(count);
    for _ in 0..count {
        let executions = Arc::clone(&executions);
        handles.push(manager.dispatch_new(RoutineMetadata::new(), move || {
            executions.fetch_add(1, Ordering::SeqCst);
        })?);
    }
    for handle in handles {
        handle.join()?;
    }
    let observed = executions.load(Ordering::SeqCst);
    if observed != count {
        return Err(format!("expected {count} validation entries, observed {observed}").into());
    }
    Ok(observed)
}

pub(super) fn check(
    stacks: &StackManager,
    mode: AllocationMode,
    workers: usize,
    observed: usize,
) -> Result<(), Box<dyn Error>> {
    let metrics = stacks.metrics()?;
    let go = metrics
        .go_inspired
        .as_ref()
        .ok_or("missing Go-inspired metrics")?;
    let measured_and_warmup = super::SAMPLE_COUNT
        .checked_mul(super::TASK_COUNT)
        .and_then(|count| count.checked_add(64))
        .and_then(|count| count.checked_mul(3))
        .ok_or("expected routine count overflow")?;
    let expected = u64::try_from(
        measured_and_warmup
            .checked_add(super::VALIDATION_COUNT)
            .ok_or("expected routine count overflow")?,
    )?;
    if go.allocations != expected
        || go.releases != expected
        || go.active_stacks != 0
        || go.allocation_failures != 0
        || go.cleanup_failures != 0
        || metrics.active_stacks != 0
        || go.local_free_slots != 0
    {
        return Err(format!("stack accounting validation failed: {metrics:?}").into());
    }
    if mode == AllocationMode::Global && (go.cache_hits != 0 || go.cache_misses != 0) {
        return Err("global path unexpectedly recorded Processor-cache activity".into());
    }
    if mode == AllocationMode::ProcessorCache && go.cache_hits + go.cache_misses != expected {
        return Err("Processor-cache operation count did not match stack allocations".into());
    }
    println!(
        "machine_processor_cache_validation mode={} workers={workers} expected={expected} observed={observed} allocations={} releases={} cache_hits={} cache_misses={} active={} local_free={} failures=0 exactly_once=true",
        mode.name(),
        go.allocations,
        go.releases,
        go.cache_hits,
        go.cache_misses,
        go.active_stacks,
        go.local_free_slots
    );
    Ok(())
}
