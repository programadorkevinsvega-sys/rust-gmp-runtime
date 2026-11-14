use super::*;

pub(super) fn run_reuse(
    strategy: &str,
    cycles: usize,
    samples: usize,
    stack_bytes: usize,
    slots_per_arena: usize,
    madvise: bool,
) -> Result<(), Box<dyn Error>> {
    for sample in 0..samples {
        for instrumented in [false, true] {
            let config = StackConfig::new(stack_bytes)?;
            let allocator =
                Allocator::create(strategy, config, slots_per_arena, instrumented, madvise)?;
            let warm = (0..WARM_SLOTS)
                .map(|_| allocator.allocate())
                .collect::<Result<Vec<_>, _>>()?;
            for stack in &warm {
                stack.touch_all_pages(allocator.page_size());
            }
            drop(warm);
            let after_warm_release = allocator.stats()?;
            let before = allocator.stats()?;
            allocator.take_timing();
            let mut latencies = if instrumented {
                Vec::with_capacity(cycles)
            } else {
                Vec::new()
            };
            let started = Instant::now();
            for _ in 0..cycles {
                let operation_started = instrumented.then(Instant::now);
                let stack = allocator.allocate()?;
                stack.touch_all_pages(allocator.page_size());
                drop(stack);
                if let Some(operation_started) = operation_started {
                    latencies.push(
                        u64::try_from(operation_started.elapsed().as_nanos()).unwrap_or(u64::MAX),
                    );
                }
            }
            let elapsed_ns = started.elapsed().as_nanos();
            let timing = allocator.take_timing();
            let after = allocator.stats()?;
            let throughput = cycles as f64 / (elapsed_ns as f64 / 1_000_000_000.0);
            let distribution = distribution(&mut latencies);
            let timing_handle = allocator.timer_handle();
            drop(allocator);
            let manager_timing = timing_handle
                .map(|handle| handle.snapshot_and_reset())
                .unwrap_or_else(|| "disabled".to_owned());
            println!(
                "reuse_sample strategy={strategy} madvise={madvise} instrumented={instrumented} sample={sample} samples={samples} touched_pages_per_cycle=true cycles={cycles} elapsed_ns={elapsed_ns} ops_s={throughput:.2} p50_p95_p99_max_ns={distribution:?} timings={timing} manager_drop_timing={manager_timing} memory_after_warm_release={after_warm_release} before={before} after={after}",
            );
        }
    }
    Ok(())
}
