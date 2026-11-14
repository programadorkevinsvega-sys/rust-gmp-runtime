use std::panic::UnwindSafe;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::routine::Routine;

use super::super::identity::MachineId;
use super::super::join::RoutineJoinError;

pub(super) enum WorkerCommand {
    Run(DispatchTask),
    Stop,
}

pub(super) struct DispatchTask {
    pub(super) routine: Routine,
    entry: Option<Box<dyn FnOnce() + Send + UnwindSafe>>,
    completion: Sender<Result<(), RoutineJoinError>>,
}

impl DispatchTask {
    pub(super) fn new<F>(
        machine_id: MachineId,
        routine: Routine,
        entry: F,
    ) -> (Self, crate::machine::RoutineJoinHandle)
    where
        F: FnOnce() + Send + UnwindSafe + 'static,
    {
        let (completion, receiver) = mpsc::channel();
        let routine_id = routine.id();
        (
            Self {
                routine,
                entry: Some(Box::new(entry)),
                completion,
            },
            crate::machine::RoutineJoinHandle::new(machine_id, routine_id, receiver),
        )
    }

    pub(super) fn take_entry(&mut self) -> Option<Box<dyn FnOnce() + Send + UnwindSafe>> {
        self.entry.take()
    }

    pub(super) fn complete(self, outcome: Result<(), RoutineJoinError>) {
        if let Err(error) = self.completion.send(outcome) {
            drop(error.0);
        }
    }
}

pub(super) fn command_channel() -> (Sender<WorkerCommand>, Receiver<WorkerCommand>) {
    mpsc::channel()
}
