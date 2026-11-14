use super::*;

pub(super) fn run_refill_matrix(
    stack_bytes: usize,
    slots_per_span: usize,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    for &cache_bytes in &config.cache_sizes_bytes {
        for instrumented in [false, true] {
            let allocator = Arc::new(Allocator::create_with_go_cache_bytes(
                "go-inspired",
                StackConfig::new(stack_bytes)?,
                slots_per_span,
                instrumented,
                false,
                Some(cache_bytes),
                None,
            )?);
            let initial = allocator.vma_snapshot()?;
            if !supported_capacity(initial, config.retained_count)? {
                println!(
                    "cache_refill_skipped cache_bytes={cache_bytes} instrumented={instrumented} retained={} vmas_before={} reason=preflight_headroom",
                    config.retained_count, initial.current
                );
                continue;
            }
            let retained = (0..config.retained_count)
                .map(|_| allocator.allocate())
                .collect::<Result<Vec<_>, _>>()?;
            let baseline = allocator.stats()?;
            for &worker_count in &config.workers {
                for force_refill in [false, true] {
                    run_refill_scenario(
                        &allocator,
                        worker_count,
                        force_refill,
                        cache_bytes,
                        &baseline,
                        config,
                    )?;
                }
                for burst_multiplier in [2, 4] {
                    run_refill_burst_scenario(
                        &allocator,
                        worker_count,
                        burst_multiplier,
                        cache_bytes,
                        &baseline,
                        config,
                    )?;
                }
                let refill_target = allocator
                    .cache_refill_target()
                    .ok_or("refill experiment requires the Go-inspired allocator")?;
                let refill_capacity = allocator
                    .cache_refill_capacity()
                    .ok_or("refill experiment requires the Go-inspired allocator")?;
                run_synchronized_refill_scenario(
                    &allocator,
                    worker_count,
                    refill_target,
                    cache_bytes,
                    &baseline,
                    config,
                )?;
                for (mode_name, mode) in [
                    ("fixed-default", DemandRefillMode::Fixed(refill_target)),
                    ("fixed-capacity", DemandRefillMode::Fixed(refill_capacity)),
                    (
                        "demand-adaptive",
                        DemandRefillMode::Adaptive {
                            baseline: refill_target,
                            capacity: refill_capacity,
                        },
                    ),
                ] {
                    run_demand_refill_scenario(
                        &allocator,
                        worker_count,
                        mode_name,
                        mode,
                        cache_bytes,
                        &baseline,
                        config,
                    )?;
                }
                for cache_retention_operations in [1, 8, 64] {
                    run_lifetime_scenario(
                        &allocator,
                        worker_count,
                        cache_retention_operations,
                        cache_bytes,
                        &baseline,
                        config,
                    )?;
                }
            }
            drop(retained);
            let final_state = allocator.stats()?;
            if final_state.active != 0 {
                return Err(format!(
                    "Go-inspired cache-size run left {} active stacks",
                    final_state.active
                )
                .into());
            }
        }
    }
    Ok(())
}
