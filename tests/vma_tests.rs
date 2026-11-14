//! Linux VMA diagnostic and stack-admission integration tests.

use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread;

use rust_gmp_runtime::stack::{StackConfig, StackError, StackManager};

#[test]
fn observes_linux_vmas_and_tracks_concurrent_stack_lifecycles() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    let expected_limit: u64 = std::fs::read_to_string("/proc/sys/vm/max_map_count")?
        .trim()
        .parse()?;
    let snapshot = manager.vma_snapshot()?;
    let maps = std::fs::read_to_string("/proc/self/maps")?;
    assert_eq!(snapshot.limit, expected_limit);
    assert_eq!(snapshot.current, maps.lines().count() as u64);

    let baseline = manager.metrics()?;
    let stack = manager.allocate()?;
    let active = manager.metrics()?;
    assert_eq!(active.active_stacks, 1);
    assert_eq!(active.runtime_mapping_count, 1);
    assert_eq!(active.runtime_vma_estimate, 2);
    assert_eq!(active.process_vma_count, Some(active.vma_current));
    drop(stack);
    let released = manager.metrics()?;
    assert_eq!(released.active_stacks, 0);
    assert_eq!(released.runtime_mapping_count, 0);
    assert_eq!(released.runtime_vma_estimate, 0);
    assert_eq!(released.free_stacks, 0);
    assert!(released.vma_limit >= baseline.vma_limit);

    concurrent_lifecycles(manager)?;
    preflight_rejection_is_recoverable_and_does_not_damage_live_stacks()?;
    disabled_preflight_is_explicit()?;
    Ok(())
}

fn concurrent_lifecycles(manager: StackManager) -> Result<(), Box<dyn Error>> {
    const WORKERS: usize = 8;
    const ROUNDS: usize = 4;
    const STACKS_PER_ROUND: usize = 4;
    let barrier = Arc::new(Barrier::new(WORKERS));
    let manager = Arc::new(manager);
    let mut workers = Vec::new();
    for _ in 0..WORKERS {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(), StackError> {
            for _ in 0..ROUNDS {
                let mut stacks = Vec::with_capacity(STACKS_PER_ROUND);
                for _ in 0..STACKS_PER_ROUND {
                    stacks.push(manager.allocate()?);
                }
                barrier.wait();
                drop(stacks);
                barrier.wait();
            }
            Ok(())
        }));
    }
    for worker in workers {
        match worker.join() {
            Ok(result) => result?,
            Err(payload) => {
                let message = match payload.downcast_ref::<String>() {
                    Some(message) => message.clone(),
                    None => match payload.downcast_ref::<&str>() {
                        Some(message) => (*message).to_owned(),
                        None => "non-string panic payload".to_owned(),
                    },
                };
                return Err(format!("stack allocation worker panicked: {message}").into());
            }
        }
    }
    let metrics = manager.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(
        metrics.successful_allocations,
        (WORKERS * ROUNDS * STACKS_PER_ROUND + 1) as u64
    );
    assert_eq!(metrics.released_stacks, metrics.successful_allocations);
    Ok(())
}

fn preflight_rejection_is_recoverable_and_does_not_damage_live_stacks() -> Result<(), Box<dyn Error>>
{
    let manager = StackManager::new(StackConfig::new(4096)?)?;
    let stack = manager.allocate()?;
    let original_bounds = stack.bounds();
    let constrained = Arc::new(StackManager::new(
        StackConfig::new(4096)?.with_vma_safety_margin(u64::MAX),
    )?);
    let barrier = Arc::new(Barrier::new(8));
    let mut workers = Vec::new();
    for _ in 0..8 {
        let constrained = Arc::clone(&constrained);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            matches!(
                constrained.allocate(),
                Err(StackError::VmaPreflightRejected { .. })
            )
        }));
    }
    for worker in workers {
        let rejected = match worker.join() {
            Ok(rejected) => rejected,
            Err(payload) => {
                let message = match payload.downcast_ref::<String>() {
                    Some(message) => message.as_str(),
                    None => match payload.downcast_ref::<&str>() {
                        Some(message) => message,
                        None => "non-string panic payload",
                    },
                };
                return Err(format!("preflight worker panicked: {message}").into());
            }
        };
        assert!(rejected);
    }
    assert_eq!(stack.bounds(), original_bounds);
    let metrics = constrained.metrics()?;
    assert_eq!(metrics.active_stacks, 0);
    assert_eq!(metrics.allocation_failures, 8);
    assert_eq!(metrics.vma_preflight_rejections, 8);
    drop(stack);
    Ok(())
}

fn disabled_preflight_is_explicit() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(4096)?.without_vma_preflight())?;
    assert!(matches!(
        manager.preflight_additional_stacks(1),
        Err(StackError::VmaPreflightDisabled)
    ));
    let stack = manager.allocate()?;
    drop(stack);
    Ok(())
}
