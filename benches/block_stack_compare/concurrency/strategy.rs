use super::*;

pub(super) fn run_strategy(
    strategy: &str,
    stack_bytes: usize,
    slots_per_arena: usize,
    instrumented: bool,
    config: &ConcurrencyConfig,
) -> Result<(), Box<dyn Error>> {
    let allocator = Arc::new(Allocator::create(
        strategy,
        StackConfig::new(stack_bytes)?,
        slots_per_arena,
        instrumented,
        false,
    )?);
    let initial = allocator.vma_snapshot()?;
    if !supported_capacity(initial, config.retained_count)? {
        println!(
            "concurrency_skipped strategy={strategy} instrumented={instrumented} retained={} vmas_before={} reason=preflight_headroom",
            config.retained_count, initial.current
        );
        return Ok(());
    }
    let retained = (0..config.retained_count)
        .map(|_| allocator.allocate())
        .collect::<Result<Vec<_>, _>>()?;
    for stack in retained.iter().step_by(100) {
        stack.touch();
    }
    let baseline = allocator.stats()?;
    allocator.take_timing();
    for snapshots in [false, true] {
        for &worker_count in &config.workers {
            run_scenario(
                &allocator,
                strategy,
                instrumented,
                snapshots,
                worker_count,
                &baseline,
                config,
            )?;
        }
    }
    drop(retained);
    let final_state = allocator.stats()?;
    if final_state.active != 0 {
        return Err(format!(
            "{strategy} left {} stacks after the retained set was dropped",
            final_state.active
        )
        .into());
    }
    let timing = allocator.timer_handle();
    drop(allocator);
    let manager_timing = timing
        .map(|handle| handle.snapshot_and_reset())
        .unwrap_or_else(|| "disabled".to_owned());
    println!(
        "concurrency_final strategy={strategy} instrumented={instrumented} manager_drop_timing={manager_timing} vmas_after_manager_drop={}",
        current_vma_count()?,
    );
    Ok(())
}
