use super::manager;
use std::collections::BTreeSet;
use std::error::Error;
use std::sync::{Arc, Barrier, mpsc};
use std::thread;

#[test]
fn concurrent_workers_never_receive_the_same_live_slot() -> Result<(), Box<dyn Error>> {
    const WORKERS: usize = 8;
    const SLOTS_PER_WORKER: usize = 32;
    let manager = manager(64)?;
    let ready = Arc::new(Barrier::new(WORKERS + 1));
    let release = Arc::new(Barrier::new(WORKERS + 1));
    let (send, receive) = mpsc::channel();
    let mut workers = Vec::new();
    for _ in 0..WORKERS {
        let manager = manager.clone();
        let ready = Arc::clone(&ready);
        let release = Arc::clone(&release);
        let send = send.clone();
        workers.push(thread::spawn(move || -> Result<(), String> {
            let stacks = (0..SLOTS_PER_WORKER)
                .map(|_| manager.allocate().map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let addresses: Vec<usize> = stacks.iter().map(|stack| stack.bounds().0).collect();
            send.send(addresses)
                .map_err(|error| format!("failed to send slot addresses: {error}"))?;
            ready.wait();
            release.wait();
            drop(stacks);
            Ok(())
        }));
    }
    drop(send);
    ready.wait();
    let mut addresses = BTreeSet::new();
    for _ in 0..WORKERS {
        let worker_addresses = receive.recv()?;
        for address in worker_addresses {
            assert!(addresses.insert(address), "two live handles shared a slot");
        }
    }
    assert_eq!(addresses.len(), WORKERS * SLOTS_PER_WORKER);
    release.wait();
    for worker in workers {
        match worker.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error.into()),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.free_slots, 4 * 64);
    Ok(())
}
