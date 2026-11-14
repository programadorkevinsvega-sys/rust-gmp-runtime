use super::manager::BlockStackInner;
use crate::stack::error::StackError;

pub(super) fn cancel_after_failure(inner: &BlockStackInner, original: StackError) -> StackError {
    let cancel = super::budget::cancel(inner);
    let failure = record_allocation_failure(inner, original);
    match cancel {
        Ok(()) => failure,
        Err(error) => combine(failure, Err(error)),
    }
}

pub(super) fn record_allocation_failure(inner: &BlockStackInner, error: StackError) -> StackError {
    match inner.lock() {
        Ok(mut state) => match state.metrics.allocation_failures.checked_add(1) {
            Some(count) => {
                state.metrics.allocation_failures = count;
                error
            }
            None => combine(error, Err(StackError::MetricsOverflow)),
        },
        Err(accounting) => combine(error, Err(accounting)),
    }
}

pub(super) fn combine(original: StackError, cleanup: Result<(), StackError>) -> StackError {
    match cleanup {
        Ok(()) => original,
        Err(cleanup) => StackError::CleanupAfterFailure {
            original: Box::new(original),
            cleanup: Box::new(cleanup),
        },
    }
}
