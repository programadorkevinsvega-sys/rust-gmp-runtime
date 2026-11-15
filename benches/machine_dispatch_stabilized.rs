//! Reproducible F4 dispatch measurements with staged component baselines.

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::stack::{StackConfig, StackManager};

#[path = "machine_dispatch_stabilized/creation.rs"]
mod creation;
#[path = "machine_dispatch_stabilized/measure.rs"]
mod measure;
#[path = "machine_dispatch_stabilized/stages.rs"]
mod stages;
#[path = "machine_dispatch_stabilized/workload.rs"]
mod workload;

use workload::Workload;

const SAMPLE_COUNT: usize = 30;
const TASK_COUNT: usize = 128;
const WARMUP_COUNT: usize = 64;
const STACK_SIZE: usize = 32 * 1024;
const WORKER_COUNTS: [usize; 3] = [1, 2, 4];
const WORKLOAD_COUNT: usize = 4;

static ENTRY_COUNT: AtomicUsize = AtomicUsize::new(0);

struct WorkerSet {
    count: usize,
    manager: MachineManager,
    samples: [Vec<TimingSample>; WORKLOAD_COUNT],
}

struct TimingSample {
    submit: Duration,
    completion_drain: Duration,
    end_to_end: Duration,
}

fn main() -> Result<(), Box<dyn Error>> {
    let stack_manager = StackManager::new(StackConfig::new(STACK_SIZE)?)?;
    let mut worker_sets = Vec::with_capacity(WORKER_COUNTS.len());
    for count in WORKER_COUNTS {
        worker_sets.push(WorkerSet {
            count,
            manager: MachineManager::start(Arc::new(ProcessorPool::new(count)?))?,
            samples: std::array::from_fn(|_| Vec::with_capacity(SAMPLE_COUNT)),
        });
    }
    println!(
        "machine_dispatch_stabilized target={}-{} samples={SAMPLE_COUNT} tasks_per_sample={TASK_COUNT} warmup_tasks={WARMUP_COUNT} stack_bytes={STACK_SIZE} order=rotating workers=1,2,4 workloads=noop,cpu_light_64,cpu_moderate_2048,cpu_heavy_65536",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    measure::warm_up(&worker_sets, &stack_manager)?;
    for sample in 0..SAMPLE_COUNT {
        measure::run_round(sample, &mut worker_sets, &stack_manager)?;
    }
    measure::verify_entries(&worker_sets, &stack_manager)?;
    creation::measure(&stack_manager)?;
    for set in &mut worker_sets {
        set.manager.shutdown()?;
        measure::report_set(set);
    }
    stages::measure_components(&stack_manager)?;
    Ok(())
}
