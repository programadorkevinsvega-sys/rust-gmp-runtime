use crate::stack::StackError;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::manager::Inner;
use super::metrics::AtomicMetrics;
use super::slot::SlotToken;
use super::span::Span;

impl Inner {
    pub fn trim(&self) -> Result<usize, StackError> {
        self.release_spans(self.layout.retained_spans)
    }

    pub fn trim_to_retained_spans(&self, retained_spans: usize) -> Result<usize, StackError> {
        self.release_spans(retained_spans)
    }

    pub fn release_empty_spans(&self) -> Result<usize, StackError> {
        self.release_spans(0)
    }

    fn release_spans(&self, retained_spans: usize) -> Result<usize, StackError> {
        let mut state = self.pool.lock_class(0, &self.metrics)?;
        let clean_free_spans = state
            .spans
            .iter()
            .filter(|span| self.is_pool_owned_and_empty(span) && span.is_wholly_free())
            .count();
        let retained_clean_spans = retained_spans.min(clean_free_spans);
        let releasable_count = state
            .spans
            .iter()
            .filter(|span| self.is_pool_owned_and_empty(span) && span.is_releasable())
            .count();
        let trim_count = releasable_count.saturating_sub(retained_clean_spans);
        if trim_count == 0 {
            return Ok(0);
        }

        let mut ids: Vec<u64> = Vec::new();
        ids.try_reserve_exact(trim_count)
            .map_err(|error| allocation_error("prepare stack-span trim", error))?;
        let clean_spans_to_release = clean_free_spans - retained_clean_spans;
        let mut selected_clean_spans = 0;
        for span in &state.spans {
            if ids.len() == trim_count {
                break;
            }
            if !self.is_pool_owned_and_empty(span) || !span.is_releasable() {
                continue;
            }
            if span.is_wholly_free() && selected_clean_spans >= clean_spans_to_release {
                continue;
            }
            ids.push(span.id);
            if span.is_wholly_free() {
                selected_clean_spans += 1;
            }
        }

        let mut released = 0_usize;
        for id in ids {
            let Some(position) = state.spans.iter().position(|span| span.id == id) else {
                return Err(StackError::MetricsInvariant {
                    operation: "find selected Go-inspired span for trim",
                    observed: id.to_string(),
                });
            };
            let mut span = state.spans.swap_remove(position);
            let prior_free_len = state.free.len();
            state.free.retain(|slot| slot.span.id != id);
            let free_slots = prior_free_len - state.free.len();
            let slot_count = span.slot_count();
            if free_slots != span.global_free.load(Ordering::Acquire) {
                restore_span(&mut state, span);
                return Err(StackError::MetricsInvariant {
                    operation: "match Go-inspired free slots before trim",
                    observed: id.to_string(),
                });
            }

            if let Err(error) = self.admission.begin_release(slot_count) {
                restore_span(&mut state, span);
                return Err(error);
            }

            let release_result = match Arc::get_mut(&mut span) {
                Some(span) => span.mapping.release(),
                None => {
                    restore_span(&mut state, span);
                    self.admission.cancel_release(slot_count)?;
                    return Err(StackError::MetricsInvariant {
                        operation: "release referenced Go-inspired span",
                        observed: id.to_string(),
                    });
                }
            };

            if let Err(original) = release_result {
                restore_span(&mut state, span);
                self.admission.cancel_release(slot_count)?;
                return Err(original);
            }

            update_trim_metrics(self, &span, free_slots)?;
            self.admission.finish_release(slot_count)?;
            released = released.checked_add(1).ok_or(StackError::SizeOverflow)?;
            drop(span);
        }
        Ok(released)
    }

    fn is_pool_owned_and_empty(&self, span: &Arc<Span>) -> bool {
        span.active.load(Ordering::Acquire) == 0
            && span.cached.load(Ordering::Acquire) == 0
            && span.global_free.load(Ordering::Acquire) + span.quarantined.load(Ordering::Acquire)
                == span.slot_count()
            && Arc::strong_count(span) == span.global_free.load(Ordering::Acquire).saturating_add(1)
    }
}

fn restore_span(state: &mut super::pool::ClassPoolState, span: Arc<Span>) {
    for index in 0..span.slot_count() {
        if span.is_global_free(index) {
            state.free.push(SlotToken {
                span: Arc::clone(&span),
                index,
            });
        }
    }
    state.spans.push(span);
}

fn update_trim_metrics(inner: &Inner, span: &Span, free_slots: usize) -> Result<(), StackError> {
    AtomicMetrics::subtract(
        &inner.metrics.global_free_slots,
        to_u64(free_slots, "trimmed global stack slots")?,
    )?;
    AtomicMetrics::subtract(
        &inner.metrics.reserved_virtual_bytes,
        to_u64(inner.layout.span_size, "trimmed span bytes")?,
    )?;
    let accessible = span
        .enabled_slots()
        .checked_mul(span.usable_size())
        .ok_or(StackError::SizeOverflow)?;
    AtomicMetrics::subtract(
        &inner.metrics.accessible_virtual_bytes,
        to_u64(accessible, "trimmed accessible stack bytes")?,
    )?;
    AtomicMetrics::add(&inner.metrics.spans_released, 1)?;
    Ok(())
}

fn to_u64(value: usize, operation: &'static str) -> Result<u64, StackError> {
    u64::try_from(value).map_err(|source| StackError::IntegerConversion { operation, source })
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
