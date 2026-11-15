use std::error::Error;
use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use super::allocator::FixedStackManager;
use super::config::StackConfig;
use super::mapping::FixedStack;
use super::vma::{MAPS_PATH, parse_maps_count, read_vma_count, read_vma_snapshot};

const BASE_STACKS: usize = 400_000;
const SAMPLE_STACKS: usize = 100_000;
const SAMPLES: usize = 10;

#[test]
#[ignore = "manual high-VMA parse and allocator comparison"]
fn high_vma_preflight_costs() -> Result<(), Box<dyn Error>> {
    let enabled = FixedStackManager::new(StackConfig::new(8 * 1024)?)?;
    let disabled = FixedStackManager::new(StackConfig::new(8 * 1024)?.without_vma_preflight())?;
    let base = allocate(&enabled, BASE_STACKS)?;
    let maps = fs::read_to_string(MAPS_PATH)?;
    println!(
        "vma_cost high_vma_observed={} lines_in_snapshot={}",
        read_vma_count()?,
        parse_maps_count(&maps)?
    );
    report("high_vma_read_maps", || {
        fs::read(MAPS_PATH)?;
        Ok(())
    })?;
    report("high_vma_parse_snapshot", || {
        parse_maps_count(&maps)?;
        Ok(())
    })?;
    report("high_vma_read_and_parse", || {
        read_vma_count()?;
        Ok(())
    })?;
    report("high_vma_full_snapshot", || {
        read_vma_snapshot()?;
        Ok(())
    })?;
    report_allocation_during_snapshot(&enabled)?;
    super::performance_tests::report_native_mapping_costs()?;

    for sample in 0..SAMPLES {
        let order = if sample % 2 == 0 {
            [("preflight", &enabled), ("disabled", &disabled)]
        } else {
            [("disabled", &disabled), ("preflight", &enabled)]
        };
        for (name, manager) in order {
            let start = Instant::now();
            let batch = allocate(manager, SAMPLE_STACKS)?;
            let elapsed = start.elapsed();
            println!(
                "vma_cost high_vma_allocator mode={name} sample={sample} stacks={SAMPLE_STACKS} total_ns={} ns_per_stack={}",
                elapsed.as_nanos(),
                elapsed.as_nanos() / SAMPLE_STACKS as u128
            );
            drop(batch);
            manager.vma_snapshot()?;
        }
    }
    drop(base);
    Ok(())
}

fn allocate(manager: &FixedStackManager, count: usize) -> Result<Vec<FixedStack>, Box<dyn Error>> {
    let mut stacks = Vec::with_capacity(count);
    for _ in 0..count {
        stacks.push(manager.allocate()?);
    }
    Ok(stacks)
}

fn report(
    label: &str,
    mut operation: impl FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    for _ in 0..SAMPLES {
        operation()?;
    }
    let elapsed = start.elapsed();
    println!(
        "vma_cost operation={label} samples={SAMPLES} total_ns={} ns_per_sample={}",
        elapsed.as_nanos(),
        elapsed.as_nanos() / SAMPLES as u128
    );
    Ok(())
}

fn report_allocation_during_snapshot(manager: &FixedStackManager) -> Result<(), Box<dyn Error>> {
    for sample in 0..SAMPLES {
        manager.lock_admission()?.expire_snapshot();
        let barrier = Arc::new(Barrier::new(2));
        let contender = manager.clone();
        let contender_barrier = Arc::clone(&barrier);
        let worker = thread::spawn(move || {
            let started = Instant::now();
            let result = contender.admission_with_snapshot_using(false, || {
                contender_barrier.wait();
                read_vma_snapshot()
            });
            let elapsed = started.elapsed().as_nanos();
            result.map(|(admission, snapshot)| {
                drop(admission);
                (elapsed, snapshot)
            })
        });
        barrier.wait();
        let started = Instant::now();
        let stack = manager.allocate()?;
        let allocation_ns = started.elapsed().as_nanos();
        drop(stack);
        let (snapshot_ns, _) = match worker.join() {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => return Err(error.into()),
            Err(payload) => return Err(format!("snapshot worker panicked: {payload:?}").into()),
        };
        println!(
            "vma_cost operation=allocation_during_snapshot sample={sample} snapshot_ns={snapshot_ns} allocation_ns={allocation_ns}"
        );
    }
    Ok(())
}
