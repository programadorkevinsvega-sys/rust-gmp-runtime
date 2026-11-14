use crate::stack::{GoInspiredOptions, GoInspiredStackManager, StackConfig};
use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn snapshots_overlap_concurrent_span_use_and_release() -> Result<(), Box<dyn Error>> {
    let options = GoInspiredOptions::default()
        .with_slots_per_span(8)
        .with_retained_spans(0);
    let manager = Arc::new(GoInspiredStackManager::with_options(
        StackConfig::new(8 * 1024)?,
        options,
    )?);
    let start = Arc::new(Barrier::new(5));
    let allocated = Arc::new(Barrier::new(5));
    let snapshot_done = Arc::new(Barrier::new(5));
    let mut workers = Vec::new();
    for _ in 0..4 {
        let manager = Arc::clone(&manager);
        let start = Arc::clone(&start);
        let allocated = Arc::clone(&allocated);
        let snapshot_done = Arc::clone(&snapshot_done);
        workers.push(thread::spawn(move || {
            for _ in 0..8 {
                start.wait();
                let released = manager.allocate().map_err(std::io::Error::other)?;
                let held = manager.allocate().map_err(std::io::Error::other)?;
                drop(released);
                allocated.wait();
                snapshot_done.wait();
                drop(held);
            }
            Ok::<(), std::io::Error>(())
        }));
    }

    let mut snapshots = Vec::new();
    for _ in 0..8 {
        start.wait();
        allocated.wait();
        snapshots.push(manager.vma_snapshot()?);
        snapshot_done.wait();
    }
    for worker in workers {
        match worker.join() {
            Ok(result) => result?,
            Err(payload) => {
                return Err(std::io::Error::other(format!(
                    "allocator worker panicked: {payload:?}"
                ))
                .into());
            }
        }
    }
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.allocations, metrics.releases);
    assert!(snapshots.iter().all(|snapshot| snapshot.current > 0));
    assert!(manager.trim()? > 0);
    assert_eq!(manager.metrics()?.reserved_virtual_bytes, 0);
    Ok(())
}
