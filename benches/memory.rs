//! Compares live stack reservations with an equal number of native threads.

use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, Instant};

use rust_gmp_runtime::stack::{StackConfig, StackManager};

const SAMPLE_COUNT: usize = 10;
const TASK_COUNT: usize = 64;
const STACK_SIZE: usize = 64 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    let mut stack_samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let mut stacks = Vec::with_capacity(TASK_COUNT);
        for _ in 0..TASK_COUNT {
            stacks.push(manager.allocate()?);
        }
        let elapsed = started.elapsed();
        let metrics = manager.metrics()?;
        stack_samples.push(elapsed);
        println!(
            "stack sample={sample} batch_ns={} ns_per_stack={} reserved={} accessible={} vsize={:?} rss={:?} vmas={:?}",
            elapsed.as_nanos(),
            elapsed.as_nanos() / TASK_COUNT as u128,
            metrics.reserved_virtual_bytes,
            metrics.accessible_virtual_bytes,
            metrics.process_virtual_bytes,
            metrics.process_resident_bytes,
            metrics.process_vma_count
        );
        drop(stacks);
    }

    let mut thread_samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let ready = Arc::new(Barrier::new(TASK_COUNT + 1));
        let release = Arc::new(Barrier::new(TASK_COUNT + 1));
        let started = Instant::now();
        let mut workers = Vec::with_capacity(TASK_COUNT);
        for _ in 0..TASK_COUNT {
            let ready = Arc::clone(&ready);
            let release = Arc::clone(&release);
            workers.push(Builder::new().spawn(move || {
                ready.wait();
                release.wait();
            })?);
        }
        ready.wait();
        let elapsed = started.elapsed();
        let metrics = manager.metrics()?;
        thread_samples.push(elapsed);
        println!(
            "native-thread sample={sample} batch_ns={} ns_per_thread={} vsize={:?} rss={:?} vmas={:?}",
            elapsed.as_nanos(),
            elapsed.as_nanos() / TASK_COUNT as u128,
            metrics.process_virtual_bytes,
            metrics.process_resident_bytes,
            metrics.process_vma_count
        );
        release.wait();
        join_workers(workers);
    }

    report_samples("stack", &mut stack_samples);
    report_samples("native-thread", &mut thread_samples);
    Ok(())
}

fn join_workers(workers: Vec<JoinHandle<()>>) {
    for worker in workers {
        match worker.join() {
            Ok(()) => {}
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

fn report_samples(label: &str, samples: &mut [Duration]) {
    samples.sort_unstable();
    let median = samples[samples.len() / 2].as_nanos();
    let maximum = samples[samples.len() - 1].as_nanos();
    let minimum = samples[0].as_nanos();
    println!(
        "{label} summary samples={SAMPLE_COUNT} median_batch_ns={median} p99_max_observed_ns={maximum} spread_ns={}",
        maximum - minimum
    );
}
