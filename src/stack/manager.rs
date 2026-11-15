use super::allocator::FixedStackManager;
use super::config::{StackConfig, StackEngine};
use super::error::StackError;
use super::go_inspired::GoInspiredStackManager;
use super::go_inspired::ProcessorStackCache;
use super::lease::Stack;
#[path = "manager/metrics.rs"]
mod metrics;

#[path = "manager/operations.rs"]
mod operations;

#[cfg(feature = "allocator-benchmark")]
#[path = "manager/benchmark.rs"]
mod benchmark;

#[derive(Clone, Debug)]
enum StackBackend {
    Fixed(FixedStackManager),
    GoInspired(GoInspiredStackManager),
}

/// Shared production facade for selectable guarded-stack allocators.
#[derive(Clone, Debug)]
pub struct StackManager {
    backend: StackBackend,
}

impl StackManager {
    /// Creates a manager for the engine selected in `config`.
    ///
    /// `Fixed` remains the default. Go-inspired uses its global pool in this
    /// caller-side allocation path; it does not claim affinity to a Processor.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected backend, platform, configuration, or
    /// initial system observation is invalid.
    pub fn new(config: StackConfig) -> Result<Self, StackError> {
        let backend = match config.engine() {
            StackEngine::Fixed => StackBackend::Fixed(FixedStackManager::new(config)?),
            StackEngine::GoInspired => {
                StackBackend::GoInspired(GoInspiredStackManager::new(config)?)
            }
        };
        Ok(Self { backend })
    }

    /// Creates a Go-inspired manager with explicit cache and span sizing.
    ///
    /// The configuration must select [`StackEngine::GoInspired`].
    ///
    /// # Errors
    ///
    /// Returns an error if another engine was selected or allocator setup fails.
    pub fn with_go_inspired_options(
        config: StackConfig,
        options: super::go_inspired::GoInspiredOptions,
    ) -> Result<Self, StackError> {
        if config.engine() != StackEngine::GoInspired {
            return Err(StackError::StackEngineMismatch {
                expected: StackEngine::GoInspired,
                selected: config.engine(),
            });
        }
        Ok(Self {
            backend: StackBackend::GoInspired(GoInspiredStackManager::with_options(
                config, options,
            )?),
        })
    }

    /// Allocates a guarded stack through the selected production engine.
    ///
    /// # Errors
    ///
    /// Returns backend admission, mapping, protection, activation, accounting,
    /// or synchronization errors.
    pub fn allocate(&self) -> Result<Stack, StackError> {
        match &self.backend {
            StackBackend::Fixed(manager) => manager.allocate().map(Stack::fixed),
            StackBackend::GoInspired(manager) => manager.allocate().map(Stack::go_inspired),
        }
    }

    /// Creates a cache intended to be owned exclusively by one Processor.
    ///
    /// # Errors
    ///
    /// Returns an error unless this manager uses the Go-inspired engine or
    /// cache metadata allocation fails.
    pub fn processor_cache(&self) -> Result<ProcessorStackCache, StackError> {
        match &self.backend {
            StackBackend::Fixed(_) => Err(StackError::StackEngineMismatch {
                expected: StackEngine::GoInspired,
                selected: StackEngine::Fixed,
            }),
            StackBackend::GoInspired(manager) => manager.processor_cache(),
        }
    }

    /// Allocates a Go-inspired stack from a Processor-owned cache.
    ///
    /// The caller is responsible for using the cache only from its owning
    /// Processor. A cache from another manager safely falls back to this
    /// manager's global pool.
    ///
    /// # Errors
    ///
    /// Returns an error unless this manager uses the Go-inspired engine or
    /// cache allocation/refill fails.
    pub fn allocate_cached(&self, cache: &mut ProcessorStackCache) -> Result<Stack, StackError> {
        match &self.backend {
            StackBackend::Fixed(_) => Err(StackError::StackEngineMismatch {
                expected: StackEngine::GoInspired,
                selected: StackEngine::Fixed,
            }),
            StackBackend::GoInspired(manager) => {
                manager.allocate_cached(cache).map(Stack::go_inspired)
            }
        }
    }

    /// Returns the operating-system page size used by this engine.
    pub fn page_size(&self) -> usize {
        match &self.backend {
            StackBackend::Fixed(manager) => manager.page_size(),
            StackBackend::GoInspired(manager) => manager.page_size(),
        }
    }

    /// Returns the requested usable size before page alignment.
    pub fn requested_usable_size(&self) -> usize {
        match &self.backend {
            StackBackend::Fixed(manager) => manager.requested_usable_size(),
            StackBackend::GoInspired(manager) => manager.requested_usable_size(),
        }
    }

    /// Returns the page-aligned usable stack size.
    pub fn usable_size(&self) -> usize {
        match &self.backend {
            StackBackend::Fixed(manager) => manager.usable_size(),
            StackBackend::GoInspired(manager) => manager.usable_size(),
        }
    }
}
