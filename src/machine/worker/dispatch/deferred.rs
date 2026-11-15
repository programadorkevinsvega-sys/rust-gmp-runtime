use std::panic::UnwindSafe;
use std::sync::mpsc;

use crate::routine::{Routine, RoutineError, RoutineId, RoutineMetadata};
use crate::stack::{ProcessorStackCache, StackManager};

use super::DispatchTask;
use crate::machine::MachineId;

impl DispatchTask {
    pub(crate) fn new_on_worker<F>(
        machine_id: MachineId,
        routine_id: RoutineId,
        metadata: RoutineMetadata,
        entry: F,
        return_stack_to_cache: bool,
    ) -> (Self, crate::machine::RoutineJoinHandle)
    where
        F: FnOnce() + Send + UnwindSafe + 'static,
    {
        let (completion, receiver) = mpsc::channel();
        (
            Self {
                machine_id,
                routine: None,
                routine_id,
                pending_routine: Some(metadata),
                return_stack_to_cache,
                entry: Some(Box::new(entry)),
                completion: Some(completion),
            },
            crate::machine::RoutineJoinHandle::new(machine_id, routine_id, receiver),
        )
    }

    pub(crate) fn prepare_routine(
        &mut self,
        stack_manager: &StackManager,
        cache: Option<&mut ProcessorStackCache>,
    ) -> Result<(), RoutineError> {
        let Some(metadata) = self.pending_routine.take() else {
            return Ok(());
        };
        let routine = Routine::new_with_id(stack_manager, self.routine_id, metadata, cache)?;
        routine.reserve_for_dispatch()?;
        self.routine = Some(routine);
        Ok(())
    }

    pub(crate) fn release_stack_to_cache(
        &self,
        cache: &mut ProcessorStackCache,
    ) -> Result<(), RoutineError> {
        if self.return_stack_to_cache {
            self.routine
                .as_ref()
                .ok_or(RoutineError::DispatchReserved)?
                .release_stack_to_processor_cache(cache)?;
        }
        Ok(())
    }

    pub(crate) fn needs_preparation(&self) -> bool {
        self.pending_routine.is_some()
    }

    pub(crate) fn returns_stack_to_cache(&self) -> bool {
        self.return_stack_to_cache
    }
}
