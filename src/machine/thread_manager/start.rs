use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use crate::processor::ProcessorPool;
use crate::scheduler::Scheduler;
use crate::stack::{StackEngine, StackManager};

use super::MachineManager;
use crate::machine::worker::Worker;
use crate::machine::{MachineError, MachineId};

impl MachineManager {
    /// Starts one parked native worker for each Processor in the pool.
    ///
    /// Startup is transactional: if spawning or binding any worker fails,
    /// already-started workers are stopped and joined before returning.
    ///
    /// # Errors
    ///
    /// Returns an error on thread creation, Processor binding, worker startup,
    /// or rollback failure.
    pub fn start(processors: Arc<ProcessorPool>) -> Result<Self, MachineError> {
        Self::start_configured(processors, None, false)
    }

    /// Starts workers with a shared stack manager for routines created by workers.
    ///
    /// Routines dispatched through [`Self::dispatch_new`] allocate from the
    /// manager's global path and are created on the worker that executes them.
    ///
    /// # Errors
    ///
    /// Returns an error on thread creation, Processor binding, worker startup,
    /// or rollback failure.
    pub fn start_with_stack_manager(
        processors: Arc<ProcessorPool>,
        stack_manager: Arc<StackManager>,
    ) -> Result<Self, MachineError> {
        Self::start_configured(processors, Some(stack_manager), false)
    }

    /// Starts workers with exclusive Go-inspired stack caches, one per Processor.
    ///
    /// Caches are created and owned by their worker threads. Routines submitted
    /// through [`Self::dispatch_new`] acquire and release their stack on the
    /// executing worker's cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not Go-inspired or worker startup fails.
    pub fn start_with_processor_caches(
        processors: Arc<ProcessorPool>,
        stack_manager: Arc<StackManager>,
    ) -> Result<Self, MachineError> {
        if stack_manager.engine() != StackEngine::GoInspired {
            return Err(MachineError::ProcessorCachesRequireGoInspired);
        }
        Self::start_configured(processors, Some(stack_manager), true)
    }

    fn start_configured(
        processors: Arc<ProcessorPool>,
        stack_manager: Option<Arc<StackManager>>,
        processor_caches: bool,
    ) -> Result<Self, MachineError> {
        if processors.is_empty() {
            return Err(MachineError::EmptyProcessorPool);
        }
        let scheduler = Arc::new(Scheduler::new(processors.len())?);
        let mut manager = Self {
            processors,
            scheduler,
            workers: Vec::new(),
            next_worker: AtomicUsize::new(0),
            stack_manager,
            processor_caches,
        };
        let processor_ids = manager
            .processors
            .iter()
            .map(|processor| processor.id())
            .collect::<Vec<_>>();
        for processor_id in processor_ids {
            let id = MachineId::new(manager.workers.len());
            let worker = match Worker::spawn(
                id,
                processor_id,
                Arc::clone(&manager.processors),
                Arc::clone(&manager.scheduler),
                manager.stack_manager.as_ref().map(Arc::clone),
                manager.processor_caches,
            ) {
                Ok(worker) => worker,
                Err(error) => return Err(manager.rollback(error)),
            };
            manager.workers.push(worker);
            let startup = match manager.workers.last() {
                Some(worker) => worker.wait_started(),
                None => Err(MachineError::EmptyProcessorPool),
            };
            if let Err(error) = startup {
                return Err(manager.rollback(error));
            }
        }
        Ok(manager)
    }
}
