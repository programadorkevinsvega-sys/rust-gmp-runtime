//! Native worker threads and their Machine lifecycle.

mod error;
mod identity;
mod join;
#[path = "machine.rs"]
mod representation;
mod thread_manager;
mod worker;

pub use error::MachineError;
pub use identity::MachineId;
pub use join::{RoutineJoinError, RoutineJoinHandle};
pub use representation::{MachineSnapshot, MachineState};
pub use thread_manager::MachineManager;

#[cfg(test)]
mod tests;
