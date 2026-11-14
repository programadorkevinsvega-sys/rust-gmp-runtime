//! Compares production Fixed Stack with the benchmark-only block arena manager.

#[path = "block_stack_compare/common.rs"]
mod common;
#[path = "block_stack_compare/concurrency.rs"]
mod concurrency;
#[path = "block_stack_compare/lifecycle.rs"]
mod lifecycle;
#[path = "block_stack_compare/workers.rs"]
mod workers;

use std::error::Error;
use std::time::SystemTime;

const STACK_BYTES: usize = 8 * 1024;
const SLOTS_PER_ARENA: usize = 1_024;

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var_os("RUST_GMP_RUN_BLOCK_STACK").is_none() {
        println!("block stack comparison skipped; set RUST_GMP_RUN_BLOCK_STACK=1");
        return Ok(());
    }
    println!(
        "block_stack_benchmark rust=1.85.0 host={} started={:?} stack_bytes={STACK_BYTES} slots_per_arena={SLOTS_PER_ARENA}",
        std::env::consts::ARCH,
        SystemTime::now(),
    );
    lifecycle::run(STACK_BYTES, SLOTS_PER_ARENA)?;
    concurrency::run(STACK_BYTES, SLOTS_PER_ARENA)?;
    Ok(())
}
