//! Task scheduling.

#[path = "scheduler.rs"]
mod implementation;

pub(crate) use implementation::Scheduler;
#[cfg(feature = "scheduler-benchmark")]
pub(crate) use implementation::StealEvent;
