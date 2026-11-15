use std::sync::Arc;

use super::super::allocator::FixedStackManager;
use super::super::benchmark_metrics::StackBenchmarkMetrics;
use super::super::config::{StackConfig, StackEngine};
use super::super::error::StackError;
use super::{StackBackend, StackManager};

impl StackManager {
    /// Creates a Fixed Stack manager using the benchmark-only two-phase path.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not Fixed or the manager cannot start.
    pub fn new_benchmark_two_phase_uninstrumented(config: StackConfig) -> Result<Self, StackError> {
        require_fixed(config)?;
        Ok(Self {
            backend: StackBackend::Fixed(
                FixedStackManager::new_benchmark_two_phase_uninstrumented(config)?,
            ),
        })
    }

    /// Creates a Fixed Stack manager with allocator timing counters.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not Fixed or the manager cannot start.
    pub fn new_benchmarked(
        config: StackConfig,
    ) -> Result<(Self, Arc<StackBenchmarkMetrics>), StackError> {
        require_fixed(config)?;
        let (manager, metrics) = FixedStackManager::new_benchmarked(config)?;
        Ok((
            Self {
                backend: StackBackend::Fixed(manager),
            },
            metrics,
        ))
    }

    /// Creates an instrumented Fixed Stack manager using two-phase syscalls.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not Fixed or the manager cannot start.
    pub fn new_benchmarked_two_phase(
        config: StackConfig,
    ) -> Result<(Self, Arc<StackBenchmarkMetrics>), StackError> {
        require_fixed(config)?;
        let (manager, metrics) = FixedStackManager::new_benchmarked_two_phase(config)?;
        Ok((
            Self {
                backend: StackBackend::Fixed(manager),
            },
            metrics,
        ))
    }

    /// Creates a timing-counter view over a Fixed Stack manager's allocator.
    ///
    /// # Errors
    ///
    /// Returns an error when this manager selected another production engine.
    pub fn benchmark_worker(&self) -> Result<(Self, Arc<StackBenchmarkMetrics>), StackError> {
        match &self.backend {
            StackBackend::Fixed(manager) => {
                let (manager, metrics) = manager.benchmark_worker();
                Ok((
                    Self {
                        backend: StackBackend::Fixed(manager),
                    },
                    metrics,
                ))
            }
            StackBackend::GoInspired(_) => Err(StackError::BenchmarkRequiresFixedEngine {
                engine: StackEngine::GoInspired,
            }),
        }
    }
}

fn require_fixed(config: StackConfig) -> Result<(), StackError> {
    match config.engine() {
        StackEngine::Fixed => Ok(()),
        engine => Err(StackError::BenchmarkRequiresFixedEngine { engine }),
    }
}
