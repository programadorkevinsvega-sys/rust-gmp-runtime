use std::panic::UnwindSafe;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

use crate::processor::{ProcessorId, ProcessorPool};

use super::error::MachineError;
use super::identity::MachineId;
use super::representation::MachineSnapshot;

#[path = "worker/control.rs"]
mod control;
#[path = "worker/dispatch.rs"]
mod dispatch;
#[path = "worker/execution.rs"]
mod execution;

pub(super) use control::WorkerControl;
use dispatch::{DispatchTask, WorkerCommand, command_channel};

#[derive(Debug)]
pub(super) struct Worker {
    pub(super) id: MachineId,
    control: Arc<WorkerControl>,
    commands: Sender<WorkerCommand>,
    join: Option<JoinHandle<Result<(), MachineError>>>,
}

impl Worker {
    pub(super) fn spawn(
        id: MachineId,
        processor_id: ProcessorId,
        processors: Arc<ProcessorPool>,
    ) -> Result<Self, MachineError> {
        let control = Arc::new(WorkerControl::new(id, processor_id));
        let thread_control = Arc::clone(&control);
        let (commands, receiver) = command_channel();
        let join = thread::Builder::new()
            .name(format!("rust-gmp-machine-{}", id.index()))
            .spawn(move || {
                execution::run_worker(
                    id,
                    processor_id,
                    processors,
                    Arc::clone(&thread_control),
                    receiver,
                )
            })
            .map_err(|source| MachineError::ThreadSpawn {
                machine_id: id,
                source,
            })?;
        Ok(Self {
            id,
            control,
            commands,
            join: Some(join),
        })
    }

    pub(super) fn wait_started(&self) -> Result<(), MachineError> {
        self.control.wait_started()
    }

    pub(super) fn snapshot(&self) -> Result<MachineSnapshot, MachineError> {
        self.control.snapshot()
    }

    pub(super) fn request_stop(&self) -> Result<(), MachineError> {
        if self.join.is_none() || !self.control.request_stop()? {
            return Ok(());
        }
        self.commands
            .send(WorkerCommand::Stop)
            .map_err(|error| MachineError::WorkerChannelClosed {
                machine_id: self.id,
                message: error.to_string(),
            })
    }

    pub(super) fn dispatch<F>(
        &self,
        routine: crate::routine::Routine,
        entry: F,
    ) -> Result<crate::machine::RoutineJoinHandle, (MachineError, crate::routine::Routine)>
    where
        F: FnOnce() + Send + UnwindSafe + 'static,
    {
        let routine_id = routine.id();
        let rollback_routine = routine.clone();
        let (task, handle) = DispatchTask::new(self.id, routine, entry);
        match self.commands.send(WorkerCommand::Run(task)) {
            Ok(()) => Ok(handle),
            Err(error) => {
                let message = match error.0 {
                    WorkerCommand::Run(_) => "worker command channel is closed",
                    WorkerCommand::Stop => "worker returned an unexpected stop command",
                };
                Err((
                    MachineError::DispatchRejected {
                        machine_id: self.id,
                        routine_id,
                        message: message.into(),
                    },
                    rollback_routine,
                ))
            }
        }
    }

    pub(super) fn join(&mut self) -> Result<(), MachineError> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        match join.join() {
            Ok(result) => result,
            Err(_) => Err(MachineError::WorkerPanicked {
                machine_id: self.id,
            }),
        }
    }

    pub(super) fn is_joinable(&self) -> bool {
        self.join.is_some()
    }
}
