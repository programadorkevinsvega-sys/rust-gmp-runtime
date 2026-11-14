//! Probes that `panic = "abort"` terminates instead of returning a panic payload.

use std::error::Error;

use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

fn main() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let routine = Routine::new(&manager, RoutineMetadata::new())?;
    routine.publish()?;
    routine.run_once(|| std::panic::resume_unwind(Box::new("panic=abort is not recoverable")))?;
    Ok(())
}
