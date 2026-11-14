use super::memory::Mapping;
use super::timing::Timing;
use crate::stack::admission::VmaAdmission;
use crate::stack::{StackError, VmaSnapshot};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SlotStatus {
    Free,
    Enabling,
    Occupied,
    Disabling,
    Poisoned,
}

#[derive(Debug)]
pub(super) struct Arena {
    pub(super) mapping: Mapping,
    pub(super) slots: Vec<SlotStatus>,
    pub(super) used: Vec<bool>,
    pub(super) available: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
/// Allocator state snapshot for the benchmark-only arena manager.
pub struct BlockStackMetrics {
    /// Successfully allocated block-stack handles.
    pub allocations: u64,
    /// Successfully protected and returned slots.
    pub releases: u64,
    /// Failed allocation attempts, including preflight rejection.
    pub allocation_failures: u64,
    /// Failed cleanup or optional memory-advice operations.
    pub cleanup_failures: u64,
    /// Arenas retained by this manager.
    pub arenas: u64,
    /// Slots currently owned by live handles.
    pub active_stacks: u64,
    /// Slots currently available for reuse.
    pub free_slots: u64,
    /// Slots currently assigned to live handles.
    pub occupied_slots: u64,
    /// Slots excluded from reuse after a protection failure.
    pub poisoned_slots: u64,
    /// Allocations served from a previously used slot.
    pub reused_slots: u64,
    /// Virtual address space reserved by retained arenas.
    pub reserved_virtual_bytes: u64,
    /// Enabled or quarantined pages that may still be read/write accessible.
    pub accessible_virtual_bytes: u64,
    /// Most recent process VMA snapshot, when one has been read.
    pub vma_snapshot: Option<VmaSnapshot>,
}

#[derive(Default)]
pub(super) struct BlockState {
    pub(super) arenas: Vec<Arena>,
    pub(super) creating_arena: bool,
    pub(super) admission: VmaAdmission,
    pub(super) metrics: BlockStackMetrics,
}

pub(super) struct SharedState {
    pub(super) state: Mutex<BlockState>,
    pub(super) changed: std::sync::Condvar,
}

impl SharedState {
    pub(super) fn lock(&self) -> Result<std::sync::MutexGuard<'_, BlockState>, StackError> {
        self.state
            .lock()
            .map_err(|error| StackError::AdmissionPoisoned {
                message: error.to_string(),
            })
    }

    pub(super) fn wait<'a>(
        &self,
        guard: std::sync::MutexGuard<'a, BlockState>,
        timing: &Timing,
    ) -> Result<std::sync::MutexGuard<'a, BlockState>, StackError> {
        let started = timing.started();
        let guard = self
            .changed
            .wait(guard)
            .map_err(|error| StackError::AdmissionPoisoned {
                message: error.to_string(),
            })?;
        timing.record_condvar_wait(started);
        Ok(guard)
    }

    pub(super) fn lock_timed(
        &self,
        timing: &Timing,
    ) -> Result<std::sync::MutexGuard<'_, BlockState>, StackError> {
        let started = timing.started();
        let guard = self.lock()?;
        timing.record_lock_wait(started);
        Ok(guard)
    }
}
