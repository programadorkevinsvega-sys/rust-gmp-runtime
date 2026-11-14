use std::sync::Arc;

use super::super::benchmark_metrics::StackBenchmarkMetrics;
use super::super::config::StackConfig;
use super::super::error::StackError;
use super::StackManager;

impl StackManager {
    /// Creates an isolated Fixed Stack manager using the two-phase syscall flow
    /// without collecting timing counters.
    ///
    /// This constructor is available only for allocator benchmarks and does not
    /// change the production default.
    ///
    /// # Errors
    ///
    /// Returns an error when the target, page size, or stack-size checks fail.
    pub fn new_benchmark_two_phase_uninstrumented(config: StackConfig) -> Result<Self, StackError> {
        Self::new_inner(config, true)
    }

    /// Creates a manager with opt-in timing counters for allocator benchmarks.
    ///
    /// # Errors
    ///
    /// Returns an error when the target, page size, or stack-size checks fail.
    pub fn new_benchmarked(
        config: StackConfig,
    ) -> Result<(Self, Arc<StackBenchmarkMetrics>), StackError> {
        let metrics = Arc::new(StackBenchmarkMetrics::default());
        let mut manager = Self::new_inner(config, false)?;
        manager.benchmark_metrics = Some(Arc::clone(&metrics));
        Ok((manager, metrics))
    }

    /// Creates a benchmark manager using the experimental two-phase syscall flow.
    ///
    /// This preserves Fixed Stack and VMA preflight, but performs mapping syscalls
    /// outside the shared admission mutex.
    ///
    /// # Errors
    ///
    /// Returns an error when the target, page size, or stack-size checks fail.
    pub fn new_benchmarked_two_phase(
        config: StackConfig,
    ) -> Result<(Self, Arc<StackBenchmarkMetrics>), StackError> {
        let metrics = Arc::new(StackBenchmarkMetrics::default());
        let mut manager = Self::new_inner(config, true)?;
        manager.benchmark_metrics = Some(Arc::clone(&metrics));
        Ok((manager, metrics))
    }

    /// Creates an isolated timing-counter view over the same allocator.
    pub fn benchmark_worker(&self) -> (Self, Arc<StackBenchmarkMetrics>) {
        let metrics = Arc::new(StackBenchmarkMetrics::default());
        let manager = Self {
            inner: Arc::clone(&self.inner),
            two_phase_syscalls: self.two_phase_syscalls,
            benchmark_metrics: Some(Arc::clone(&metrics)),
        };
        (manager, metrics)
    }
}
