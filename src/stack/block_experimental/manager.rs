use super::budget;
use super::memory::Mapping;
use super::state::{BlockState, SharedState};
use super::timing::Timing;
use crate::stack::{StackConfig, StackError, VmaSnapshot};
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

/// Cloneable handle to an isolated experimental arena allocator.
#[derive(Clone)]
pub struct BlockStackManager {
    pub(super) inner: Arc<BlockStackInner>,
}

pub(super) struct BlockStackInner {
    pub(super) shared: SharedState,
    pub(super) page_size: usize,
    pub(super) usable_size: usize,
    pub(super) slot_size: usize,
    pub(super) arena_size: usize,
    pub(super) slots_per_arena: usize,
    pub(super) safety_margin: Option<u64>,
    pub(super) madvise_dontneed: bool,
    pub(super) timing: Arc<Timing>,
    #[cfg(test)]
    pub(super) faults: FaultInjection,
}

#[cfg(test)]
pub(super) struct FaultInjection {
    pub(super) reserve: AtomicBool,
    pub(super) enable: AtomicBool,
    pub(super) disable: AtomicBool,
}

/// RAII handle to one stable-address slot in a live arena.
pub struct BlockStack {
    pub(super) inner: Arc<BlockStackInner>,
    pub(super) arena_index: usize,
    pub(super) slot_index: usize,
    pub(super) slot_start: usize,
    pub(super) released: bool,
}

impl BlockStackManager {
    /// Creates an uninstrumented manager with an optional benchmark-only
    /// `MADV_DONTNEED` release policy.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::new`].
    pub fn new_benchmark_uninstrumented(
        config: StackConfig,
        slots_per_arena: usize,
        madvise_dontneed: bool,
    ) -> Result<Self, StackError> {
        Self::build(config, slots_per_arena, madvise_dontneed, false)
    }

    /// Creates an uninstrumented experimental manager with preflight enabled by
    /// the supplied stack configuration.
    ///
    /// # Errors
    ///
    /// Returns an error for zero slot capacity, unsupported platforms, invalid
    /// system page size, or overflowing arena dimensions.
    pub fn new(config: StackConfig, slots_per_arena: usize) -> Result<Self, StackError> {
        Self::build(config, slots_per_arena, false, false)
    }

    /// Creates a measured manager; timings add overhead and are benchmark-only.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::new`].
    pub fn new_benchmarked(
        config: StackConfig,
        slots_per_arena: usize,
        madvise_dontneed: bool,
    ) -> Result<Self, StackError> {
        Self::build(config, slots_per_arena, madvise_dontneed, true)
    }

    fn build(
        config: StackConfig,
        slots_per_arena: usize,
        madvise_dontneed: bool,
        instrumented: bool,
    ) -> Result<Self, StackError> {
        if slots_per_arena == 0 {
            return Err(StackError::InvalidSize {
                requested_bytes: slots_per_arena,
            });
        }
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(StackError::UnsupportedPlatform);
        }
        let page_size = super::super::protection::dispatch::page_size()?;
        let usable_size =
            super::super::size::round_up_to_page(config.requested_usable_size(), page_size)?;
        let slot_size = page_size
            .checked_add(usable_size)
            .ok_or(StackError::SizeOverflow)?;
        let arena_size = slot_size
            .checked_mul(slots_per_arena)
            .ok_or(StackError::SizeOverflow)?;
        Ok(Self {
            inner: Arc::new(BlockStackInner {
                shared: SharedState {
                    state: std::sync::Mutex::new(BlockState::default()),
                    changed: std::sync::Condvar::new(),
                },
                page_size,
                usable_size,
                slot_size,
                arena_size,
                slots_per_arena,
                safety_margin: config
                    .vma_preflight()
                    .map(|settings| settings.safety_margin),
                madvise_dontneed,
                timing: Arc::new(Timing::new(instrumented)),
                #[cfg(test)]
                faults: FaultInjection {
                    reserve: AtomicBool::new(false),
                    enable: AtomicBool::new(false),
                    disable: AtomicBool::new(false),
                },
            }),
        })
    }
}

impl BlockStackInner {
    pub(super) fn reserve_arena(&self) -> Result<Mapping, StackError> {
        #[cfg(test)]
        if self.faults.reserve.swap(false, Ordering::SeqCst) {
            return Err(Self::injected_failure("mmap(block arena)"));
        }
        Mapping::reserve(self.arena_size)
    }

    pub(super) fn enable_slot(&self, address: usize) -> Result<(), StackError> {
        #[cfg(test)]
        if self.faults.enable.swap(false, Ordering::SeqCst) {
            return Err(Self::injected_failure("mprotect(enable block slot)"));
        }
        Mapping::enable_at(address, self.usable_size)
    }

    pub(super) fn disable_slot(&self, address: usize) -> Result<(), StackError> {
        #[cfg(test)]
        if self.faults.disable.swap(false, Ordering::SeqCst) {
            return Err(Self::injected_failure("mprotect(disable block slot)"));
        }
        Mapping::disable_at(address, self.usable_size)
    }

    pub(super) fn lock(&self) -> Result<std::sync::MutexGuard<'_, BlockState>, StackError> {
        self.shared.lock_timed(&self.timing)
    }

    #[cfg(test)]
    fn injected_failure(operation: &'static str) -> StackError {
        StackError::SystemCall {
            operation,
            source: std::io::Error::other("injected test failure"),
        }
    }

    pub(super) fn wait<'a>(
        &self,
        guard: std::sync::MutexGuard<'a, BlockState>,
    ) -> Result<std::sync::MutexGuard<'a, BlockState>, StackError> {
        self.shared.wait(guard, &self.timing)
    }

    pub(super) fn refresh_snapshot(&self) -> Result<VmaSnapshot, StackError> {
        budget::refresh_snapshot(self)
    }
}
