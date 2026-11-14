use std::collections::{HashSet, VecDeque};

use crate::routine::{Routine, RoutineId, RoutineState};

use super::ProcessorError;

#[derive(Debug, Default)]
pub(super) struct LocalQueue {
    routines: VecDeque<Routine>,
    ids: HashSet<RoutineId>,
}

impl LocalQueue {
    pub(super) fn push(&mut self, routine: &Routine) -> Result<(), ProcessorError> {
        let id = routine.id();
        let state = routine.state()?;
        if state != RoutineState::Runnable {
            return Err(ProcessorError::RoutineNotRunnable {
                routine_id: id,
                state,
            });
        }
        if !self.ids.insert(id) {
            return Err(ProcessorError::DuplicateRoutine(id));
        }
        self.routines.push_back(routine.clone());
        Ok(())
    }

    pub(super) fn pop(&mut self) -> Result<Option<Routine>, ProcessorError> {
        let Some(routine) = self.routines.pop_front() else {
            return Ok(None);
        };
        if !self.ids.remove(&routine.id()) {
            return Err(ProcessorError::QueueInvariantViolation(routine.id()));
        }
        Ok(Some(routine))
    }

    pub(super) fn len(&self) -> usize {
        self.routines.len()
    }
}
