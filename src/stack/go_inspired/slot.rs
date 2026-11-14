use crate::stack::StackError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

use super::span::Span;

pub(super) const NEW: u8 = 0;
pub(super) const GLOBAL_FREE: u8 = 1;
pub(super) const LOCAL_FREE: u8 = 2;
pub(super) const ACTIVATING: u8 = 3;
pub(super) const LIVE: u8 = 4;
pub(super) const QUARANTINED: u8 = 5;

#[derive(Debug)]
pub(super) struct SlotState {
    pub state: AtomicU8,
    pub enabled: AtomicBool,
}

#[derive(Clone, Debug)]
pub(super) struct SlotToken {
    pub span: Arc<Span>,
    pub index: usize,
}

pub(super) fn increment(counter: &AtomicUsize) -> Result<(), StackError> {
    increment_by(counter, 1)
}

pub(super) fn increment_by(counter: &AtomicUsize, amount: usize) -> Result<(), StackError> {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(amount)
        })
        .map(|_| ())
        .map_err(|current| StackError::MetricsInvariant {
            operation: "increment Go-inspired slot counter",
            observed: current.to_string(),
        })
}

pub(super) fn decrement(counter: &AtomicUsize) -> Result<(), StackError> {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_sub(1)
        })
        .map(|_| ())
        .map_err(|current| StackError::MetricsInvariant {
            operation: "decrement Go-inspired slot counter",
            observed: current.to_string(),
        })
}
