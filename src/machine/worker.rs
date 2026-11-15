use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::processor::{ProcessorId, ProcessorPool};

use super::error::MachineError;
use super::identity::MachineId;
use super::representation::MachineSnapshot;
use crate::scheduler::Scheduler;
use crate::stack::StackManager;

#[path = "worker/control.rs"]
mod control;
pub(crate) mod dispatch;
#[path = "worker/execution.rs"]
mod execution;

pub(super) use control::WorkerControl;

#[derive(Debug)]
pub(super) struct Worker {
    pub(super) id: MachineId,
    pub(super) processor_id: ProcessorId,
    control: Arc<WorkerControl>,
    join: Option<JoinHandle<Result<(), MachineError>>>,
}

impl Worker {
    pub(super) fn spawn(
        id: MachineId,
        processor_id: ProcessorId,
        processors: Arc<ProcessorPool>,
        scheduler: Arc<Scheduler>,
        stack_manager: Option<Arc<StackManager>>,
        processor_caches: bool,
    ) -> Result<Self, MachineError> {
        let control = Arc::new(WorkerControl::new(id, processor_id));
        let thread_control = Arc::clone(&control);
        let join = thread::Builder::new()
            .name(format!("rust-gmp-machine-{}", id.index()))
            .spawn(move || {
                execution::run_worker(
                    id,
                    processor_id,
                    processors,
                    scheduler,
                    stack_manager,
                    processor_caches,
                    Arc::clone(&thread_control),
                )
            })
            .map_err(|source| MachineError::ThreadSpawn {
                machine_id: id,
                source,
            })?;
        Ok(Self {
            id,
            processor_id,
            control,
            join: Some(join),
        })
    }

    pub(super) fn wait_started(&self) -> Result<(), MachineError> {
        self.control.wait_started()
    }

    pub(super) fn snapshot(&self) -> Result<MachineSnapshot, MachineError> {
        self.control.snapshot()
    }

    pub(super) fn request_stop(&self) -> Result<bool, MachineError> {
        if self.join.is_none() {
            return Ok(false);
        }
        self.control.request_stop()
    }

    pub(super) fn join(&mut self) -> Result<(), MachineError> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        match join.join() {
            Ok(result) => result,
            Err(payload) => Err(MachineError::WorkerPanicked {
                machine_id: self.id,
                message: Self::panic_payload_summary(payload),
            }),
        }
    }

    fn panic_payload_summary(payload: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = payload.downcast_ref::<String>() {
            return message.clone();
        }
        if let Some(message) = payload.downcast_ref::<&'static str>() {
            return (*message).to_owned();
        }
        "non-string panic payload".to_owned()
    }

    pub(super) fn is_joinable(&self) -> bool {
        self.join.is_some()
    }
}
