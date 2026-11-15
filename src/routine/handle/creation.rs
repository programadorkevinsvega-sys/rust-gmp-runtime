use std::sync::Arc;

use super::{Routine, RoutineInner};
use crate::routine::context_storage::RoutineContext;
use crate::routine::lifecycle::RoutineLifecycle;
use crate::routine::{RoutineError, RoutineId, RoutineMetadata};
use crate::stack::{ProcessorStackCache, StackManager};

impl Routine {
    /// Allocates a routine identity and its stack, initially in `Created`.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier cannot be allocated or the stack
    /// manager cannot reserve a stack.
    pub fn new(
        stack_manager: &StackManager,
        metadata: RoutineMetadata,
    ) -> Result<Self, RoutineError> {
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        let creation_started = std::time::Instant::now();
        let id = RoutineId::allocate()?;
        Self::new_with_id_started(
            stack_manager,
            id,
            metadata,
            None,
            #[cfg(feature = "runtime-benchmark-instrumentation")]
            creation_started,
        )
    }

    pub(crate) fn new_with_id(
        stack_manager: &StackManager,
        id: RoutineId,
        metadata: RoutineMetadata,
        cache: Option<&mut ProcessorStackCache>,
    ) -> Result<Self, RoutineError> {
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        let creation_started = std::time::Instant::now();
        Self::new_with_id_started(
            stack_manager,
            id,
            metadata,
            cache,
            #[cfg(feature = "runtime-benchmark-instrumentation")]
            creation_started,
        )
    }

    fn new_with_id_started(
        stack_manager: &StackManager,
        id: RoutineId,
        metadata: RoutineMetadata,
        cache: Option<&mut ProcessorStackCache>,
        #[cfg(feature = "runtime-benchmark-instrumentation")] creation_started: std::time::Instant,
    ) -> Result<Self, RoutineError> {
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        let stack_started = std::time::Instant::now();
        let stack = match cache {
            Some(cache) => stack_manager.allocate_cached(cache)?,
            None => stack_manager.allocate()?,
        };
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        let stack_acquisition = stack_started.elapsed();
        Self::from_stack(
            id,
            metadata,
            stack,
            #[cfg(feature = "runtime-benchmark-instrumentation")]
            creation_started,
            #[cfg(feature = "runtime-benchmark-instrumentation")]
            stack_acquisition,
        )
    }

    /// Allocates a routine using the cache owned by the current Processor.
    ///
    /// The caller must ensure this method and later cache release occur on the
    /// Processor that owns `cache`. Fixed stack managers do not support this
    /// allocation path.
    ///
    /// # Errors
    ///
    /// Returns an error if the routine identifier, stack lease, context, or
    /// cache allocation cannot be created.
    pub fn new_with_processor_cache(
        stack_manager: &StackManager,
        cache: &mut ProcessorStackCache,
        metadata: RoutineMetadata,
    ) -> Result<Self, RoutineError> {
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        let creation_started = std::time::Instant::now();
        let id = RoutineId::allocate()?;
        Self::new_with_id_started(
            stack_manager,
            id,
            metadata,
            Some(cache),
            #[cfg(feature = "runtime-benchmark-instrumentation")]
            creation_started,
        )
    }

    fn from_stack(
        id: RoutineId,
        metadata: RoutineMetadata,
        stack: crate::stack::Stack,
        #[cfg(feature = "runtime-benchmark-instrumentation")] creation_started: std::time::Instant,
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        stack_acquisition: std::time::Duration,
    ) -> Result<Self, RoutineError> {
        let context = RoutineContext::new(stack.bounds())?;
        let routine = Self {
            inner: Arc::new(RoutineInner {
                id,
                metadata,
                stack: std::sync::Mutex::new(stack),
                context: std::sync::Mutex::new(context),
                lifecycle: std::sync::Mutex::new(RoutineLifecycle::new()),
                #[cfg(feature = "runtime-benchmark-instrumentation")]
                benchmark: super::super::benchmark::RoutineBenchmark::default(),
            }),
        };
        #[cfg(feature = "runtime-benchmark-instrumentation")]
        routine
            .inner
            .benchmark
            .record_creation(creation_started.elapsed(), stack_acquisition);
        Ok(routine)
    }

    /// Returns per-routine stage timings collected by the benchmark-only feature.
    #[cfg(feature = "runtime-benchmark-instrumentation")]
    #[doc(hidden)]
    pub fn benchmark_timing(&self) -> super::super::RoutineBenchmarkTiming {
        self.inner.benchmark.snapshot()
    }
}
