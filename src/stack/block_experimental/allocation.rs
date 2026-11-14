use super::arena::{SlotReservation, reserve_slot};
use super::budget;
use super::failure::{cancel_after_failure, combine, record_allocation_failure};
use super::manager::{BlockStack, BlockStackInner};
use super::state::SlotStatus;
use super::timing::TimingKind;
use crate::stack::error::StackError;
use std::sync::Arc;

pub(super) fn allocate_stack(inner: &Arc<BlockStackInner>) -> Result<BlockStack, StackError> {
    let usable_bytes =
        u64::try_from(inner.usable_size).map_err(|source| StackError::IntegerConversion {
            operation: "usable stack size",
            source,
        })?;
    if let Err(error) = budget::reserve(inner) {
        return Err(record_allocation_failure(inner, error));
    }
    let reservation = match reserve_slot(inner) {
        Ok(reservation) => reservation,
        Err(error) => {
            return Err(cancel_after_failure(inner, error));
        }
    };
    let enable = {
        let _timer = inner.timing.time(TimingKind::Mprotect);
        inner.enable_slot(reservation.usable_address)
    };
    if let Err(error) = enable {
        return Err(rollback_enable(inner, reservation, error));
    }

    let mut state = match inner.lock() {
        Ok(state) => state,
        Err(error) => {
            let cleanup = inner.disable_slot(reservation.usable_address);
            return Err(combine(error, cleanup));
        }
    };
    let _hold = inner.timing.time(TimingKind::LockHold);
    if state
        .arenas
        .get(reservation.arena_index)
        .and_then(|arena| arena.slots.get(reservation.slot_index))
        != Some(&SlotStatus::Enabling)
    {
        drop(_hold);
        drop(state);
        return Err(rollback_enable(
            inner,
            reservation,
            StackError::MetricsOverflow,
        ));
    }
    let occupied = state.metrics.occupied_slots.checked_add(1);
    let active = state.metrics.active_stacks.checked_add(1);
    let accessible = state
        .metrics
        .accessible_virtual_bytes
        .checked_add(usable_bytes);
    let allocations = state.metrics.allocations.checked_add(1);
    let (Some(occupied), Some(active), Some(accessible), Some(allocations)) =
        (occupied, active, accessible, allocations)
    else {
        drop(_hold);
        drop(state);
        return Err(rollback_enable(
            inner,
            reservation,
            StackError::MetricsOverflow,
        ));
    };
    if inner.safety_margin.is_some() {
        if let Err(error) = state.admission.finish_allocation() {
            drop(_hold);
            drop(state);
            return Err(rollback_enable(inner, reservation, error));
        }
    }
    state.arenas[reservation.arena_index].slots[reservation.slot_index] = SlotStatus::Occupied;
    state.metrics.occupied_slots = occupied;
    state.metrics.active_stacks = active;
    state.metrics.accessible_virtual_bytes = accessible;
    state.metrics.allocations = allocations;
    inner.shared.changed.notify_all();
    drop(_hold);
    drop(state);
    Ok(BlockStack {
        inner: Arc::clone(inner),
        arena_index: reservation.arena_index,
        slot_index: reservation.slot_index,
        slot_start: reservation.address,
        released: false,
    })
}

fn rollback_enable(
    inner: &BlockStackInner,
    reservation: SlotReservation,
    original: StackError,
) -> StackError {
    let cleanup = {
        let _timer = inner.timing.time(TimingKind::Mprotect);
        inner.disable_slot(reservation.usable_address)
    };
    let mut state = match inner.lock() {
        Ok(state) => state,
        Err(error) => return combine(original, Err(error)),
    };
    let cleanup_error = match cleanup {
        Ok(()) => {
            let free_slots = state.metrics.free_slots.checked_add(1);
            if let Some(free_slots) = free_slots {
                state.metrics.free_slots = free_slots;
            } else {
                state.arenas[reservation.arena_index].slots[reservation.slot_index] =
                    SlotStatus::Poisoned;
                state.metrics.poisoned_slots = match state.metrics.poisoned_slots.checked_add(1) {
                    Some(count) => count,
                    None => {
                        drop(state);
                        return combine(original, Err(StackError::MetricsOverflow));
                    }
                };
                drop(state);
                return combine(original, Err(StackError::MetricsOverflow));
            }
            state.arenas[reservation.arena_index].slots[reservation.slot_index] = SlotStatus::Free;
            state.arenas[reservation.arena_index]
                .available
                .push(reservation.slot_index);
            None
        }
        Err(cleanup_error) => {
            let poisoned = match state.metrics.poisoned_slots.checked_add(1) {
                Some(count) => count,
                None => return combine(original, Err(StackError::MetricsOverflow)),
            };
            let failures = match state.metrics.cleanup_failures.checked_add(1) {
                Some(count) => count,
                None => return combine(original, Err(StackError::MetricsOverflow)),
            };
            let usable = match u64::try_from(inner.usable_size) {
                Ok(usable) => usable,
                Err(source) => {
                    return combine(
                        original,
                        Err(StackError::IntegerConversion {
                            operation: "quarantined stack size",
                            source,
                        }),
                    );
                }
            };
            let accessible = match state.metrics.accessible_virtual_bytes.checked_add(usable) {
                Some(accessible) => accessible,
                None => return combine(original, Err(StackError::MetricsOverflow)),
            };
            state.arenas[reservation.arena_index].slots[reservation.slot_index] =
                SlotStatus::Poisoned;
            state.metrics.poisoned_slots = poisoned;
            state.metrics.cleanup_failures = failures;
            state.metrics.accessible_virtual_bytes = accessible;
            Some(cleanup_error)
        }
    };
    let allocation_failures = match state.metrics.allocation_failures.checked_add(1) {
        Some(count) => count,
        None => return combine(original, Err(StackError::MetricsOverflow)),
    };
    state.metrics.allocation_failures = allocation_failures;
    inner.shared.changed.notify_all();
    drop(state);
    let cancellation = budget::cancel(inner);
    let error = match cleanup_error {
        Some(cleanup) => combine(original, Err(cleanup)),
        None => original,
    };
    match cancellation {
        Ok(()) => error,
        Err(cancellation) => combine(error, Err(cancellation)),
    }
}
