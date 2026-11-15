use super::*;

pub(super) fn allocation_error(
    operation: &'static str,
    error: std::collections::TryReserveError,
) -> StackError {
    StackError::SystemCall {
        operation,
        source: std::io::Error::other(error),
    }
}

pub(super) fn rollback_accept(
    cache: &ProcessorStackCache,
    original: StackError,
    releases_added: bool,
    local_free_added: bool,
) -> StackError {
    let metrics = &cache.inner.metrics;
    let mut cleanup = None;
    if local_free_added {
        if let Err(error) = AtomicMetrics::subtract(&metrics.local_free_slots, 1) {
            cleanup = Some(error);
        }
    }
    if releases_added {
        if let Err(error) = AtomicMetrics::subtract(&metrics.releases, 1) {
            cleanup = combine_failure(cleanup, error);
        }
    }
    if let Err(error) = AtomicMetrics::add(&metrics.active_stacks, 1) {
        cleanup = combine_failure(cleanup, error);
    }
    match cleanup {
        Some(cleanup) => StackError::CleanupAfterFailure {
            original: Box::new(original),
            cleanup: Box::new(cleanup),
        },
        None => original,
    }
}

pub(super) fn combine_failure(
    previous: Option<StackError>,
    current: StackError,
) -> Option<StackError> {
    match previous {
        Some(previous) => Some(StackError::CleanupAfterFailure {
            original: Box::new(previous),
            cleanup: Box::new(current),
        }),
        None => Some(current),
    }
}

pub(in crate::stack::go_inspired) fn record_failure(
    metrics: &AtomicMetrics,
    original: StackError,
    allocation: bool,
) -> StackError {
    let counter = if allocation {
        &metrics.allocation_failures
    } else {
        &metrics.cleanup_failures
    };
    let mut result = match AtomicMetrics::add(counter, 1) {
        Ok(()) => original,
        Err(cleanup) => StackError::CleanupAfterFailure {
            original: Box::new(original),
            cleanup: Box::new(cleanup),
        },
    };
    if allocation && contains_vma_rejection(&result) {
        if let Err(cleanup) = AtomicMetrics::add(&metrics.vma_preflight_rejections, 1) {
            result = StackError::CleanupAfterFailure {
                original: Box::new(result),
                cleanup: Box::new(cleanup),
            };
        }
    }
    result
}

pub(in crate::stack::go_inspired) fn record_preflight_rejection(
    metrics: &AtomicMetrics,
    original: StackError,
) -> StackError {
    if contains_vma_rejection(&original) {
        match AtomicMetrics::add(&metrics.vma_preflight_rejections, 1) {
            Ok(()) => original,
            Err(cleanup) => StackError::CleanupAfterFailure {
                original: Box::new(original),
                cleanup: Box::new(cleanup),
            },
        }
    } else {
        original
    }
}

fn contains_vma_rejection(error: &StackError) -> bool {
    match error {
        StackError::VmaPreflightRejected { .. } => true,
        StackError::CleanupAfterFailure { original, cleanup } => {
            contains_vma_rejection(original) || contains_vma_rejection(cleanup)
        }
        StackError::InvalidSize { .. }
        | StackError::InvalidCacheRefillTarget { .. }
        | StackError::BenchmarkRequiresFixedEngine { .. }
        | StackError::StackEngineMismatch { .. }
        | StackError::SizeOverflow
        | StackError::UnsupportedPlatform
        | StackError::SystemCall { .. }
        | StackError::InvalidSystemValue { .. }
        | StackError::MetricsPoisoned { .. }
        | StackError::MetricsOverflow
        | StackError::MetricsInvariant { .. }
        | StackError::IntegerConversion { .. }
        | StackError::ProcessMetricsParse { .. }
        | StackError::ProcessMetrics { .. }
        | StackError::VmaProcfs { .. }
        | StackError::VmaLimitParse { .. }
        | StackError::MalformedVmaLimit
        | StackError::MalformedVmaMaps { .. }
        | StackError::VmaPreflightDisabled
        | StackError::AdmissionPoisoned { .. }
        | StackError::MalformedProcessMetrics { .. }
        | StackError::CleanupFailure { .. } => false,
    }
}
