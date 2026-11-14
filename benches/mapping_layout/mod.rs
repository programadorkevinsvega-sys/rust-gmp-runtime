mod ffi;

use std::error::Error;
use std::io;
use std::time::{Duration, Instant};

use rust_gmp_runtime::stack::{StackConfig, StackManager};

use self::ffi::Mapping;

const SAMPLE_COUNT: usize = 10;
const SCALE_POINTS: [usize; 4] = [1_000, 10_000, 100_000, 500_000];
const USABLE_SIZE: usize = 8 * 1024;

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    if std::env::var_os("RUST_GMP_RUN_SCALE").is_none() {
        println!("mapping layout benchmark skipped; set RUST_GMP_RUN_SCALE=1");
        return Ok(());
    }

    let manager = StackManager::new(StackConfig::new(USABLE_SIZE)?)?;
    let page_size = manager.page_size();
    let usable_size = manager.usable_size();
    let slot_size = page_size
        .checked_add(usable_size)
        .ok_or("stack slot size overflow")?;
    println!("page_size={page_size} usable_size={usable_size} slot_size={slot_size}");
    warm_up(slot_size, page_size, usable_size)?;

    for stack_count in SCALE_POINTS {
        measure_individual(stack_count, slot_size, page_size, usable_size)?;
        measure_grouped(stack_count, slot_size, page_size, usable_size)?;
    }
    Ok(())
}

fn warm_up(slot: usize, page: usize, usable: usize) -> Result<(), Box<dyn Error>> {
    let individual = reserve_individual(64, slot, page, usable)?;
    drop(individual);
    let grouped = reserve_grouped(64, slot, page, usable)?;
    drop(grouped);
    Ok(())
}

fn measure_individual(
    count: usize,
    slot: usize,
    page: usize,
    usable: usize,
) -> Result<(), Box<dyn Error>> {
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let mappings = reserve_individual(count, slot, page, usable)?;
        let elapsed = started.elapsed();
        let vmas = read_vma_count()?;
        samples.push(elapsed);
        println!(
            "layout=individual stacks={count} sample={sample} elapsed_ns={} vmas={vmas}",
            elapsed.as_nanos()
        );
        drop(mappings);
    }
    report("individual", count, &mut samples);
    Ok(())
}

fn reserve_individual(
    count: usize,
    slot: usize,
    page: usize,
    usable: usize,
) -> Result<Vec<Mapping>, Box<dyn Error>> {
    let mut mappings = Vec::with_capacity(count);
    for _ in 0..count {
        let mapping = Mapping::reserve(slot)?;
        mapping.enable(page, usable)?;
        mappings.push(mapping);
    }
    Ok(mappings)
}

fn measure_grouped(
    count: usize,
    slot: usize,
    page: usize,
    usable: usize,
) -> Result<(), Box<dyn Error>> {
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let mapping = reserve_grouped(count, slot, page, usable)?;
        let elapsed = started.elapsed();
        let vmas = read_vma_count()?;
        samples.push(elapsed);
        println!(
            "layout=grouped stacks={count} sample={sample} elapsed_ns={} vmas={vmas}",
            elapsed.as_nanos()
        );
        drop(mapping);
    }
    report("grouped", count, &mut samples);
    Ok(())
}

fn reserve_grouped(
    count: usize,
    slot: usize,
    page: usize,
    usable: usize,
) -> Result<Mapping, Box<dyn Error>> {
    let length = count.checked_mul(slot).ok_or("slab size overflow")?;
    let mapping = Mapping::reserve(length)?;
    for index in 0..count {
        let offset = index
            .checked_mul(slot)
            .and_then(|start| start.checked_add(page))
            .ok_or("slab slot offset overflow")?;
        mapping.enable(offset, usable)?;
    }
    Ok(mapping)
}

fn read_vma_count() -> Result<usize, io::Error> {
    Ok(std::fs::read_to_string("/proc/self/maps")?.lines().count())
}

fn report(layout: &str, count: usize, samples: &mut [Duration]) {
    samples.sort_unstable();
    let middle = samples.len() / 2;
    let lower = samples[middle - 1].as_nanos();
    let upper = samples[middle].as_nanos();
    let median = lower + (upper - lower) / 2;
    let spread = samples[samples.len() - 1].as_nanos() - samples[0].as_nanos();
    println!(
        "summary layout={layout} stacks={count} samples={SAMPLE_COUNT} median_ns={median} max_ns={} spread_ns={spread}",
        samples[samples.len() - 1].as_nanos()
    );
}
