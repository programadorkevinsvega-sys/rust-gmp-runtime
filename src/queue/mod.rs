//! Runtime scheduling queues.

mod global;
mod injection;
mod local;

pub(crate) use global::GlobalRunQueue;
pub(crate) use injection::InjectionQueue;
#[cfg(feature = "scheduler-benchmark")]
pub(crate) use local::LOCAL_QUEUE_CAPACITY;
pub(crate) use local::LocalRunQueue;
