//! Compares routine lifecycle setup with native thread spawn and completion.

use std::error::Error;
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, Instant};

use rust_gmp_runtime::routine::{Routine, RoutineMetadata};
use rust_gmp_runtime::stack::{StackConfig, StackManager};

const SAMPLE_COUNT: usize = 10;
const TASK_COUNT: usize = 64;
const STACK_SIZE: usize = 64 * 1024;

fn main() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    let mut routine_samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let started = Instant::now();
        for _ in 0..TASK_COUNT {
            let routine = Routine::new(&manager, RoutineMetadata::new())?;
            routine.publish()?;
            routine.try_start()?.complete()?;
        }
        let elapsed = started.elapsed();
        routine_samples.push(elapsed);
        let metrics = manager.metrics()?;
        println!(
            "routine sample={sample} batch_ns={} ns_per_routine={} vsize={:?} rss={:?} vmas={:?}",
            elapsed.as_nanos(),
            elapsed.as_nanos() / TASK_COUNT as u128,
            metrics.process_virtual_bytes,
            metrics.process_resident_bytes,
            metrics.process_vma_count
        );
    }

    let mut thread_samples = Vec::with_capacity(SAMPLE_COUNT);
    for sample in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let mut workers = Vec::with_capacity(TASK_COUNT);
        for _ in 0..TASK_COUNT {
            workers.push(Builder::new().spawn(|| {})?);
        }
        join_workers(workers);
        let elapsed = started.elapsed();
        thread_samples.push(elapsed);
        let metrics = manager.metrics()?;
        println!(
            "native-thread sample={sample} batch_ns={} ns_per_thread={} vsize={:?} rss={:?} vmas={:?}",
            elapsed.as_nanos(),
            elapsed.as_nanos() / TASK_COUNT as u128,
            metrics.process_virtual_bytes,
            metrics.process_resident_bytes,
            metrics.process_vma_count
        );
    }

    report_samples("routine", &mut routine_samples);
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
