use crate::stack::StackError;
use std::sync::Arc;

use super::manager::Inner;
use super::metrics::AtomicMetrics;
use super::slot::SlotToken;
use super::span::local_free_state;

/// Exclusive local cache that a future Processor can own directly.
pub struct ProcessorStackCache {
    pub(super) inner: Arc<Inner>,
    slots: Vec<SlotToken>,
    refill_target: usize,
}

impl ProcessorStackCache {
    pub(super) fn new(inner: Arc<Inner>) -> Result<Self, StackError> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(inner.layout.cache_capacity)
            .map_err(|error| allocation_error("reserve Processor stack cache", error))?;
        Ok(Self {
            refill_target: inner.layout.cache_target,
            inner,
            slots,
        })
    }

    /// Returns the number of free slots currently held by this cache.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Reports whether this cache contains no free slots.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Returns the target number of slots requested when this cache refills.
    pub fn refill_target(&self) -> usize {
        self.refill_target
    }

    /// Sets the target for future refills without changing the manager default.
    ///
    /// Already cached slots are unaffected. The target must fit the cache's
    /// configured capacity.
    ///
    /// # Errors
    ///
    /// Returns an error when the target is zero or exceeds cache capacity.
    pub fn set_refill_target(&mut self, target: usize) -> Result<(), StackError> {
        if target == 0 || target > self.inner.layout.cache_capacity {
            return Err(StackError::InvalidCacheRefillTarget {
                requested_slots: target,
                capacity_slots: self.inner.layout.cache_capacity,
            });
        }
        self.refill_target = target;
        Ok(())
    }

    /// Transfers a batch from the class pool, creating a span if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error if span admission, mapping, pool locking, or metadata
    /// allocation fails.
    pub fn refill(&mut self) -> Result<usize, StackError> {
        match self.refill_inner() {
            Ok(count) => Ok(count),
            Err(error) => Err(record_failure(&self.inner.metrics, error, true)),
        }
    }

    /// Returns all locally cached slots to the global pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the global pool is poisoned or accounting overflows.
    pub fn clear(&mut self) -> Result<(), StackError> {
        self.flush_cache().map(|_| ())
    }

    /// Flushes all free slots from this cache to the manager's global pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the global pool is poisoned or accounting overflows.
    pub fn flush_cache(&mut self) -> Result<usize, StackError> {
        match self.flush_cache_inner() {
            Ok(flushed) => Ok(flushed),
            Err(error) => Err(record_failure(&self.inner.metrics, error, false)),
        }
    }

    fn flush_cache_inner(&mut self) -> Result<usize, StackError> {
        if self.slots.is_empty() {
            return Ok(0);
        }
        let count =
            u64::try_from(self.slots.len()).map_err(|source| StackError::IntegerConversion {
                operation: "cleared Processor stack cache",
                source,
            })?;
        self.inner
            .pool
            .return_global(&mut self.slots, local_free_state(), &self.inner.metrics)?;
        super::metrics::AtomicMetrics::subtract(&self.inner.metrics.local_free_slots, count)?;
        super::metrics::AtomicMetrics::add(&self.inner.metrics.cache_releases, count)?;
        usize::try_from(count).map_err(|source| StackError::IntegerConversion {
            operation: "flushed Processor stack cache count",
            source,
        })
    }

    pub(super) fn refill_inner(&mut self) -> Result<usize, StackError> {
        if !self.slots.is_empty() {
            return Ok(0);
        }
        AtomicMetrics::add(&self.inner.metrics.cache_refill_requests, 1)?;
        let _refill = self.inner.metrics.begin_refill()?;
        loop {
            let batch = self
                .inner
                .pool
                .take_batch(self.refill_target, &self.inner.metrics)?;
            if !batch.is_empty() {
                let count = batch.len();
                self.slots.extend(batch);
                return Ok(count);
            }
            self.inner.create_span()?;
        }
    }

    pub(super) fn belongs_to(&self, inner: &Arc<Inner>) -> bool {
        Arc::ptr_eq(&self.inner, inner)
    }

    pub(super) fn take(&mut self) -> Option<SlotToken> {
        self.slots.pop()
    }

    pub(super) fn accept(&mut self, token: &mut Option<SlotToken>) -> Result<bool, StackError> {
        if self.slots.len() >= self.inner.layout.cache_capacity {
            return Ok(false);
        }
        let token_ref = token.as_ref().ok_or(StackError::MetricsInvariant {
            operation: "cache an absent Go-inspired stack lease",
            observed: "none".to_owned(),
        })?;
        let metrics = &self.inner.metrics;
        AtomicMetrics::subtract(&metrics.active_stacks, 1)?;
        if let Err(original) = AtomicMetrics::add(&metrics.releases, 1) {
            return Err(rollback_accept(self, original, false, false));
        }
        if let Err(original) = AtomicMetrics::add(&metrics.local_free_slots, 1) {
            return Err(rollback_accept(self, original, true, false));
        }
        if let Err(original) = token_ref.span.set_free_state(
            token_ref.index,
            super::span::live_state(),
            local_free_state(),
        ) {
            return Err(rollback_accept(self, original, true, true));
        }
        self.slots.push(token_ref.clone());
        *token = None;
        Ok(true)
    }
}

impl Drop for ProcessorStackCache {
    fn drop(&mut self) {
        if let Err(error) = self.flush_cache_inner() {
            let error = record_failure(&self.inner.metrics, error, false);
            eprintln!("fatal: failed to clear Processor stack cache: {error}");
            std::process::abort();
        }
    }
}

mod failure;
pub(super) use failure::record_failure;
use failure::{allocation_error, rollback_accept};
