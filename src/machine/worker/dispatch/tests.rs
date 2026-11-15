use std::error::Error;

use crate::machine::{MachineId, RoutineJoinError};
use crate::routine::{Routine, RoutineCompletion, RoutineMetadata, RoutineState};
use crate::stack::{StackConfig, StackManager};

use super::DispatchTask;

#[test]
fn dropping_a_queued_task_discards_it_and_resolves_its_join_handle() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(8 * 1024)?)?;
    let routine = Routine::new(&stack_manager, RoutineMetadata::new())?;
    routine.publish()?;
    let (task, handle) = DispatchTask::new(MachineId::new(3), routine.clone(), || {});

    drop(task);

    assert!(matches!(
        handle.join(),
        Err(RoutineJoinError::WorkerFailed { .. })
    ));
    assert_eq!(routine.state()?, RoutineState::Dead);
    assert_eq!(routine.completion()?, Some(RoutineCompletion::Abandoned));
    Ok(())
}
