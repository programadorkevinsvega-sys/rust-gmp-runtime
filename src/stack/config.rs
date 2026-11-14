use super::error::StackError;

/// Default VMA headroom retained by the stack allocator.
pub const DEFAULT_VMA_SAFETY_MARGIN: u64 = 1_024;

/// Validated allocation configuration for a stack manager.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackConfig {
    requested_usable_size: usize,
    vma_preflight: Option<VmaPreflightConfig>,
}

/// Controls preventive admission against the Linux process VMA budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmaPreflightConfig {
    /// Number of VMA slots to leave unused after the estimated reservation.
    pub safety_margin: u64,
}

impl Default for VmaPreflightConfig {
    fn default() -> Self {
        Self {
            safety_margin: DEFAULT_VMA_SAFETY_MARGIN,
        }
    }
}

impl StackConfig {
    /// Creates a configuration for a nonzero usable stack size in bytes.
    ///
    /// VMA preflight is enabled by default with [`DEFAULT_VMA_SAFETY_MARGIN`].
    ///
    /// # Errors
    ///
    /// Returns [`StackError::InvalidSize`] if the requested size is zero.
    pub fn new(usable_size_bytes: usize) -> Result<Self, StackError> {
        if usable_size_bytes == 0 {
            return Err(StackError::InvalidSize {
                requested_bytes: usable_size_bytes,
            });
        }
        Ok(Self {
            requested_usable_size: usable_size_bytes,
            vma_preflight: Some(VmaPreflightConfig::default()),
        })
    }

    /// Returns the requested usable size before page alignment.
    pub fn requested_usable_size(self) -> usize {
        self.requested_usable_size
    }

    /// Sets the number of VMA slots that preflight leaves unused.
    pub fn with_vma_safety_margin(mut self, safety_margin: u64) -> Self {
        self.vma_preflight = Some(VmaPreflightConfig { safety_margin });
        self
    }

    /// Disables preventive VMA admission; native reservation errors still propagate.
    pub fn without_vma_preflight(mut self) -> Self {
        self.vma_preflight = None;
        self
    }

    pub(super) fn vma_preflight(self) -> Option<VmaPreflightConfig> {
        self.vma_preflight
    }
}
