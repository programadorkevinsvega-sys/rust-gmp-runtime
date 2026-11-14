use super::*;

impl Allocator {
    pub fn create(
        strategy: &str,
        config: StackConfig,
        slots_per_arena: usize,
        instrumented: bool,
        madvise: bool,
    ) -> Result<Self, Box<dyn Error>> {
        Self::create_with_go_cache_bytes(
            strategy,
            config,
            slots_per_arena,
            instrumented,
            madvise,
            None,
            None,
        )
    }

    pub fn create_with_go_cache_bytes(
        strategy: &str,
        config: StackConfig,
        slots_per_arena: usize,
        instrumented: bool,
        madvise: bool,
        cache_bytes: Option<usize>,
        retained_spans: Option<usize>,
    ) -> Result<Self, Box<dyn Error>> {
        match (strategy, instrumented) {
            ("fixed" | "fixed-serial", true) => {
                let (manager, timing) = StackManager::new_benchmarked(config)?;
                Ok(Self::Fixed {
                    manager,
                    timing: Some(timing),
                })
            }
            ("fixed" | "fixed-serial", false) => Ok(Self::Fixed {
                manager: StackManager::new(config)?,
                timing: None,
            }),
            ("fixed-two-phase", true) => {
                let (manager, timing) = StackManager::new_benchmarked_two_phase(config)?;
                Ok(Self::Fixed {
                    manager,
                    timing: Some(timing),
                })
            }
            ("fixed-two-phase", false) => Ok(Self::Fixed {
                manager: StackManager::new_benchmark_two_phase_uninstrumented(config)?,
                timing: None,
            }),
            ("block", true) => {
                let manager = BlockStackManager::new_benchmarked(config, slots_per_arena, madvise)?;
                let timing = Some(manager.timing_handle());
                Ok(Self::Block { manager, timing })
            }
            ("block", false) => Ok(Self::Block {
                manager: BlockStackManager::new_benchmark_uninstrumented(
                    config,
                    slots_per_arena,
                    madvise,
                )?,
                timing: None,
            }),
            ("go-inspired", _) => {
                let mut options = GoInspiredOptions::default().with_slots_per_span(slots_per_arena);
                if let Some(cache_bytes) = cache_bytes {
                    options = options.with_cache_bytes(cache_bytes);
                }
                if let Some(retained_spans) = retained_spans {
                    options = options.with_retained_spans(retained_spans);
                }
                let manager = if instrumented {
                    GoInspiredStackManager::new_benchmarked(config, options)?
                } else {
                    GoInspiredStackManager::with_options(config, options)?
                };
                Ok(Self::GoInspired {
                    manager,
                    instrumented,
                })
            }
            (unknown, _) => Err(format!("unknown allocator strategy: {unknown}").into()),
        }
    }
}
