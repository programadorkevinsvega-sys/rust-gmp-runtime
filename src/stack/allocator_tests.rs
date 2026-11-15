use std::error::Error;
use std::io;
#[cfg(feature = "allocator-benchmark")]
use std::sync::{Arc, Barrier};
#[cfg(feature = "allocator-benchmark")]
use std::thread;

use super::FixedStackManager;
use crate::stack::{StackConfig, StackError, protection};

#[test]
fn failed_protection_cleans_partial_mapping_without_damaging_live_stacks()
-> Result<(), Box<dyn Error>> {
    let manager = FixedStackManager::new(StackConfig::new(4096)?)?;
    let live_stack = manager.allocate()?;
    let original_bounds = live_stack.bounds();
    let partial = protection::dispatch::reserve(manager.inner.reservation_size)?;
    let failure = StackError::SystemCall {
        operation: "mprotect",
        source: io::Error::other("injected protection failure"),
    };

    let error = manager.cleanup_failed_allocation(partial, failure);
    assert!(matches!(
        error,
        StackError::SystemCall {
            operation: "mprotect",
            ..
        }
    ));
    assert_eq!(live_stack.bounds(), original_bounds);
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 1);
    assert_eq!(metrics.active_mappings, 1);
    assert_eq!(metrics.failed_allocations, 1);
    assert_eq!(
        metrics.reserved_virtual_bytes,
        manager.inner.reservation_size as u64
    );

    drop(live_stack);
    assert_eq!(manager.metrics()?.active_mappings, 0);
    Ok(())
}

#[cfg(feature = "allocator-benchmark")]
#[test]
fn two_phase_allocator_keeps_accounting_consistent_under_concurrency() -> Result<(), Box<dyn Error>>
{
    const WORKERS: usize = 4;
    const ALLOCATIONS_PER_WORKER: usize = 500;
    let (manager, _) = FixedStackManager::new_benchmarked_two_phase(StackConfig::new(4096)?)?;
    let manager = Arc::new(manager);
    let barrier = Arc::new(Barrier::new(WORKERS));
    let mut workers = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(), StackError> {
            barrier.wait();
            for _ in 0..ALLOCATIONS_PER_WORKER {
                drop(manager.allocate()?);
            }
            Ok(())
        }));
    }
    for worker in workers {
        match worker.join() {
            Ok(result) => result?,
            Err(payload) => {
                let message = match payload.downcast_ref::<String>() {
                    Some(message) => message.as_str(),
                    None => match payload.downcast_ref::<&str>() {
                        Some(message) => message,
                        None => "two-phase allocator worker failed with a non-string payload",
                    },
                };
                return Err(message.to_owned().into());
            }
        }
    }

    let metrics = manager.metrics()?;
    let expected = u64::try_from(WORKERS * ALLOCATIONS_PER_WORKER)?;
    assert_eq!(metrics.successful_allocations, expected);
    assert_eq!(metrics.released_stacks, expected);
    assert_eq!(metrics.active_mappings, 0);
    assert_eq!(metrics.failed_allocations, 0);
    assert_eq!(metrics.cleanup_failures, 0);
    assert_eq!(metrics.vma_preflight_rejections, 0);
    Ok(())
}
