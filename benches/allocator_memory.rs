//! Measures process RSS as touched Go-inspired stacks move through retention states.

use std::error::Error;
use std::process::Command;

const RETAINED_SPANS: [usize; 4] = [0, 1, 4, 16];

#[path = "allocator_memory/worker.rs"]
mod worker;

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == "--worker") {
        return worker::run(&args[1..]);
    }

    let stack_count = env_usize("RUST_GMP_MEMORY_STACKS", 8_192)?;
    let samples = env_usize("RUST_GMP_MEMORY_SAMPLES", 3)?;
    if stack_count == 0 || samples == 0 {
        return Err("stack count and sample count must be nonzero".into());
    }
    println!(
        "allocator_memory rust={} target={}-{} stacks={stack_count} samples={samples} stack_bytes={} slots_per_span={} cache_count={} retention_spans={RETAINED_SPANS:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        worker::STACK_BYTES,
        worker::SLOTS_PER_SPAN,
        worker::CACHE_COUNT
    );
    let executable = std::env::current_exe()?;
    for scenario in ["global-pool", "processor-cache"] {
        for retained_spans in RETAINED_SPANS {
            for sample in 0..samples {
                let output = Command::new(&executable)
                    .args([
                        "--worker",
                        scenario,
                        &retained_spans.to_string(),
                        &stack_count.to_string(),
                        &sample.to_string(),
                    ])
                    .output()?;
                if !output.status.success() {
                    return Err(format!(
                        "memory sample failed: scenario={scenario} retained_spans={retained_spans} sample={sample}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )
                    .into());
                }
                print!("{}", String::from_utf8(output.stdout)?);
            }
        }
    }
    Ok(())
}

fn env_usize(name: &str, default: usize) -> Result<usize, Box<dyn Error>> {
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}
