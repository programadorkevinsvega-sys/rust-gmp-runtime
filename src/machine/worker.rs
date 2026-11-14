use std::cell::Cell;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::processor::{ProcessorId, ProcessorPool};

use super::error::MachineError;
use super::identity::MachineId;
use super::representation::MachineSnapshot;

#[path = "worker/control.rs"]
mod control;

pub(super) use control::WorkerControl;

thread_local! {
    static CURRENT_MACHINE_ID: Cell<Option<MachineId>> = const { Cell::new(None) };
}

#[derive(Debug)]
pub(super) struct Worker {
    pub(super) id: MachineId,
    control: Arc<WorkerControl>,
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
        let join = thread::Builder::new()
            .name(format!("rust-gmp-machine-{}", id.index()))
            .spawn(move || run_worker(id, processor_id, processors, Arc::clone(&thread_control)))
            .map_err(|source| MachineError::ThreadSpawn {
                machine_id: id,
                source,
            })?;
        Ok(Self {
            id,
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

    pub(super) fn request_stop(&self) -> Result<(), MachineError> {
        self.control.request_stop()
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

fn run_worker(
    id: MachineId,
    processor_id: ProcessorId,
    processors: Arc<ProcessorPool>,
    control: Arc<WorkerControl>,
) -> Result<(), MachineError> {
    let result = worker_loop(id, processor_id, &processors, &control);
    if let Err(error) = &result {
        control.mark_failed(error.to_string())?;
    }
    result
}

fn worker_loop(
    id: MachineId,
    processor_id: ProcessorId,
    processors: &ProcessorPool,
    control: &WorkerControl,
) -> Result<(), MachineError> {
    register_current_machine(id)?;
    let processor = processors
        .get(processor_id)
        .ok_or(MachineError::EmptyProcessorPool)?;
    let binding = match processor.bind_current() {
        Ok(binding) => binding,
        Err(error) => {
            control.mark_failed(error.to_string())?;
            return Ok(());
        }
    };
    control.mark_idle(thread::current().id(), current_machine_id())?;
    control.wait_for_stop()?;
    drop(binding);
    control.mark_terminated()?;
    Ok(())
}

fn register_current_machine(id: MachineId) -> Result<(), MachineError> {
    CURRENT_MACHINE_ID.with(|current| match current.get() {
        Some(previous) => Err(MachineError::WorkerFailed {
            machine_id: id,
            message: format!(
                "worker thread already registered as Machine {}",
                previous.index()
            ),
        }),
        None => {
            current.set(Some(id));
            Ok(())
        }
    })
}

fn current_machine_id() -> Option<MachineId> {
    CURRENT_MACHINE_ID.with(Cell::get)
}
