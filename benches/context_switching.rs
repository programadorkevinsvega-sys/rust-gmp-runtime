#![allow(unsafe_code)]

//! Measures one-shot context dispatch and return against a direct Rust call.

use std::error::Error;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

const SAMPLE_COUNT: usize = 10;
const DEFAULT_ITERATIONS: usize = 256;
const MAX_ITERATIONS: usize = 4096;
const STACK_SIZE: usize = 32 * 1024;
const ITERATIONS_ENV: &str = "RUST_GMP_CONTEXT_BENCH_ITERATIONS";

static ENTRY_COUNT: AtomicUsize = AtomicUsize::new(0);

fn main() -> Result<(), Box<dyn Error>> {
    let iterations = configured_iterations()?;
    let manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    warm_up(&manager)?;

    let mut direct_ns_per_call = Vec::with_capacity(SAMPLE_COUNT);
    let mut cold_ns_per_call = Vec::with_capacity(SAMPLE_COUNT);
    let mut prefaulted_ns_per_call = Vec::with_capacity(SAMPLE_COUNT);
    println!(
        "context_switching target={}-{} samples={SAMPLE_COUNT} iterations_per_sample={iterations} stack_bytes={STACK_SIZE} timed_work=direct_entry_and_one_shot_run_once cold_vs_prefaulted",
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    for sample in 0..SAMPLE_COUNT {
        let direct_started = Instant::now();
        for _ in 0..iterations {
            benchmark_entry();
        }
        let direct_elapsed = direct_started.elapsed();

        let cold_routines = prepare_routines(&manager, iterations)?;
        let cold_started = Instant::now();
        for routine in &cold_routines {
            routine.run_once(benchmark_entry)?;
        }
        let cold_elapsed = cold_started.elapsed();
        drop(cold_routines);

        let prefaulted_routines = prepare_routines(&manager, iterations)?;
        prefault_routines(&prefaulted_routines, manager.page_size())?;
        let prefaulted_started = Instant::now();
        for routine in &prefaulted_routines {
            routine.run_once(benchmark_entry)?;
        }
        let prefaulted_elapsed = prefaulted_started.elapsed();

        let direct_per_call = direct_elapsed.as_nanos() / iterations as u128;
        let cold_per_call = cold_elapsed.as_nanos() / iterations as u128;
        let prefaulted_per_call = prefaulted_elapsed.as_nanos() / iterations as u128;
        direct_ns_per_call.push(direct_per_call);
        cold_ns_per_call.push(cold_per_call);
        prefaulted_ns_per_call.push(prefaulted_per_call);
        println!(
            "context_switching sample={sample} direct_batch_ns={} direct_ns_per_call={direct_per_call} cold_run_once_batch_ns={} cold_run_once_ns_per_call={cold_per_call} cold_estimated_ns_per_transfer={} prefaulted_run_once_batch_ns={} prefaulted_run_once_ns_per_call={prefaulted_per_call} prefaulted_estimated_ns_per_transfer={}",
            direct_elapsed.as_nanos(),
            cold_elapsed.as_nanos(),
            cold_per_call / 2,
            prefaulted_elapsed.as_nanos(),
            prefaulted_per_call / 2
        );
        drop(prefaulted_routines);
    }

    report_summary("direct", &mut direct_ns_per_call);
    report_summary("run_once_cold", &mut cold_ns_per_call);
    report_summary("run_once_prefaulted", &mut prefaulted_ns_per_call);
    println!(
        "context_switching entries_observed={} interpretation=run_once_includes_trampoline_catch_unwind_lifecycle; cold_includes_first_stack_touch; prefaulted_touches_each_usable_page_before_timing",
        ENTRY_COUNT.load(Ordering::Relaxed)
    );
    Ok(())
}

fn configured_iterations() -> Result<usize, Box<dyn Error>> {
    let iterations = match std::env::var(ITERATIONS_ENV) {
        Ok(value) => value.parse::<usize>()?,
        Err(std::env::VarError::NotPresent) => DEFAULT_ITERATIONS,
        Err(error) => return Err(Box::new(error)),
    };
    if !(1..=MAX_ITERATIONS).contains(&iterations) {
        return Err(format!("{ITERATIONS_ENV} must be between 1 and {MAX_ITERATIONS}").into());
    }
    Ok(iterations)
}

fn warm_up(manager: &StackManager) -> Result<(), Box<dyn Error>> {
    for _ in 0..16 {
        let routine = Routine::new(manager, RoutineMetadata::new())?;
        routine.publish()?;
        routine.run_once(benchmark_entry)?;
    }
    Ok(())
}

fn prepare_routines(manager: &StackManager, count: usize) -> Result<Vec<Routine>, Box<dyn Error>> {
    let mut routines = Vec::with_capacity(count);
    for _ in 0..count {
        let routine = Routine::new(manager, RoutineMetadata::new())?;
        routine.publish()?;
        routines.push(routine);
    }
    Ok(routines)
}

fn prefault_routines(routines: &[Routine], page_size: usize) -> Result<(), Box<dyn Error>> {
    for routine in routines {
        let bounds = routine.stack_bounds()?;
        let mut address = bounds.usable_start;
        while address < bounds.usable_end {
            // SAFETY: Each address is in this non-executing routine's writable
            // stack mapping, and the routine is not executing concurrently.
            unsafe {
                (address as *mut u8).write_volatile(0);
            }
            address = address
                .checked_add(page_size)
                .ok_or("stack prefault address overflow")?;
        }
    }
    Ok(())
}

#[inline(never)]
fn benchmark_entry() {
    ENTRY_COUNT.fetch_add(black_box(1), Ordering::Relaxed);
}

fn report_summary(label: &str, samples: &mut [u128]) {
    samples.sort_unstable();
    let median = percentile(samples, 50);
    let p95 = percentile(samples, 95);
    let p99 = percentile(samples, 99);
    println!(
        "context_switching summary={label} unit=ns_per_call samples={} p50={median} p95={p95} p99_max_observed={p99} min={} max={} spread={}",
        samples.len(),
        samples[0],
        samples[samples.len() - 1],
        samples[samples.len() - 1] - samples[0]
    );
}

fn percentile(sorted_samples: &[u128], percentile: usize) -> u128 {
    let rank = (sorted_samples.len() * percentile).div_ceil(100);
    sorted_samples[rank.saturating_sub(1)]
}
