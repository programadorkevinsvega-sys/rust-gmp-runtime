//! Compares Fixed serial, Fixed two-phase, Block/Arena, and Go-inspired stacks.

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
const SLOTS_PER_SPAN: usize = 1_024;

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var_os("RUST_GMP_RUN_ALLOCATOR_TOURNAMENT").is_none() {
        println!("allocator tournament skipped; set RUST_GMP_RUN_ALLOCATOR_TOURNAMENT=1");
        return Ok(());
    }
    println!(
        "allocator_tournament rust=1.85.0 host={} started={:?} stack_bytes={STACK_BYTES} slots_per_span={SLOTS_PER_SPAN}",
        std::env::consts::ARCH,
        SystemTime::now(),
    );
    lifecycle::run(STACK_BYTES, SLOTS_PER_SPAN)?;
    concurrency::run(STACK_BYTES, SLOTS_PER_SPAN)?;
    Ok(())
}
