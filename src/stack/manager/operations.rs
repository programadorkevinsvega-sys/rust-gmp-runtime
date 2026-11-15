use super::super::metrics::StackMetrics;
use super::super::vma::VmaSnapshot;
use super::{StackBackend, StackEngine, StackError, StackManager};

impl StackManager {
    /// Returns the selected production allocator.
    pub fn engine(&self) -> StackEngine {
        match &self.backend {
            StackBackend::Fixed(_) => StackEngine::Fixed,
            StackBackend::GoInspired(_) => StackEngine::GoInspired,
        }
    }

    /// Reads the current Linux VMA limit and process mapping count.
    ///
    /// # Errors
    ///
    /// Returns an error if procfs is unavailable, malformed, or admission
    /// accounting is poisoned.
    pub fn vma_snapshot(&self) -> Result<VmaSnapshot, StackError> {
        match &self.backend {
            StackBackend::Fixed(manager) => manager.vma_snapshot(),
            StackBackend::GoInspired(manager) => manager.vma_snapshot(),
        }
    }

    /// Performs an advisory admission check for additional stack slots.
    ///
    /// # Errors
    ///
    /// Returns an error if VMA observation is unavailable or the estimate
    /// exceeds the configured budget.
    pub fn preflight_additional_stacks(&self, count: usize) -> Result<VmaSnapshot, StackError> {
        match &self.backend {
            StackBackend::Fixed(manager) => manager.preflight_additional_stacks(count),
            StackBackend::GoInspired(manager) => manager.preflight_additional_stacks(count),
        }
    }

    /// Returns common counters and the selected engine's detailed observations.
    ///
    /// # Errors
    ///
    /// Returns an error if accounting or required procfs observations fail.
    pub fn metrics(&self) -> Result<StackMetrics, StackError> {
        match &self.backend {
            StackBackend::Fixed(manager) => manager.metrics(),
            StackBackend::GoInspired(manager) => super::metrics::go_inspired(manager),
        }
    }

    /// Trims fully idle reusable spans; Fixed has no retained spans.
    ///
    /// # Errors
    ///
    /// Returns an error if pool, mapping, or accounting cleanup fails.
    pub fn trim(&self) -> Result<usize, StackError> {
        match &self.backend {
            StackBackend::Fixed(_) => Ok(0),
            StackBackend::GoInspired(manager) => manager.trim(),
        }
    }

    /// Trims idle Go-inspired spans to a caller-selected retention target.
    ///
    /// Fixed has no retained spans and returns zero.
    ///
    /// # Errors
    ///
    /// Returns an error if pool, mapping, or accounting cleanup fails.
    pub fn trim_to_retained_spans(&self, retained_spans: usize) -> Result<usize, StackError> {
        match &self.backend {
            StackBackend::Fixed(_) => Ok(0),
            StackBackend::GoInspired(manager) => manager.trim_to_retained_spans(retained_spans),
        }
    }
}
