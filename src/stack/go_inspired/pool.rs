use crate::stack::StackError;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use super::metrics::AtomicMetrics;
use super::slot::SlotToken;
use super::span::{Span, global_free_state, local_free_state, new_state};

mod returning;

#[derive(Debug)]
pub(super) struct GlobalStackPool {
    classes: Vec<StackPoolClass>,
    instrumented: bool,
}

#[derive(Debug)]
struct StackPoolClass {
    state: Mutex<ClassPoolState>,
}

#[derive(Debug, Default)]
pub(super) struct ClassPoolState {
    pub(super) spans: Vec<Arc<Span>>,
    pub(super) free: Vec<SlotToken>,
}

pub(super) struct PoolGuard<'guard, 'metrics> {
    guard: MutexGuard<'guard, ClassPoolState>,
    metrics: &'metrics AtomicMetrics,
    started: Option<Instant>,
}

impl GlobalStackPool {
    pub fn one_class(instrumented: bool) -> Self {
        Self {
            classes: vec![StackPoolClass {
                state: Mutex::new(ClassPoolState::default()),
            }],
            instrumented,
        }
    }

    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    pub fn add_span(&self, span: Arc<Span>, metrics: &AtomicMetrics) -> Result<(), StackError> {
        let mut state = self.lock_class(0, metrics)?;
        state
            .free
            .try_reserve(span.slot_count())
            .map_err(|error| allocation_error("reserve global stack slots", error))?;
        state
            .spans
            .try_reserve(1)
            .map_err(|error| allocation_error("reserve stack span list", error))?;
        for index in 0..span.slot_count() {
            span.set_free_state(index, new_state(), global_free_state())?;
            state.free.push(span.token(index)?);
        }
        let slots = to_u64(span.slot_count(), "global free slot count")?;
        AtomicMetrics::add(&metrics.global_free_slots, slots)?;
        state.spans.push(span);
        Ok(())
    }

    pub fn take_global(&self, metrics: &AtomicMetrics) -> Result<Option<SlotToken>, StackError> {
        let mut state = self.lock_class(0, metrics)?;
        let token = state.free.pop();
        if token.is_some() {
            AtomicMetrics::subtract(&metrics.global_free_slots, 1)?;
        }
        Ok(token)
    }

    pub fn take_batch(
        &self,
        count: usize,
        metrics: &AtomicMetrics,
    ) -> Result<Vec<SlotToken>, StackError> {
        let mut state = self.lock_class(0, metrics)?;
        let count = count.min(state.free.len());
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(count)
            .map_err(|error| allocation_error("reserve local stack cache", error))?;
        for _ in 0..count {
            let token = state.free.pop().ok_or(StackError::MetricsOverflow)?;
            token
                .span
                .set_free_state(token.index, global_free_state(), local_free_state())?;
            slots.push(token);
        }
        if count > 0 {
            let count = to_u64(count, "stack-cache refill count")?;
            AtomicMetrics::subtract(&metrics.global_free_slots, count)?;
            AtomicMetrics::add(&metrics.local_free_slots, count)?;
            AtomicMetrics::add(&metrics.cache_refills, 1)?;
            AtomicMetrics::add(&metrics.cache_refill_slots, count)?;
            AtomicMetrics::record_max(&metrics.cache_refill_max_batch, count);
        }
        Ok(slots)
    }

    pub fn lock_class<'guard, 'metrics>(
        &'guard self,
        class: usize,
        metrics: &'metrics AtomicMetrics,
    ) -> Result<PoolGuard<'guard, 'metrics>, StackError> {
        let pool = self.classes.get(class).ok_or(StackError::MetricsOverflow)?;
        let wait_started = self.instrumented.then(Instant::now);
        let guard = pool
            .state
            .lock()
            .map_err(|error| StackError::AdmissionPoisoned {
                message: error.to_string(),
            })?;
        if let Some(started) = wait_started {
            AtomicMetrics::record_elapsed(&metrics.lock_wait_ns, started);
        }
        Ok(PoolGuard {
            guard,
            metrics,
            started: self.instrumented.then(Instant::now),
        })
    }
}

impl Deref for PoolGuard<'_, '_> {
    type Target = ClassPoolState;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl DerefMut for PoolGuard<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl Drop for PoolGuard<'_, '_> {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            AtomicMetrics::record_elapsed(&self.metrics.lock_hold_ns, started);
        }
    }
}

fn to_u64(value: usize, operation: &'static str) -> Result<u64, StackError> {
    u64::try_from(value).map_err(|source| StackError::IntegerConversion { operation, source })
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
