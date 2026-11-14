use crate::stack::{StackConfig, StackError, VmaSnapshot};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::admission::Admission;
use super::layout::{GoInspiredOptions, ValidatedLayout};
use super::memory::SpanMapping;
use super::metrics::AtomicMetrics;
use super::pool::GlobalStackPool;
use super::slot::SlotToken;
use super::span::Span;

pub(super) struct Inner {
    pub layout: ValidatedLayout,
    pub pool: GlobalStackPool,
    pub admission: Admission,
    pub metrics: Arc<AtomicMetrics>,
    next_span_id: AtomicU64,
    pub instrumented: bool,
}

impl Inner {
    pub fn new(
        config: StackConfig,
        options: GoInspiredOptions,
        instrumented: bool,
    ) -> Result<Arc<Self>, StackError> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(StackError::UnsupportedPlatform);
        }
        let page_size = super::super::protection::dispatch::page_size()?;
        let layout = options.validate(config, page_size)?;
        Ok(Arc::new(Self {
            layout,
            pool: GlobalStackPool::one_class(instrumented),
            admission: Admission::new(layout.safety_margin),
            metrics: Arc::new(AtomicMetrics::default()),
            next_span_id: AtomicU64::new(1),
            instrumented,
        }))
    }

    pub fn create_span(&self) -> Result<(), StackError> {
        self.admission.reserve_span(self.layout.slots_per_span)?;
        let mapping = match SpanMapping::reserve(
            self.layout.span_size,
            Arc::clone(&self.metrics),
            self.instrumented,
        ) {
            Ok(mapping) => mapping,
            Err(error) => return self.cancel_after_failure(error),
        };
        let id = match self
            .next_span_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
        {
            Ok(id) => id,
            Err(current) => {
                let error = StackError::MetricsInvariant {
                    operation: "allocate Go-inspired span identifier",
                    observed: current.to_string(),
                };
                return self.cancel_after_failure(error);
            }
        };
        let span = match Span::new(id, mapping, self.layout) {
            Ok(span) => Arc::new(span),
            Err(error) => return self.cancel_after_failure(error),
        };
        if let Err(error) = self.pool.add_span(Arc::clone(&span), &self.metrics) {
            return self.cancel_after_failure(error);
        }
        self.admission.finish_span(self.layout.slots_per_span)?;
        AtomicMetrics::add(&self.metrics.spans_created, 1)?;
        AtomicMetrics::add(
            &self.metrics.reserved_virtual_bytes,
            to_u64(self.layout.span_size, "reserved span bytes")?,
        )?;
        Ok(())
    }

    pub fn allocate_global(self: &Arc<Self>) -> Result<super::lease::GoInspiredStack, StackError> {
        let token = match self.pool.take_global(&self.metrics)? {
            Some(token) => token,
            None => {
                self.create_span()?;
                self.pool
                    .take_global(&self.metrics)?
                    .ok_or(StackError::MetricsOverflow)?
            }
        };
        self.activate(token, super::span::global_free_state())
    }

    pub fn activate(
        self: &Arc<Self>,
        token: SlotToken,
        prior_state: u8,
    ) -> Result<super::lease::GoInspiredStack, StackError> {
        let start = token.span.slot_start(token.index)?;
        let newly_enabled = match token.span.activate(&token, prior_state) {
            Ok(newly_enabled) => newly_enabled,
            Err(error) => {
                AtomicMetrics::add(&self.metrics.quarantined_slots, 1)?;
                return Err(error);
            }
        };
        AtomicMetrics::add(&self.metrics.allocations, 1)?;
        AtomicMetrics::add(&self.metrics.active_stacks, 1)?;
        if !newly_enabled {
            AtomicMetrics::add(&self.metrics.reused_slots, 1)?;
        } else {
            AtomicMetrics::add(
                &self.metrics.accessible_virtual_bytes,
                to_u64(self.layout.usable_size, "accessible stack bytes")?,
            )?;
        }
        Ok(super::lease::GoInspiredStack::new(
            Arc::clone(self),
            token,
            start,
        ))
    }

    pub fn release(&self, token: &SlotToken, prior_state: u8) -> Result<(), StackError> {
        AtomicMetrics::subtract(&self.metrics.active_stacks, 1)?;
        if let Err(original) = AtomicMetrics::add(&self.metrics.releases, 1) {
            return Err(rollback_release(self, original, false));
        }
        let mut slots = vec![token.clone()];
        if let Err(original) = self
            .pool
            .return_global(&mut slots, prior_state, &self.metrics)
        {
            return Err(rollback_release(self, original, true));
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Result<VmaSnapshot, StackError> {
        self.admission.snapshot()
    }

    fn cancel_after_failure<T>(&self, original: StackError) -> Result<T, StackError> {
        match self.admission.cancel_span(self.layout.slots_per_span) {
            Ok(()) => Err(original),
            Err(cleanup) => Err(StackError::CleanupAfterFailure {
                original: Box::new(original),
                cleanup: Box::new(cleanup),
            }),
        }
    }
}

fn rollback_release(inner: &Inner, original: StackError, releases_added: bool) -> StackError {
    let mut cleanup = None;
    if releases_added {
        if let Err(error) = AtomicMetrics::subtract(&inner.metrics.releases, 1) {
            cleanup = Some(error);
        }
    }
    if let Err(error) = AtomicMetrics::add(&inner.metrics.active_stacks, 1) {
        cleanup = combine_failure(cleanup, error);
    }
    match cleanup {
        Some(cleanup) => StackError::CleanupAfterFailure {
            original: Box::new(original),
            cleanup: Box::new(cleanup),
        },
        None => original,
    }
}

fn combine_failure(previous: Option<StackError>, current: StackError) -> Option<StackError> {
    match previous {
        Some(previous) => Some(StackError::CleanupAfterFailure {
            original: Box::new(previous),
            cleanup: Box::new(current),
        }),
        None => Some(current),
    }
}

fn to_u64(value: usize, operation: &'static str) -> Result<u64, StackError> {
    u64::try_from(value).map_err(|source| StackError::IntegerConversion { operation, source })
}
