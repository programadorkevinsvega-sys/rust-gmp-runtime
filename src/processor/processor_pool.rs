use std::num::NonZeroUsize;
use std::thread;

use super::ProcessorError;
use super::id::ProcessorId;
use super::implementation::Processor;

/// Configurable collection of Processor execution capacities.
#[derive(Debug)]
pub struct ProcessorPool {
    processors: Vec<Processor>,
}

impl ProcessorPool {
    /// Creates a pool with the requested nonzero number of Processors.
    ///
    /// # Errors
    ///
    /// Returns an error if `count` is zero.
    pub fn new(count: usize) -> Result<Self, ProcessorError> {
        let count = NonZeroUsize::new(count).ok_or(ProcessorError::EmptyPool)?;
        Ok(Self::with_count(count))
    }

    /// Creates a pool sized to the OS-reported logical parallelism.
    ///
    /// No fallback is applied if the OS cannot report this value.
    ///
    /// # Errors
    ///
    /// Returns an error if logical parallelism is unavailable.
    pub fn default_for_host() -> Result<Self, ProcessorError> {
        let count =
            thread::available_parallelism().map_err(ProcessorError::ParallelismUnavailable)?;
        Ok(Self::with_count(count))
    }

    fn with_count(count: NonZeroUsize) -> Self {
        let processors = (0..count.get())
            .map(|index| Processor::new(ProcessorId::new(index)))
            .collect();
        Self { processors }
    }

    /// Returns the number of Processors in this pool.
    pub fn len(&self) -> usize {
        self.processors.len()
    }

    /// Returns whether this pool contains no Processors.
    pub fn is_empty(&self) -> bool {
        self.processors.is_empty()
    }

    /// Returns a Processor by its pool-local identifier.
    pub fn get(&self, id: ProcessorId) -> Option<&Processor> {
        self.processors.get(id.index())
    }

    /// Iterates over Processors in stable pool-local index order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Processor> {
        self.processors.iter()
    }
}
