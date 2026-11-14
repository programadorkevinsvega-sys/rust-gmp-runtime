use super::manager::BlockStackInner;
use super::memory::Mapping;
use super::state::{Arena, BlockState, SlotStatus};
use crate::stack::error::StackError;

pub(super) struct SlotReservation {
    pub(super) arena_index: usize,
    pub(super) slot_index: usize,
    pub(super) address: usize,
    pub(super) usable_address: usize,
}

pub(super) fn reserve_slot(inner: &BlockStackInner) -> Result<SlotReservation, StackError> {
    loop {
        let mut state = inner.lock()?;
        let _hold = inner.timing.time(super::timing::TimingKind::LockHold);
        if let Some(reservation) = take_free_slot(inner, &mut state)? {
            return Ok(reservation);
        }
        if state.creating_arena {
            drop(_hold);
            state = inner.wait(state)?;
            drop(state);
            continue;
        }
        state.creating_arena = true;
        drop(_hold);
        drop(state);

        let mapping = {
            let _timer = inner.timing.time(super::timing::TimingKind::Mmap);
            inner.reserve_arena()
        };
        let mut state = inner.lock()?;
        let _hold = inner.timing.time(super::timing::TimingKind::LockHold);
        state.creating_arena = false;
        return match mapping {
            Ok(mapping) => insert_arena(inner, &mut state, mapping),
            Err(error) => {
                inner.shared.changed.notify_all();
                drop(_hold);
                drop(state);
                Err(error)
            }
        };
    }
}

fn insert_arena(
    inner: &BlockStackInner,
    state: &mut BlockState,
    mapping: Mapping,
) -> Result<SlotReservation, StackError> {
    let reserved =
        u64::try_from(inner.arena_size).map_err(|source| StackError::IntegerConversion {
            operation: "block arena size",
            source,
        })?;
    let free_count = u64::try_from(inner.slots_per_arena - 1).map_err(|source| {
        StackError::IntegerConversion {
            operation: "free block slot count",
            source,
        }
    })?;
    let arenas = state
        .metrics
        .arenas
        .checked_add(1)
        .ok_or(StackError::MetricsOverflow)?;
    let bytes = state
        .metrics
        .reserved_virtual_bytes
        .checked_add(reserved)
        .ok_or(StackError::MetricsOverflow)?;
    let free_slots = state
        .metrics
        .free_slots
        .checked_add(free_count)
        .ok_or(StackError::MetricsOverflow)?;
    let address = mapping.address(0, inner.slot_size)?;
    let usable_address = address
        .checked_add(inner.page_size)
        .ok_or(StackError::SizeOverflow)?;
    let mut slots = vec![SlotStatus::Free; inner.slots_per_arena];
    slots[0] = SlotStatus::Enabling;
    let mut used = vec![false; inner.slots_per_arena];
    used[0] = true;
    let mut available = Vec::with_capacity(inner.slots_per_arena);
    available.extend(1..inner.slots_per_arena);
    state.arenas.push(Arena {
        mapping,
        slots,
        used,
        available,
    });
    state.metrics.arenas = arenas;
    state.metrics.reserved_virtual_bytes = bytes;
    state.metrics.free_slots = free_slots;
    inner.shared.changed.notify_all();
    Ok(SlotReservation {
        arena_index: state.arenas.len() - 1,
        slot_index: 0,
        address,
        usable_address,
    })
}

fn take_free_slot(
    inner: &BlockStackInner,
    state: &mut BlockState,
) -> Result<Option<SlotReservation>, StackError> {
    for arena_index in 0..state.arenas.len() {
        if let Some(slot_index) = state.arenas[arena_index].available.last().copied() {
            if state.arenas[arena_index].slots.get(slot_index) != Some(&SlotStatus::Free) {
                return Err(StackError::MetricsOverflow);
            }
            if state.arenas[arena_index].available.pop() != Some(slot_index) {
                return Err(StackError::MetricsOverflow);
            }
            let offset = slot_index
                .checked_mul(inner.slot_size)
                .ok_or(StackError::SizeOverflow)?;
            let address = state.arenas[arena_index]
                .mapping
                .address(offset, inner.slot_size)?;
            let usable_address = address
                .checked_add(inner.page_size)
                .ok_or(StackError::SizeOverflow)?;
            let free_slots = state
                .metrics
                .free_slots
                .checked_sub(1)
                .ok_or(StackError::MetricsOverflow)?;
            let reused_slots = if state.arenas[arena_index].used[slot_index] {
                state.metrics.reused_slots.checked_add(1)
            } else {
                Some(state.metrics.reused_slots)
            }
            .ok_or(StackError::MetricsOverflow)?;
            state.arenas[arena_index].slots[slot_index] = SlotStatus::Enabling;
            state.arenas[arena_index].used[slot_index] = true;
            state.metrics.free_slots = free_slots;
            state.metrics.reused_slots = reused_slots;
            return Ok(Some(SlotReservation {
                arena_index,
                slot_index,
                address,
                usable_address,
            }));
        }
    }
    Ok(None)
}
