//! Compares global Go-inspired pool access with explicit worker-owned caches.

use std::error::Error;
use std::io::{self, Write};
use std::process::{Command, Output};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

use rust_gmp_runtime::stack::{GoInspiredOptions, StackConfig};

#[path = "pool_contention/child.rs"]
mod child;
#[path = "pool_contention/metrics.rs"]
mod metrics;
#[path = "pool_contention/worker.rs"]
mod worker;

const CHILD_FLAG: &str = "--isolated";
const WORKER_COUNTS: [usize; 6] = [1, 2, 4, 8, 12, 16];
const SAMPLE_COUNT: usize = 10;
const OPERATIONS_PER_WORKER: usize = 1_024;
const STACK_SIZE: usize = 64 * 1024;
const SLOTS_PER_SPAN: usize = 64;
const CACHE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PoolPath {
    Global,
    WorkerCache,
}

impl PoolPath {
    fn parse(value: &str) -> Result<Self, io::Error> {
        match value {
            "global" => Ok(Self::Global),
            "worker-cache" => Ok(Self::WorkerCache),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown pool path",
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::WorkerCache => "worker-cache",
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == CHILD_FLAG) {
        return child::run_child(&args);
    }
    if !args.is_empty() && args.as_slice() != ["--bench"] {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid arguments").into());
    }

    println!(
        "pool_contention process_isolated=true instrumented={} workers={WORKER_COUNTS:?} samples={SAMPLE_COUNT} operations_per_worker={OPERATIONS_PER_WORKER} stack_bytes={STACK_SIZE} cache_bytes={CACHE_BYTES} slots_per_span={SLOTS_PER_SPAN}",
        cfg!(feature = "allocator-benchmark")
    );
    let executable = std::env::current_exe()?;
    for workers in WORKER_COUNTS {
        for sample in 0..SAMPLE_COUNT {
            let paths = if sample % 2 == 0 {
                [PoolPath::Global, PoolPath::WorkerCache]
            } else {
                [PoolPath::WorkerCache, PoolPath::Global]
            };
            for path in paths {
                let output = Command::new(&executable)
                    .arg(CHILD_FLAG)
                    .arg(path.name())
                    .arg(workers.to_string())
                    .arg(sample.to_string())
                    .output()?;
                forward_output(output, path, workers, sample)?;
            }
        }
    }
    Ok(())
}

fn forward_output(
    output: Output,
    path: PoolPath,
    workers: usize,
    sample: usize,
) -> Result<(), Box<dyn Error>> {
    if !output.status.success() {
        io::stderr().write_all(&output.stderr)?;
        return Err(io::Error::other(format!(
            "isolated {path:?}/{workers} sample {sample} exited with {}",
            output.status
        ))
        .into());
    }
    io::stdout().write_all(&output.stdout)?;
    Ok(())
}
