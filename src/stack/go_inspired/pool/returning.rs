use crate::stack::StackError;

use super::super::metrics::AtomicMetrics;
use super::super::slot::SlotToken;
use super::super::span::global_free_state;
use super::GlobalStackPool;

impl GlobalStackPool {
    pub fn return_global(
        &self,
        slots: &mut Vec<SlotToken>,
        prior_state: u8,
        metrics: &AtomicMetrics,
    ) -> Result<(), StackError> {
        let count = slots.len();
        let mut state = self.lock_class(0, metrics)?;
        state
            .free
            .try_reserve(count)
            .map_err(|error| allocation_error("grow global stack pool", error))?;
        let count_u64 = to_u64(count)?;
        if count > 0 {
            AtomicMetrics::add(&metrics.global_free_slots, count_u64)?;
        }

        for (index, token) in slots.iter().enumerate() {
            if let Err(original) =
                token
                    .span
                    .set_free_state(token.index, prior_state, global_free_state())
            {
                let rollback = rollback_states(&slots[..index], prior_state, metrics, count_u64);
                return Err(match rollback {
                    Some(cleanup) => StackError::CleanupAfterFailure {
                        original: Box::new(original),
                        cleanup: Box::new(cleanup),
                    },
                    None => original,
                });
            }
        }
        state.free.append(slots);
        Ok(())
    }
}

fn rollback_states(
    slots: &[SlotToken],
    prior_state: u8,
    metrics: &AtomicMetrics,
    count: u64,
) -> Option<StackError> {
    let mut failure = None;
    for token in slots.iter().rev() {
        if let Err(error) = token
            .span
            .set_free_state(token.index, global_free_state(), prior_state)
        {
            failure = combine(failure, error);
        }
    }
    if count > 0 {
        if let Err(error) = AtomicMetrics::subtract(&metrics.global_free_slots, count) {
            failure = combine(failure, error);
        }
    }
    failure
}

fn combine(previous: Option<StackError>, current: StackError) -> Option<StackError> {
    match previous {
        Some(previous) => Some(StackError::CleanupAfterFailure {
            original: Box::new(previous),
            cleanup: Box::new(current),
        }),
        None => Some(current),
    }
}

fn to_u64(value: usize) -> Result<u64, StackError> {
    u64::try_from(value).map_err(|source| StackError::IntegerConversion {
        operation: "global free slot count",
        source,
    })
}

fn allocation_error(
    operation: &'static str,
    error: std::collections::TryReserveError,
) -> StackError {
    StackError::SystemCall {
        operation,
        source: std::io::Error::other(error),
    }
}
