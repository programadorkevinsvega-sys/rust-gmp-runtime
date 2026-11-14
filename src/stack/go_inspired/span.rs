use crate::stack::StackError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

use super::layout::ValidatedLayout;
use super::memory::SpanMapping;
use super::slot::{
    ACTIVATING, GLOBAL_FREE, LIVE, LOCAL_FREE, NEW, QUARANTINED, SlotState, SlotToken, decrement,
    increment,
};

#[derive(Debug)]
pub(super) struct Span {
    pub id: u64,
    pub mapping: SpanMapping,
    slots: Vec<SlotState>,
    pub active: AtomicUsize,
    pub enabled: AtomicUsize,
    pub cached: AtomicUsize,
    pub global_free: AtomicUsize,
    pub quarantined: AtomicUsize,
    layout: ValidatedLayout,
}

impl Span {
    pub fn new(id: u64, mapping: SpanMapping, layout: ValidatedLayout) -> Result<Self, StackError> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(layout.slots_per_span)
            .map_err(|error| allocation_error("reserve span slot metadata", error))?;
        slots.extend((0..layout.slots_per_span).map(|_| SlotState {
            state: AtomicU8::new(NEW),
            enabled: AtomicBool::new(false),
        }));
        Ok(Self {
            id,
            mapping,
            slots,
            active: AtomicUsize::new(0),
            enabled: AtomicUsize::new(0),
            cached: AtomicUsize::new(0),
            global_free: AtomicUsize::new(0),
            quarantined: AtomicUsize::new(0),
            layout,
        })
    }

    pub fn token(self: &Arc<Self>, index: usize) -> Result<SlotToken, StackError> {
        if index >= self.slots.len() {
            return Err(StackError::MetricsOverflow);
        }
        Ok(SlotToken {
            span: Arc::clone(self),
            index,
        })
    }

    pub fn is_wholly_free(&self) -> bool {
        self.active.load(Ordering::Acquire) == 0
            && self.cached.load(Ordering::Acquire) == 0
            && self.quarantined.load(Ordering::Acquire) == 0
            && self.global_free.load(Ordering::Acquire) == self.slots.len()
    }

    pub fn is_releasable(&self) -> bool {
        self.active.load(Ordering::Acquire) == 0
            && self.cached.load(Ordering::Acquire) == 0
            && self
                .global_free
                .load(Ordering::Acquire)
                .checked_add(self.quarantined.load(Ordering::Acquire))
                == Some(self.slots.len())
    }

    pub fn is_global_free(&self, index: usize) -> bool {
        self.slots
            .get(index)
            .is_some_and(|slot| slot.state.load(Ordering::Acquire) == GLOBAL_FREE)
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn stride(&self) -> usize {
        self.layout.stride
    }

    pub fn usable_size(&self) -> usize {
        self.layout.usable_size
    }

    pub fn slot_start(&self, index: usize) -> Result<usize, StackError> {
        self.mapping.slot_start(index, self.layout.stride)
    }

    pub fn activate(&self, token: &SlotToken, prior: u8) -> Result<bool, StackError> {
        let slot = self.slot(token.index)?;
        slot.state
            .compare_exchange(prior, ACTIVATING, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|observed| StackError::MetricsInvariant {
                operation: "activate Go-inspired stack slot",
                observed: observed.to_string(),
            })?;
        if slot.enabled.load(Ordering::Acquire) {
            slot.state.store(LIVE, Ordering::Release);
            self.move_counter(prior, LIVE)?;
            return Ok(false);
        }
        let address = match self.slot_start(token.index) {
            Ok(address) => address,
            Err(error) => return self.quarantine_failure(slot, prior, error),
        };
        if let Err(error) =
            self.mapping
                .enable(address, self.layout.page_size, self.layout.usable_size)
        {
            return self.quarantine_failure(slot, prior, error);
        }
        slot.enabled.store(true, Ordering::Release);
        increment(&self.enabled)?;
        slot.state.store(LIVE, Ordering::Release);
        self.move_counter(prior, LIVE)?;
        Ok(true)
    }

    pub fn enabled_slots(&self) -> usize {
        self.enabled.load(Ordering::Acquire)
    }

    pub fn set_free_state(&self, index: usize, prior: u8, next: u8) -> Result<(), StackError> {
        let slot = self.slot(index)?;
        slot.state
            .compare_exchange(prior, next, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|observed| StackError::MetricsInvariant {
                operation: "change Go-inspired stack slot state",
                observed: observed.to_string(),
            })?;
        self.move_counter(prior, next)
    }

    fn quarantine_failure<T>(
        &self,
        slot: &SlotState,
        prior: u8,
        error: StackError,
    ) -> Result<T, StackError> {
        slot.state.store(QUARANTINED, Ordering::Release);
        self.move_counter(prior, QUARANTINED)?;
        Err(error)
    }

    fn move_counter(&self, prior: u8, next: u8) -> Result<(), StackError> {
        match prior {
            GLOBAL_FREE => decrement(&self.global_free)?,
            LOCAL_FREE => decrement(&self.cached)?,
            LIVE => decrement(&self.active)?,
            ACTIVATING => {}
            _ => {}
        }
        match next {
            GLOBAL_FREE => increment(&self.global_free)?,
            LOCAL_FREE => increment(&self.cached)?,
            LIVE => increment(&self.active)?,
            QUARANTINED => increment(&self.quarantined)?,
            _ => {}
        }
        Ok(())
    }

    fn slot(&self, index: usize) -> Result<&SlotState, StackError> {
        self.slots.get(index).ok_or(StackError::MetricsOverflow)
    }
}

pub(super) fn global_free_state() -> u8 {
    GLOBAL_FREE
}

pub(super) fn new_state() -> u8 {
    NEW
}

pub(super) fn local_free_state() -> u8 {
    LOCAL_FREE
}

pub(super) fn live_state() -> u8 {
    LIVE
}

fn allocation_error(
    operation: &'static str,
    error: std::collections::TryReserveError,
) -> StackError {
    StackError::SystemCall {
        operation,
        source: std::io::Error::other(error),
    }
}
