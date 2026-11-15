use super::common::{
    Allocator, current_vma_count, distribution, env_usize, env_usize_list, supported_capacity,
};
use rust_gmp_runtime::stack::{GoInspiredOptions, GoInspiredStackManager, StackConfig};
use std::error::Error;
use std::fs;
use std::time::Instant;

const WARM_SLOTS: usize = 1_024;
const RECOVERY_CACHE_COUNT: usize = 8;

pub fn run(stack_bytes: usize, slots_per_arena: usize) -> Result<(), Box<dyn Error>> {
    let scale_points = env_usize_list(
        "RUST_GMP_TOURNAMENT_SCALE_POINTS",
        &[1_000, 10_000, 100_000, 500_000],
    )?;
    if scale_points.is_empty() || scale_points.contains(&0) {
        return Err("tournament scale points must be nonzero".into());
    }
    let samples = env_usize("RUST_GMP_BLOCK_STACK_SAMPLES", 10)?;
    let reuse_cycles = env_usize("RUST_GMP_BLOCK_STACK_REUSE_CYCLES", 20_000)?;
    let reuse_samples = env_usize("RUST_GMP_BLOCK_STACK_REUSE_SAMPLES", 10)?;
    for count in scale_points {
        for strategy in ["fixed-serial", "fixed-two-phase", "block", "go-inspired"] {
            run_scale(strategy, count, samples, stack_bytes, slots_per_arena)?;
        }
    }
    for strategy in ["fixed-serial", "fixed-two-phase", "block", "go-inspired"] {
        let madvise_options: &[bool] = if strategy == "block" {
            &[false, true]
        } else {
            &[false]
        };
        for &madvise in madvise_options {
            run_reuse(
                strategy,
                reuse_cycles,
                reuse_samples,
                stack_bytes,
                slots_per_arena,
                madvise,
            )?;
        }
    }
    run_retention_policies(stack_bytes, slots_per_arena)?;
    Ok(())
}

#[path = "lifecycle/retention.rs"]
mod retention;
#[path = "lifecycle/reuse.rs"]
mod reuse;
#[path = "lifecycle/scale.rs"]
mod scale;

use retention::*;
use reuse::*;
use scale::*;
