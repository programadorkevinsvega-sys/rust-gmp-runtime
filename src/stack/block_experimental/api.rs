use super::allocation::allocate_stack;
use super::manager::{BlockStack, BlockStackManager};
use super::timing::BlockStackTimingHandle;
use crate::stack::{BlockStackMetrics, StackError, VmaSnapshot};
use std::sync::Arc;

impl BlockStackManager {
    /// Reserves a protected slot and makes only its usable pages accessible.
    ///
    /// # Errors
    ///
    /// Returns an error if VMA admission, arena reservation, protection, or
    /// accounting fails.
    pub fn allocate(&self) -> Result<BlockStack, StackError> {
        allocate_stack(&self.inner)
    }

    /// Returns allocator state and a fresh process VMA observation.
    ///
    /// # Errors
    ///
    /// Returns an error if procfs data cannot be read or accounting is poisoned.
    pub fn metrics(&self) -> Result<BlockStackMetrics, StackError> {
        let snapshot = self.inner.refresh_snapshot()?;
        let mut state = self.inner.shared.lock()?;
        state.metrics.vma_snapshot = Some(snapshot);
        state.metrics.arenas =
            u64::try_from(state.arenas.len()).map_err(|source| StackError::IntegerConversion {
                operation: "block arena count",
                source,
            })?;
        Ok(state.metrics.clone())
    }

    /// Refreshes admission from procfs outside the allocator mutex.
    ///
    /// # Errors
    ///
    /// Returns an error if procfs cannot be read or its contents are malformed.
    pub fn vma_snapshot(&self) -> Result<VmaSnapshot, StackError> {
        self.inner.refresh_snapshot()
    }

    /// Returns the page size used to size arena slots.
    pub fn page_size(&self) -> usize {
        self.inner.page_size
    }

    /// Returns and clears the optional per-operation timing counters.
    pub fn timing_snapshot_and_reset(&self) -> super::BlockStackTimingSnapshot {
        self.inner.timing.snapshot_and_reset()
    }

    /// Returns a timing handle that can observe final arena `munmap` calls.
    pub fn timing_handle(&self) -> BlockStackTimingHandle {
        BlockStackTimingHandle::new(Arc::clone(&self.inner.timing))
    }
}
