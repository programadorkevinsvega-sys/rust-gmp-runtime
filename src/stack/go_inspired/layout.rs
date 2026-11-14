use crate::stack::{StackConfig, StackError};

const REFERENCE_CACHE_BYTES: usize = 32 * 1024;
const DEFAULT_SLOTS_PER_SPAN: usize = 64;

/// Experimental span and local-cache sizing, expressed in bytes and slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GoInspiredOptions {
    cache_bytes_per_class: usize,
    slots_per_span: usize,
    retained_spans: usize,
}

impl Default for GoInspiredOptions {
    fn default() -> Self {
        Self {
            cache_bytes_per_class: REFERENCE_CACHE_BYTES,
            slots_per_span: DEFAULT_SLOTS_PER_SPAN,
            retained_spans: 1,
        }
    }
}

impl GoInspiredOptions {
    /// Sets the target cache capacity; slot counts are rounded down by stride.
    pub fn with_cache_bytes(mut self, bytes: usize) -> Self {
        self.cache_bytes_per_class = bytes;
        self
    }

    /// Sets the number of slots reserved in each new span.
    pub fn with_slots_per_span(mut self, slots: usize) -> Self {
        self.slots_per_span = slots;
        self
    }

    /// Sets the warm-span target retained by `trim`; defaults to one span.
    pub fn with_retained_spans(mut self, spans: usize) -> Self {
        self.retained_spans = spans;
        self
    }

    pub(crate) fn validate(
        self,
        config: StackConfig,
        page_size: usize,
    ) -> Result<ValidatedLayout, StackError> {
        if self.cache_bytes_per_class == 0 || self.slots_per_span == 0 {
            return Err(StackError::InvalidSize { requested_bytes: 0 });
        }
        let usable_size =
            super::super::size::round_up_to_page(config.requested_usable_size(), page_size)?;
        let stride = page_size
            .checked_add(usable_size)
            .ok_or(StackError::SizeOverflow)?;
        let span_size = stride
            .checked_mul(self.slots_per_span)
            .ok_or(StackError::SizeOverflow)?;
        let cache_capacity = (self.cache_bytes_per_class / stride).max(1);
        let cache_target = (cache_capacity / 2).max(1).min(cache_capacity);
        let _span_vma_cost = u64::try_from(self.slots_per_span)
            .ok()
            .and_then(|slots| slots.checked_mul(2))
            .ok_or(StackError::SizeOverflow)?;
        Ok(ValidatedLayout {
            page_size,
            usable_size,
            stride,
            span_size,
            slots_per_span: self.slots_per_span,
            cache_capacity,
            cache_target,
            retained_spans: self.retained_spans,
            safety_margin: config
                .vma_preflight()
                .map(|settings| settings.safety_margin),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidatedLayout {
    pub page_size: usize,
    pub usable_size: usize,
    pub stride: usize,
    pub span_size: usize,
    pub slots_per_span: usize,
    pub cache_capacity: usize,
    pub cache_target: usize,
    pub retained_spans: usize,
    pub safety_margin: Option<u64>,
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{GoInspiredOptions, REFERENCE_CACHE_BYTES};
    use crate::stack::StackConfig;

    #[test]
    fn capacity_rounds_to_whole_slots_for_non_power_of_two_stride() -> Result<(), Box<dyn Error>> {
        let layout = match GoInspiredOptions::default()
            .with_slots_per_span(3)
            .validate(StackConfig::new(5000)?, 4096)
        {
            Ok(layout) => layout,
            Err(error) => return Err(Box::new(error)),
        };
        assert_eq!(layout.stride, 12_288);
        assert_eq!(layout.cache_capacity, REFERENCE_CACHE_BYTES / 12_288);
        assert_eq!(layout.cache_target, 1);
        Ok(())
    }

    #[test]
    fn cache_budget_controls_refill_batch_for_eight_kib_stacks() -> Result<(), Box<dyn Error>> {
        let expected = [(32, 1), (64, 2), (128, 5), (256, 10)];
        for (cache_kib, expected_batch) in expected {
            let layout = GoInspiredOptions::default()
                .with_cache_bytes(cache_kib * 1024)
                .with_slots_per_span(32)
                .validate(StackConfig::new(8 * 1024)?, 4096)?;
            assert_eq!(layout.stride, 12 * 1024);
            assert_eq!(layout.cache_target, expected_batch);
        }
        Ok(())
    }
}
