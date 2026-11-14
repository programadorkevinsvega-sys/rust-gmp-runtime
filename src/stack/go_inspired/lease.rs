use crate::stack::{StackBounds, StackError};
use std::sync::Arc;

use super::cache::{ProcessorStackCache, record_failure};
use super::manager::Inner;
use super::slot::SlotToken;
use super::span::live_state;

/// RAII lease to one stable-address slot owned by the Go-inspired manager.
pub struct GoInspiredStack {
    inner: Arc<Inner>,
    token: Option<SlotToken>,
    start: usize,
}

impl GoInspiredStack {
    pub(super) fn new(inner: Arc<Inner>, token: SlotToken, start: usize) -> Self {
        Self {
            inner,
            token: Some(token),
            start,
        }
    }

    /// Returns the guard and usable address range.
    pub fn bounds(&self) -> StackBounds {
        let token = match &self.token {
            Some(token) => token,
            None => {
                return StackBounds {
                    guard_start: self.start,
                    guard_end: self.start,
                    usable_start: self.start,
                    usable_end: self.start,
                };
            }
        };
        let page_size = token.span.stride() - token.span.usable_size();
        let usable_start = self.start.saturating_add(page_size);
        StackBounds {
            guard_start: self.start,
            guard_end: usable_start,
            usable_start,
            usable_end: usable_start.saturating_add(token.span.usable_size()),
        }
    }

    /// Returns the usable capacity of this stack slot.
    pub fn usable_size(&self) -> usize {
        self.token
            .as_ref()
            .map_or(0, |token| token.span.usable_size())
    }

    /// Returns the reserved slot size, including its guard page.
    pub fn reserved_size(&self) -> usize {
        self.token.as_ref().map_or(0, |token| token.span.stride())
    }

    /// Returns the lease to the supplied current-Processor cache.
    ///
    /// If the cache belongs to another manager or has no capacity, the slot is
    /// returned to the global pool instead.
    /// If this operation fails, the lease remains live and can be retried.
    ///
    /// # Errors
    ///
    /// Returns an error if the global pool or accounting is poisoned.
    pub fn release_to_cache(&mut self, cache: &mut ProcessorStackCache) -> Result<(), StackError> {
        let result = self.release_to_cache_inner(cache);
        result.map_err(|error| record_failure(&self.inner.metrics, error, false))
    }

    fn release_to_cache_inner(
        &mut self,
        cache: &mut ProcessorStackCache,
    ) -> Result<(), StackError> {
        if self.token.is_none() {
            return Err(StackError::MetricsInvariant {
                operation: "release an absent Go-inspired stack lease",
                observed: "none".to_owned(),
            });
        }
        if cache.belongs_to(&self.inner) && cache.accept(&mut self.token)? {
            return Ok(());
        }
        let token = self.token.as_ref().ok_or(StackError::MetricsInvariant {
            operation: "release an absent Go-inspired stack lease",
            observed: "none".to_owned(),
        })?;
        self.inner.release(token, live_state())?;
        self.token = None;
        Ok(())
    }
}

impl Drop for GoInspiredStack {
    fn drop(&mut self) {
        if let Some(token) = self.token.as_ref() {
            if let Err(error) = self.inner.release(token, live_state()) {
                let error = record_failure(&self.inner.metrics, error, false);
                eprintln!("fatal: failed to return Go-inspired stack lease: {error}");
                std::process::abort();
            }
            self.token = None;
        }
    }
}
