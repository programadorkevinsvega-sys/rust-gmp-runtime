use super::common::{
    Allocator, Stats, current_vma_count, distribution, env_usize, env_usize_list,
    supported_capacity,
};
use super::workers::DemandRefillMode;
use super::workers::SNAPSHOTS_PER_SAMPLE;
use super::workers::controlled::run_controlled_sample;
use super::workers::demand_refill::run_demand_refill_sample;
use super::workers::lifetime::run_lifetime_sample;
use super::workers::refill_burst::run_refill_burst_sample;
use super::workers::synchronized_refill::run_synchronized_refill_sample;
use rust_gmp_runtime::stack::StackConfig;
use std::error::Error;
use std::sync::Arc;

struct ConcurrencyConfig {
    retained_count: usize,
    operations_per_worker: usize,
    bursts_per_worker: usize,
    demand_epochs: usize,
    refill_waves: usize,
    samples: usize,
    workers: Vec<usize>,
    cache_sizes_bytes: Vec<usize>,
}

pub fn run(stack_bytes: usize, slots_per_arena: usize) -> Result<(), Box<dyn Error>> {
    let retained_count = env_usize("RUST_GMP_TOURNAMENT_RETAINED", 100_000)?;
    let operations_per_worker = env_usize("RUST_GMP_TOURNAMENT_OPERATIONS", 2_000)?;
    let bursts_per_worker = env_usize("RUST_GMP_TOURNAMENT_BURSTS", 256)?;
    let demand_epochs = env_usize("RUST_GMP_TOURNAMENT_DEMAND_EPOCHS", 256)?;
    let refill_waves = env_usize("RUST_GMP_TOURNAMENT_REFILL_WAVES", 128)?;
    let samples = env_usize("RUST_GMP_TOURNAMENT_SAMPLES", 10)?;
    let workers = env_usize_list("RUST_GMP_TOURNAMENT_WORKERS", &[1, 2, 4, 8])?;
    let cache_sizes_kib = env_usize_list("RUST_GMP_TOURNAMENT_CACHE_KIB", &[32, 64, 128, 256])?;
    if operations_per_worker == 0
        || bursts_per_worker == 0
        || demand_epochs == 0
        || refill_waves == 0
    {
        return Err(
            "tournament operation, burst, demand-epoch, and refill-wave counts must be positive"
                .to_owned()
                .into(),
        );
    }
    if retained_count == 0
        || samples == 0
        || workers.is_empty()
        || workers.contains(&0)
        || cache_sizes_kib.is_empty()
        || cache_sizes_kib.contains(&0)
    {
        return Err("tournament retained count, samples, and workers must be nonzero".into());
    }
    let cache_sizes_bytes = cache_sizes_kib
        .into_iter()
        .map(|size| size.checked_mul(1024).ok_or("cache size overflow"))
        .collect::<Result<Vec<_>, _>>()?;
    let config = ConcurrencyConfig {
        retained_count,
        operations_per_worker,
        bursts_per_worker,
        demand_epochs,
        refill_waves,
        samples,
        workers,
        cache_sizes_bytes,
    };
    for strategy in ["fixed-serial", "fixed-two-phase", "block", "go-inspired"] {
        for instrumented in [false, true] {
            run_strategy(
                strategy,
                stack_bytes,
                slots_per_arena,
                instrumented,
                &config,
            )?;
        }
    }
    run_refill_matrix(stack_bytes, slots_per_arena, &config)?;
    Ok(())
}

#[path = "concurrency/demand_refill.rs"]
mod demand_refill;
#[path = "concurrency/lifetime.rs"]
mod lifetime;
#[path = "concurrency/refill_burst.rs"]
mod refill_burst;
#[path = "concurrency/refill_matrix.rs"]
mod refill_matrix;
#[path = "concurrency/refill_scenario.rs"]
mod refill_scenario;
#[path = "concurrency/scenario.rs"]
mod scenario;
#[path = "concurrency/strategy.rs"]
mod strategy;
#[path = "concurrency/synchronized_refill.rs"]
mod synchronized_refill;
#[path = "concurrency/timing_helpers.rs"]
mod timing_helpers;

use demand_refill::*;
use lifetime::*;
use refill_burst::*;
use refill_matrix::*;
use refill_scenario::*;
use scenario::*;
use strategy::*;
use synchronized_refill::*;
use timing_helpers::*;
