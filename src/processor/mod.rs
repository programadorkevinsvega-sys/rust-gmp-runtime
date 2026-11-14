//! Processor resources and their local runnable queues.

mod error;
mod id;
#[path = "processor.rs"]
mod implementation;
mod local_queue;
mod processor_pool;

pub use error::ProcessorError;
pub use id::ProcessorId;
pub use implementation::{Processor, ProcessorBinding, ProcessorState};
pub use processor_pool::ProcessorPool;

#[cfg(test)]
mod tests;
