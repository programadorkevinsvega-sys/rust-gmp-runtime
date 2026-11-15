use super::*;

impl GoInspiredStackManager {
    /// Releases completely idle spans beyond the configured retention count.
    ///
    /// A span referenced by a lease or local cache is never released.
    ///
    /// # Errors
    ///
    /// Returns an error if admission, metadata allocation, or accounting fails.
    pub fn trim(&self) -> Result<usize, StackError> {
        self.inner
            .trim()
            .map_err(|error| record_failure(&self.inner.metrics, error, false))
    }

    /// Trims empty spans to a caller-selected retention target.
    ///
    /// Callers may lower the target in response to memory pressure or raise it
    /// to retain warm capacity. The pressure signal and cache quiescence remain
    /// responsibilities of the caller; spans referenced by leases or caches
    /// are never released.
    ///
    /// # Errors
    ///
    /// Returns an error if admission, metadata allocation, or accounting fails.
    pub fn trim_to_retained_spans(&self, retained_spans: usize) -> Result<usize, StackError> {
        self.inner
            .trim_to_retained_spans(retained_spans)
            .map_err(|error| record_failure(&self.inner.metrics, error, false))
    }

    /// Flushes the supplied set of local caches at a caller-coordinated safe
    /// point. The caller must pass every cache belonging to this manager.
    ///
    /// # Errors
    ///
    /// Returns an error if a cache belongs to another manager or any pool
    /// transfer fails. Caches successfully flushed before an error remain
    /// flushed.
    pub fn flush_all_caches(
        &self,
        caches: &mut [&mut ProcessorStackCache],
    ) -> Result<usize, StackError> {
        if caches.iter().any(|cache| !cache.belongs_to(&self.inner)) {
            return Err(record_failure(
                &self.inner.metrics,
                StackError::MetricsInvariant {
                    operation: "flush a cache belonging to another Go-inspired manager",
                    observed: "foreign cache".to_owned(),
                },
                false,
            ));
        }
        let mut flushed_slots = 0_usize;
        for cache in caches.iter_mut() {
            flushed_slots = flushed_slots
                .checked_add(cache.flush_cache()?)
                .ok_or(StackError::SizeOverflow)?;
        }
        Ok(flushed_slots)
    }

    /// Flushes every supplied local cache and releases every fully idle span.
    ///
    /// This operation is intended for a coordinated scheduler safe point. All
    /// live caches must be supplied, and active leases remain untouched.
    ///
    /// # Errors
    ///
    /// Returns an error if a cache is foreign, a cache transfer fails, or a
    /// span cannot be safely unmapped.
    pub fn flush_all_caches_and_release_empty_spans(
        &self,
        caches: &mut [&mut ProcessorStackCache],
    ) -> Result<usize, StackError> {
        self.flush_all_caches(caches)?;
        self.release_empty_spans()
    }

    /// Releases every span with no active or locally cached slots.
    ///
    /// Unlike [`Self::trim`], this ignores the configured warm-span retention
    /// target. Local caches must be flushed separately before this call.
    ///
    /// # Errors
    ///
    /// Returns an error if a span cannot be safely unmapped or accounting fails.
    pub fn release_empty_spans(&self) -> Result<usize, StackError> {
        self.inner
            .release_empty_spans()
            .map_err(|error| record_failure(&self.inner.metrics, error, false))
    }

    /// Returns the number of real size classes owned by this manager.
    pub fn class_count(&self) -> usize {
        self.inner.pool.class_count()
    }

    /// Returns the configured number of slots requested by each cache refill.
    pub fn cache_refill_target(&self) -> usize {
        self.inner.layout.cache_target
    }

    /// Returns the maximum whole-slot refill size supported by this cache.
    pub fn cache_refill_capacity(&self) -> usize {
        self.inner.layout.cache_capacity
    }

    /// Returns the page size used by the span layout.
    pub fn page_size(&self) -> usize {
        self.inner.layout.page_size
    }

    /// Returns the requested usable stack size before page alignment.
    pub fn requested_usable_size(&self) -> usize {
        self.inner.layout.requested_usable_size
    }

    /// Returns the page-aligned usable size of each stack.
    pub fn usable_size(&self) -> usize {
        self.inner.layout.usable_size
    }

    /// Performs an advisory preflight for additional stack slots.
    ///
    /// The estimate does not reserve kernel VMA capacity; allocation still
    /// performs admission before creating each span.
    ///
    /// # Errors
    ///
    /// Returns an error if VMA observation or admission accounting fails.
    pub fn preflight_additional_stacks(&self, count: usize) -> Result<VmaSnapshot, StackError> {
        self.inner
            .admission
            .preflight_slots(count)
            .map_err(|error| record_preflight_rejection(&self.inner.metrics, error))
    }

    #[cfg(test)]
    pub(in crate::stack::go_inspired) fn fail_next_span_release_for_test(
        &self,
    ) -> Result<(), StackError> {
        let state = self.inner.pool.lock_class(0, &self.inner.metrics)?;
        let span = state.spans.first().ok_or(StackError::MetricsInvariant {
            operation: "inject span release failure",
            observed: "no spans".to_owned(),
        })?;
        span.mapping.fail_next_release_for_test();
        Ok(())
    }
}
