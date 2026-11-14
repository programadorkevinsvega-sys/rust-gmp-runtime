use super::*;

impl GoInspiredStackManager {
    /// Allocates from an explicit local cache, refilling it in batches as needed.
    ///
    /// A cache from another manager is not used; the allocation safely falls
    /// back to this manager's global pool.
    ///
    /// # Errors
    ///
    /// Returns an error if refill, admission, mapping, activation, accounting,
    /// or synchronization fails.
    pub fn allocate_cached(
        &self,
        cache: &mut ProcessorStackCache,
    ) -> Result<GoInspiredStack, StackError> {
        let result = self.allocate_cached_inner(cache);
        result.map_err(|error| record_failure(&self.inner.metrics, error, true))
    }

    fn allocate_cached_inner(
        &self,
        cache: &mut ProcessorStackCache,
    ) -> Result<GoInspiredStack, StackError> {
        if !cache.belongs_to(&self.inner) {
            return self.inner.allocate_global();
        }
        let token = if let Some(token) = cache.take() {
            AtomicMetrics::subtract(&self.inner.metrics.local_free_slots, 1)?;
            AtomicMetrics::add(&self.inner.metrics.cache_hits, 1)?;
            token
        } else {
            AtomicMetrics::add(&self.inner.metrics.cache_misses, 1)?;
            cache.refill_inner()?;
            let token = cache.take().ok_or(StackError::MetricsOverflow)?;
            AtomicMetrics::subtract(&self.inner.metrics.local_free_slots, 1)?;
            token
        };
        self.inner.activate(token, local_free_state())
    }
}
