use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;

use crate::machine::{MachineError, MachineManager, MachineState};
use crate::processor::{ProcessorPool, ProcessorState};

#[test]
fn workers_bind_distinct_processors_and_register_machine_identity_in_thread_tls()
-> Result<(), Box<dyn Error>> {
    let processors = Arc::new(ProcessorPool::new(3)?);
    let manager = MachineManager::start(Arc::clone(&processors))?;
    let snapshots = manager.machines()?;

    assert_eq!(snapshots.len(), 3);
    assert!(
        snapshots
            .iter()
            .all(|machine| machine.state == MachineState::Idle)
    );
    assert!(snapshots.iter().all(|machine| {
        machine.thread_id.is_some() && machine.tls_machine_id == Some(machine.id)
    }));
    let threads = snapshots
        .iter()
        .filter_map(|machine| machine.thread_id)
        .collect::<HashSet<_>>();
    assert_eq!(threads.len(), 3);

    for machine in &snapshots {
        let processor = processors
            .get(machine.processor_id)
            .ok_or_else(|| std::io::Error::other("assigned Processor missing"))?;
        assert_eq!(
            processor.state()?,
            ProcessorState::Owned {
                thread_id: machine
                    .thread_id
                    .ok_or_else(|| std::io::Error::other("worker thread id missing"))?
            }
        );
    }

    drop(manager);
    for processor in processors.iter() {
        assert_eq!(processor.state()?, ProcessorState::Unowned);
    }
    Ok(())
}

#[test]
fn shutdown_wakes_and_joins_every_worker() -> Result<(), Box<dyn Error>> {
    let processors = Arc::new(ProcessorPool::new(2)?);
    let mut manager = MachineManager::start(Arc::clone(&processors))?;
    manager.shutdown()?;
    manager.shutdown()?;

    assert!(
        manager
            .machines()?
            .iter()
            .all(|machine| machine.state == MachineState::Terminated)
    );
    for processor in processors.iter() {
        assert_eq!(processor.state()?, ProcessorState::Unowned);
    }
    Ok(())
}

#[test]
fn startup_failure_rolls_back_workers_and_preserves_existing_binding() -> Result<(), Box<dyn Error>>
{
    let processors = Arc::new(ProcessorPool::new(2)?);
    let mut pool_processors = processors.iter();
    let first = pool_processors
        .next()
        .ok_or_else(|| std::io::Error::other("first Processor missing"))?;
    let second = pool_processors
        .next()
        .ok_or_else(|| std::io::Error::other("second Processor missing"))?;
    let binding = second.bind_current()?;

    assert!(matches!(
        MachineManager::start(Arc::clone(&processors)),
        Err(MachineError::WorkerFailed { .. })
    ));
    assert_eq!(first.state()?, ProcessorState::Unowned);
    assert!(matches!(second.state()?, ProcessorState::Owned { .. }));
    drop(binding);
    assert_eq!(second.state()?, ProcessorState::Unowned);
    Ok(())
}
