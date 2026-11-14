use crate::stack::{StackConfig, StackError, VmaSnapshot};
use std::sync::Arc;

use super::cache::{ProcessorStackCache, record_failure};
use super::layout::GoInspiredOptions;
use super::lease::GoInspiredStack;
use super::manager::Inner;
use super::metrics::{AtomicMetrics, GoInspiredStackMetrics};
use super::span::local_free_state;
use super::timing::GoInspiredTimingSnapshot;

/// Isolated experimental allocator using spans, global pools, and local caches.
#[derive(Clone)]
pub struct GoInspiredStackManager {
    inner: Arc<Inner>,
}

impl GoInspiredStackManager {
    /// Creates a manager with the 32 KiB cache target and 64-slot spans.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported targets, invalid dimensions, overflow,
    /// or an invalid system page size.
    pub fn new(config: StackConfig) -> Result<Self, StackError> {
        Self::with_options(config, GoInspiredOptions::default())
    }

    /// Creates a manager with explicit experimental cache and span sizing.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported targets, invalid dimensions, overflow,
    /// or an invalid system page size.
    pub fn with_options(
        config: StackConfig,
        options: GoInspiredOptions,
    ) -> Result<Self, StackError> {
        Ok(Self {
            inner: Inner::new(config, options, false)?,
        })
    }

    /// Creates a manager intended for benchmark timing collection.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::with_options`].
    pub fn new_benchmarked(
        config: StackConfig,
        options: GoInspiredOptions,
    ) -> Result<Self, StackError> {
        Ok(Self {
            inner: Inner::new(config, options, true)?,
        })
    }

    /// Creates an empty cache to be owned exclusively by a Processor or worker.
    ///
    /// # Errors
    ///
    /// Returns an error if reserving the cache metadata fails.
    pub fn processor_cache(&self) -> Result<ProcessorStackCache, StackError> {
        ProcessorStackCache::new(Arc::clone(&self.inner))
            .map_err(|error| record_failure(&self.inner.metrics, error, true))
    }

    /// Allocates a lease from the global class pool.
    ///
    /// # Errors
    ///
    /// Returns an error if admission, mapping, slot activation, accounting, or
    /// synchronization fails.
    pub fn allocate(&self) -> Result<GoInspiredStack, StackError> {
        self.inner
            .allocate_global()
            .map_err(|error| record_failure(&self.inner.metrics, error, true))
    }
}

mod cached;
mod metrics;
mod spans;
