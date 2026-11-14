use super::manager::{BlockStack, BlockStackInner};
use super::memory::Mapping;
use super::state::SlotStatus;
use super::timing::TimingKind;
use crate::stack::error::StackError;

pub(super) fn release_slot(
    inner: &BlockStackInner,
    arena_index: usize,
    slot_index: usize,
) -> Result<(), StackError> {
    let (address, accounting_error) = {
        let mut state = inner.lock()?;
        let _hold = inner.timing.time(TimingKind::LockHold);
        let arena = state
            .arenas
            .get(arena_index)
            .ok_or(StackError::MetricsOverflow)?;
        if arena.slots.get(slot_index) != Some(&SlotStatus::Occupied) {
            return Err(StackError::MetricsOverflow);
        }
        let offset = slot_index
            .checked_mul(inner.slot_size)
            .and_then(|offset| offset.checked_add(inner.page_size))
            .ok_or(StackError::SizeOverflow)?;
        let address = arena.mapping.address(offset, inner.usable_size)?;
        let accounting_error = if inner.safety_margin.is_some() {
            match state.admission.begin_release() {
                Ok(()) => None,
                Err(error) => Some(error),
            }
        } else {
            None
        };
        state.arenas[arena_index].slots[slot_index] = SlotStatus::Disabling;
        (address, accounting_error)
    };

    let protection = {
        let _timer = inner.timing.time(TimingKind::Mprotect);
        inner.disable_slot(address)
    };
    if let Err(error) = protection {
        return quarantine_slot(inner, arena_index, slot_index, error);
    }

    let advice = if inner.madvise_dontneed {
        let _timer = inner.timing.time(TimingKind::Madvise);
        Mapping::discard_at(address, inner.usable_size)
    } else {
        Ok(())
    };
    let mut state = inner.lock()?;
    let _hold = inner.timing.time(TimingKind::LockHold);
    let free_slots = state
        .metrics
        .free_slots
        .checked_add(1)
        .ok_or(StackError::MetricsOverflow)?;
    let occupied_slots = state
        .metrics
        .occupied_slots
        .checked_sub(1)
        .ok_or(StackError::MetricsOverflow)?;
    let active_stacks = state
        .metrics
        .active_stacks
        .checked_sub(1)
        .ok_or(StackError::MetricsOverflow)?;
    let accessible_bytes = state
        .metrics
        .accessible_virtual_bytes
        .checked_sub(u64::try_from(inner.usable_size).map_err(|source| {
            StackError::IntegerConversion {
                operation: "usable stack size",
                source,
            }
        })?)
        .ok_or(StackError::MetricsOverflow)?;
    let releases = state
        .metrics
        .releases
        .checked_add(1)
        .ok_or(StackError::MetricsOverflow)?;
    state.arenas[arena_index].slots[slot_index] = SlotStatus::Free;
    state.arenas[arena_index].available.push(slot_index);
    state.metrics.free_slots = free_slots;
    state.metrics.occupied_slots = occupied_slots;
    state.metrics.active_stacks = active_stacks;
    state.metrics.accessible_virtual_bytes = accessible_bytes;
    state.metrics.releases = releases;
    if inner.safety_margin.is_some() && accounting_error.is_none() {
        if let Err(error) = state.admission.finish_release() {
            state.metrics.cleanup_failures = state
                .metrics
                .cleanup_failures
                .checked_add(1)
                .ok_or(StackError::MetricsOverflow)?;
            inner.shared.changed.notify_all();
            return Err(error);
        }
    }
    if let Err(error) = advice {
        state.metrics.cleanup_failures = state
            .metrics
            .cleanup_failures
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        inner.shared.changed.notify_all();
        return match accounting_error {
            Some(accounting) => Err(combine(error, accounting)),
            None => Err(error),
        };
    }
    inner.shared.changed.notify_all();
    match accounting_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn quarantine_slot(
    inner: &BlockStackInner,
    arena_index: usize,
    slot_index: usize,
    error: StackError,
) -> Result<(), StackError> {
    let mut state = inner.lock()?;
    let _hold = inner.timing.time(TimingKind::LockHold);
    let occupied = state
        .metrics
        .occupied_slots
        .checked_sub(1)
        .ok_or(StackError::MetricsOverflow)?;
    let active = state
        .metrics
        .active_stacks
        .checked_sub(1)
        .ok_or(StackError::MetricsOverflow)?;
    let poisoned = state
        .metrics
        .poisoned_slots
        .checked_add(1)
        .ok_or(StackError::MetricsOverflow)?;
    let failures = state
        .metrics
        .cleanup_failures
        .checked_add(1)
        .ok_or(StackError::MetricsOverflow)?;
    state.arenas[arena_index].slots[slot_index] = SlotStatus::Poisoned;
    state.metrics.occupied_slots = occupied;
    state.metrics.active_stacks = active;
    state.metrics.poisoned_slots = poisoned;
    state.metrics.cleanup_failures = failures;
    inner.shared.changed.notify_all();
    Err(error)
}

fn combine(original: StackError, cleanup: StackError) -> StackError {
    StackError::CleanupAfterFailure {
        original: Box::new(original),
        cleanup: Box::new(cleanup),
    }
}

impl BlockStack {
    /// Returns stable numeric bounds for the guard and accessible pages.
    pub fn bounds(&self) -> (usize, usize, usize) {
        let start = self.slot_start;
        (
            start,
            start + self.inner.page_size,
            start + self.inner.slot_size,
        )
    }

    /// Returns usable bytes in this slot.
    pub fn usable_size(&self) -> usize {
        self.inner.usable_size
    }

    /// Releases the slot and reports protection/advice failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the slot cannot be protected, memory advice fails,
    /// or allocator accounting becomes inconsistent.
    pub fn release(mut self) -> Result<(), StackError> {
        self.released = true;
        release_slot(&self.inner, self.arena_index, self.slot_index)
    }
}
