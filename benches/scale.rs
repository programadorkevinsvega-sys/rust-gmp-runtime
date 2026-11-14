//! Measures stack allocation and release across increasing simultaneous counts.

use std::error::Error;
use std::time::{Duration, Instant};

use rust_gmp_runtime::stack::{Stack, StackConfig, StackError, StackManager, StackMetrics};

const SAMPLE_COUNT: usize = 10;
const SCALE_POINTS: [usize; 5] = [1_000, 10_000, 100_000, 500_000, 1_000_000];
const RELEASE_BEFORE_RECONCILIATION: usize = 100_000;
const RECONCILIATION_PROBE_STACKS: usize = 25_000;
const STACK_SIZE: usize = 8 * 1024;
const RUN_FLAG: &str = "RUST_GMP_RUN_SCALE";

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var_os(RUN_FLAG).is_none() {
        println!("scale benchmark skipped; set {RUN_FLAG}=1 to run it explicitly");
        return Ok(());
    }

    let manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    let warmup = allocate_batch(&manager, 64)?;
    drop(warmup);
    println!("scale warmup stacks=64");
    for stack_count in SCALE_POINTS {
        measure_count(&manager, stack_count)?;
    }
    Ok(())
}

fn measure_count(manager: &StackManager, stack_count: usize) -> Result<(), Box<dyn Error>> {
    let mut preflight_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut allocation_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut release_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut recovery_samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let before = manager.metrics()?;
        let preflight_started = Instant::now();
        if let Err(error) = manager.preflight_additional_stacks(stack_count) {
            let preflight_time = preflight_started.elapsed();
            let rejected = manager.metrics()?;
            report_preflight_skip(stack_count, &rejected, preflight_time, error);
            return Ok(());
        }
        let preflight_time = preflight_started.elapsed();
        preflight_samples.push(preflight_time);
        let started = Instant::now();
        let mut stacks = match allocate_batch(manager, stack_count) {
            Ok(stacks) => stacks,
            Err(error) => {
                let metrics = manager.metrics()?;
                println!(
                    "scale allocation_failed stacks={stack_count} sample={sample} error={error} failures={} vma_rejections={} vmas_before={} vmas_after={}",
                    metrics.allocation_failures,
                    metrics.vma_preflight_rejections,
                    before.vma_current,
                    metrics.vma_current
                );
                return Ok(());
            }
        };
        let allocation_time = started.elapsed();
        let active = manager.metrics()?;
        let release_count = stack_count.min(RELEASE_BEFORE_RECONCILIATION);
        let released = stacks.split_off(stack_count - release_count);
        let release_started = Instant::now();
        drop(released);
        let first_release_time = release_started.elapsed();

        let recovery_started = Instant::now();
        let recovered = manager.preflight_additional_stacks(RECONCILIATION_PROBE_STACKS)?;
        let recovery_time = recovery_started.elapsed();
        let reconciled = recovered.current < active.vma_current;
        if stack_count == 500_000 && !reconciled {
            return Err("500k release did not trigger a fresh VMA reconciliation".into());
        }

        let remaining_release_started = Instant::now();
        drop(stacks);
        let release_time = first_release_time + remaining_release_started.elapsed();
        let after = manager.metrics()?;
        recovery_samples.push(recovery_time);
        allocation_samples.push(allocation_time);
        release_samples.push(release_time);
        println!(
            "scale stacks={stack_count} sample={sample} preflight_ns={} allocate_ns={} release_ns={} post_release_preflight_ns={} reconciled_after_release={reconciled} vmas_before={} vmas_active={} vmas_after={} mappings_active={} runtime_vma_estimate={} rss_before={:?} rss_active={:?} rss_after={:?} reserved_active={} accessible_active={} reserved_after={} rejections={} failures={}",
            preflight_time.as_nanos(),
            allocation_time.as_nanos(),
            release_time.as_nanos(),
            recovery_time.as_nanos(),
            before.vma_current,
            active.vma_current,
            after.vma_current,
            active.runtime_mapping_count,
            active.runtime_vma_estimate,
            before.process_resident_bytes,
            active.process_resident_bytes,
            after.process_resident_bytes,
            active.reserved_virtual_bytes,
            active.accessible_virtual_bytes,
            after.reserved_virtual_bytes,
            after.vma_preflight_rejections,
            after.allocation_failures
        );
    }
    report_samples("preflight", stack_count, &mut preflight_samples);
    report_samples("allocate", stack_count, &mut allocation_samples);
    report_samples("release", stack_count, &mut release_samples);
    report_samples("post_release_preflight", stack_count, &mut recovery_samples);
    Ok(())
}

fn report_preflight_skip(
    stack_count: usize,
    metrics: &StackMetrics,
    preflight_time: Duration,
    error: StackError,
) {
    println!(
        "scale skipped stacks={stack_count} reason={error} preflight_ns={} current_vmas={} limit={} remaining={} runtime_vma_estimate={} rejections={} allocation_failures={}",
        preflight_time.as_nanos(),
        metrics.vma_current,
        metrics.vma_limit,
        metrics.vma_remaining,
        metrics.runtime_vma_estimate,
        metrics.vma_preflight_rejections,
        metrics.allocation_failures
    );
}

fn allocate_batch(
    manager: &StackManager,
    stack_count: usize,
) -> Result<Vec<Stack>, Box<dyn Error>> {
    let mut stacks = Vec::with_capacity(stack_count);
    for _ in 0..stack_count {
        stacks.push(manager.allocate()?);
    }
    Ok(stacks)
}

fn report_samples(label: &str, stack_count: usize, samples: &mut [Duration]) {
    samples.sort_unstable();
    let median = median_nanoseconds(samples);
    let maximum = samples[samples.len() - 1].as_nanos();
    let minimum = samples[0].as_nanos();
    println!(
        "scale summary operation={label} stacks={stack_count} samples={} median_ns={median} max_observed_ns={maximum} spread_ns={}",
        samples.len(),
        maximum - minimum
    );
}

fn median_nanoseconds(samples: &[Duration]) -> u128 {
    let middle = samples.len() / 2;
    if samples.len() % 2 == 0 {
        let lower = samples[middle - 1].as_nanos();
        let upper = samples[middle].as_nanos();
        lower / 2 + upper / 2 + (lower % 2 + upper % 2) / 2
    } else {
        samples[middle].as_nanos()
    }
}
