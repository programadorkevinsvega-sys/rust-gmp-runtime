use std::panic::UnwindSafe;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::routine::{Routine, RoutineState};

use super::super::identity::MachineId;
use super::super::join::RoutineJoinError;

pub(super) enum WorkerCommand {
    Run(DispatchTask),
    Stop,
}

pub(super) struct DispatchTask {
    machine_id: MachineId,
    pub(super) routine: Routine,
    entry: Option<Box<dyn FnOnce() + Send + UnwindSafe>>,
    completion: Option<Sender<Result<(), RoutineJoinError>>>,
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
                machine_id,
                routine,
                entry: Some(Box::new(entry)),
                completion: Some(completion),
            },
            crate::machine::RoutineJoinHandle::new(machine_id, routine_id, receiver),
        )
    }

    pub(super) fn take_entry(&mut self) -> Option<Box<dyn FnOnce() + Send + UnwindSafe>> {
        self.entry.take()
    }

    pub(super) fn complete(mut self, outcome: Result<(), RoutineJoinError>) {
        self.send_completion(outcome);
    }

    pub(super) fn fail(mut self, machine_id: MachineId, message: String) {
        self.fail_if_pending(machine_id, message);
    }

    fn send_completion(&mut self, outcome: Result<(), RoutineJoinError>) {
        if let Some(completion) = self.completion.take() {
            if let Err(error) = completion.send(outcome) {
                drop(error.0);
            }
        }
    }

    fn fail_if_pending(&mut self, machine_id: MachineId, message: String) {
        if self.completion.is_none() {
            return;
        }
        let message = match self.routine.state() {
            Ok(RoutineState::Created | RoutineState::Runnable) => match self.routine.discard() {
                Ok(()) => message,
                Err(error) => format!("{message}; routine rollback failed: {error}"),
            },
            Ok(_) => message,
            Err(error) => format!("{message}; routine state unavailable: {error}"),
        };
        self.send_completion(Err(RoutineJoinError::WorkerFailed {
            machine_id,
            routine_id: self.routine.id(),
            message,
        }));
    }
}

impl Drop for DispatchTask {
    fn drop(&mut self) {
        self.fail_if_pending(
            self.machine_id,
            "worker exited before dispatch completed".into(),
        );
    }
}

pub(super) fn command_channel() -> (Sender<WorkerCommand>, Receiver<WorkerCommand>) {
    mpsc::channel()
}

#[cfg(test)]
#[path = "dispatch/tests.rs"]
mod tests;
