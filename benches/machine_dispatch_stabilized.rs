//! Reproducible F4 dispatch measurements with staged component baselines.

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rust_gmp_runtime::machine::MachineManager;
use rust_gmp_runtime::processor::ProcessorPool;
use rust_gmp_runtime::stack::{StackConfig, StackManager};

#[path = "machine_dispatch_stabilized/measure.rs"]
mod measure;
#[path = "machine_dispatch_stabilized/stages.rs"]
mod stages;

const SAMPLE_COUNT: usize = 30;
const TASK_COUNT: usize = 128;
const WARMUP_COUNT: usize = 64;
const STACK_SIZE: usize = 32 * 1024;
const WORKER_COUNTS: [usize; 3] = [1, 2, 4];

static ENTRY_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug)]
enum Workload {
    Noop,
    CpuLight,
    CpuModerate,
}

impl Workload {
    const ALL: [Self; 3] = [Self::Noop, Self::CpuLight, Self::CpuModerate];

    fn index(self) -> usize {
        match self {
            Self::Noop => 0,
            Self::CpuLight => 1,
            Self::CpuModerate => 2,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::CpuLight => "cpu_light",
            Self::CpuModerate => "cpu_moderate",
        }
    }

    fn execute(self) {
        let iterations = match self {
            Self::Noop => 0,
            Self::CpuLight => 64,
            Self::CpuModerate => 2_048,
        };
        let mut value = std::hint::black_box(0x9e37_79b9_u64);
        for _ in 0..iterations {
            value = value
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
        }
        std::hint::black_box(value);
        ENTRY_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

struct WorkerSet {
    count: usize,
    manager: MachineManager,
    samples: [Vec<TimingSample>; 3],
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
        "machine_dispatch_stabilized target={}-{} samples={SAMPLE_COUNT} tasks_per_sample={TASK_COUNT} warmup_tasks={WARMUP_COUNT} stack_bytes={STACK_SIZE} order=rotating workers=1,2,4 workloads=noop,cpu_light_64,cpu_moderate_2048",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    measure::warm_up(&worker_sets, &stack_manager)?;
    for sample in 0..SAMPLE_COUNT {
        measure::run_round(sample, &mut worker_sets, &stack_manager)?;
    }
    for set in &mut worker_sets {
        set.manager.shutdown()?;
        measure::report_set(set);
    }
    stages::measure_components(&stack_manager)?;
    measure::verify_entries()?;
    Ok(())
}
