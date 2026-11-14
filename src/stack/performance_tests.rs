use std::error::Error;
use std::fs;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::StackManager;
use super::admission::VmaAdmission;
use super::config::StackConfig;
use super::protection::dispatch;
use super::vma::{MAPS_PATH, VmaSnapshot, parse_maps_count, read_vma_count, read_vma_snapshot};

const SAMPLES: usize = 10;
const OPERATIONS: usize = 1_000;

#[test]
#[ignore = "manual VMA preflight component benchmark"]
fn vma_preflight_component_costs() -> Result<(), Box<dyn Error>> {
    let maps = fs::read_to_string(MAPS_PATH)?;
    report("procfs_read_maps", SAMPLES, || {
        fs::read(MAPS_PATH)?;
        Ok(())
    })?;
    report("parse_maps_snapshot", SAMPLES, || {
        parse_maps_count(&maps)?;
        Ok(())
    })?;
    report("procfs_read_and_parse_maps", SAMPLES, || {
        read_vma_count()?;
        Ok(())
    })?;
    report("read_full_vma_snapshot", SAMPLES, || {
        read_vma_snapshot()?;
        Ok(())
    })?;

    let mut budget = VmaAdmission::default();
    budget.observe(VmaSnapshot::from_counts(u64::MAX, 1)?, 0)?;
    report("budget_check", SAMPLES * OPERATIONS, || {
        std::hint::black_box(budget.check_cached(1, 1_024)?);
        Ok(())
    })?;
    let mutex = Mutex::new(0_u64);
    report("admission_mutex_uncontended", SAMPLES * OPERATIONS, || {
        let mut count = mutex.lock().map_err(|error| error.to_string())?;
        *count = count.saturating_add(1);
        std::hint::black_box(*count);
        Ok(())
    })?;
    std::hint::black_box(*mutex.lock().map_err(|error| error.to_string())?);
    report_contended_mutex()?;
    report_native_mapping_costs()?;
    report_allocator_modes()?;
    Ok(())
}

pub(super) fn report_native_mapping_costs() -> Result<(), Box<dyn Error>> {
    let page = dispatch::page_size()?;
    let usable = page * 2;
    let total = page + usable;
    for sample in 0..SAMPLES {
        let start = Instant::now();
        let mut maps = Vec::with_capacity(OPERATIONS);
        for _ in 0..OPERATIONS {
            maps.push(dispatch::reserve(total)?);
        }
        report_one("mmap", sample, start.elapsed(), OPERATIONS);
        for map in maps {
            dispatch::release(map, total)?;
        }

        let mut maps = reserve_maps(total)?;
        let start = Instant::now();
        for map in &maps {
            dispatch::enable_stack(*map, page, usable)?;
        }
        report_one("mprotect", sample, start.elapsed(), OPERATIONS);
        release_maps(maps, total)?;

        maps = reserve_maps(total)?;
        for map in &maps {
            dispatch::enable_stack(*map, page, usable)?;
        }
        let start = Instant::now();
        release_maps(maps, total)?;
        report_one("munmap", sample, start.elapsed(), OPERATIONS);
    }
    Ok(())
}

fn reserve_maps(total: usize) -> Result<Vec<std::ptr::NonNull<u8>>, Box<dyn Error>> {
    (0..OPERATIONS)
        .map(|_| dispatch::reserve(total).map_err(Into::into))
        .collect()
}

fn release_maps(maps: Vec<std::ptr::NonNull<u8>>, total: usize) -> Result<(), Box<dyn Error>> {
    for map in maps {
        dispatch::release(map, total)?;
    }
    Ok(())
}

fn report_allocator_modes() -> Result<(), Box<dyn Error>> {
    for (name, config) in [
        ("allocator_preflight", StackConfig::new(8 * 1024)?),
        (
            "allocator_preflight_disabled",
            StackConfig::new(8 * 1024)?.without_vma_preflight(),
        ),
    ] {
        let manager = StackManager::new(config)?;
        report(name, 10_000, || {
            let stack = manager.allocate()?;
            drop(stack);
            Ok(())
        })?;
    }
    Ok(())
}

fn report_contended_mutex() -> Result<(), Box<dyn Error>> {
    const WORKERS: usize = 8;
    const LOCKS_PER_WORKER: usize = 10_000;
    let mutex = Arc::new(Mutex::new(0_u64));
    let barrier = Arc::new(Barrier::new(WORKERS));
    let start = Instant::now();
    let mut workers = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let mutex = Arc::clone(&mutex);
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || -> Result<(), String> {
            barrier.wait();
            for _ in 0..LOCKS_PER_WORKER {
                let mut count = mutex.lock().map_err(|error| error.to_string())?;
                *count += 1;
            }
            Ok(())
        }));
    }
    for worker in workers {
        match worker.join() {
            Ok(result) => result.map_err(|error| -> Box<dyn Error> { error.into() })?,
            Err(payload) => {
                return Err(format!("mutex benchmark worker panicked: {payload:?}").into());
            }
        }
    }
    let elapsed = start.elapsed();
    let expected = (WORKERS * LOCKS_PER_WORKER) as u64;
    let actual = *mutex.lock().map_err(|error| error.to_string())?;
    if actual != expected {
        return Err("mutex benchmark lost updates".into());
    }
    report_one(
        "admission_mutex_contended_8_threads",
        0,
        elapsed,
        expected as usize,
    );
    Ok(())
}

fn report(
    label: &str,
    count: usize,
    mut operation: impl FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    for _ in 0..count {
        operation()?;
    }
    report_one(label, 0, start.elapsed(), count);
    Ok(())
}

fn report_one(label: &str, sample: usize, duration: Duration, operations: usize) {
    println!(
        "vma_cost operation={label} sample={sample} operations={operations} total_ns={} ns_per_operation={}",
        duration.as_nanos(),
        duration.as_nanos() / operations as u128
    );
}
