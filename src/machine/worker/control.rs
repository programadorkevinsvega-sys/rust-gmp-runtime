use std::sync::{Condvar, Mutex};
use std::thread::ThreadId;

use crate::processor::ProcessorId;

use super::super::error::MachineError;
use super::super::identity::MachineId;
use super::super::representation::{MachineSnapshot, MachineState};

#[derive(Debug)]
struct WorkerStatus {
    state: MachineState,
    thread_id: Option<ThreadId>,
    tls_machine_id: Option<MachineId>,
    stop_requested: bool,
}

#[derive(Debug)]
pub(in crate::machine) struct WorkerControl {
    id: MachineId,
    processor_id: ProcessorId,
    status: Mutex<WorkerStatus>,
    changed: Condvar,
}

impl WorkerControl {
    pub(super) fn new(id: MachineId, processor_id: ProcessorId) -> Self {
        Self {
            id,
            processor_id,
            status: Mutex::new(WorkerStatus {
                state: MachineState::Starting,
                thread_id: None,
                tls_machine_id: None,
                stop_requested: false,
            }),
            changed: Condvar::new(),
        }
    }

    pub(super) fn mark_idle(
        &self,
        thread_id: ThreadId,
        tls_machine_id: Option<MachineId>,
    ) -> Result<(), MachineError> {
        let mut status = self.lock()?;
        status.thread_id = Some(thread_id);
        status.tls_machine_id = tls_machine_id;
        status.state = if status.stop_requested {
            MachineState::Stopping
        } else {
            MachineState::Idle
        };
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn mark_failed(&self, message: String) -> Result<(), MachineError> {
        let mut status = self.lock()?;
        status.state = MachineState::Failed { message };
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn mark_terminated(&self) -> Result<(), MachineError> {
        let mut status = self.lock()?;
        status.state = MachineState::Terminated;
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn wait_started(&self) -> Result<(), MachineError> {
        let mut status = self.lock()?;
        while status.state == MachineState::Starting {
            status = self
                .changed
                .wait(status)
                .map_err(|error| self.poisoned(error.to_string()))?;
        }
        match &status.state {
            MachineState::Idle => Ok(()),
            MachineState::Stopping if status.thread_id.is_some() => Ok(()),
            MachineState::Failed { message } => Err(MachineError::WorkerFailed {
                machine_id: self.id,
                message: message.clone(),
            }),
            state
            @ (MachineState::Starting | MachineState::Stopping | MachineState::Terminated) => {
                Err(MachineError::WorkerFailed {
                    machine_id: self.id,
                    message: format!("unexpected startup state {state:?}"),
                })
            }
        }
    }

    pub(super) fn wait_for_stop(&self) -> Result<(), MachineError> {
        let mut status = self.lock()?;
        while !status.stop_requested {
            status = self
                .changed
                .wait(status)
                .map_err(|error| self.poisoned(error.to_string()))?;
        }
        Ok(())
    }

    pub(super) fn request_stop(&self) -> Result<(), MachineError> {
        let mut status = match self.status.lock() {
            Ok(status) => status,
            Err(error) => {
                let mut status = error.into_inner();
                status.stop_requested = true;
                status.state = MachineState::Stopping;
                self.changed.notify_all();
                return Err(self.poisoned("stop requested after recovering poisoned state".into()));
            }
        };
        match status.state {
            MachineState::Terminated | MachineState::Failed { .. } => return Ok(()),
            MachineState::Stopping => {}
            MachineState::Starting | MachineState::Idle => {
                status.state = MachineState::Stopping;
            }
        }
        status.stop_requested = true;
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn snapshot(&self) -> Result<MachineSnapshot, MachineError> {
        let status = self.lock()?;
        Ok(MachineSnapshot {
            id: self.id,
            processor_id: self.processor_id,
            thread_id: status.thread_id,
            tls_machine_id: status.tls_machine_id,
            state: status.state.clone(),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, WorkerStatus>, MachineError> {
        self.status
            .lock()
            .map_err(|error| self.poisoned(error.to_string()))
    }

    fn poisoned(&self, message: String) -> MachineError {
        MachineError::ControlPoisoned {
            machine_id: self.id,
            message,
        }
    }
}
