use std::fmt;

use super::go_inspired::GoInspiredStack;
use super::go_inspired::ProcessorStackCache;
use super::mapping::FixedStack;
use super::mapping::StackBounds;

enum StackLease {
    Fixed(FixedStack),
    GoInspired(GoInspiredStack),
}

/// RAII owner of a stable-address stack selected by the stack manager.
pub struct Stack {
    lease: StackLease,
}

impl Stack {
    pub(super) fn fixed(stack: FixedStack) -> Self {
        Self {
            lease: StackLease::Fixed(stack),
        }
    }

    pub(crate) fn go_inspired(stack: GoInspiredStack) -> Self {
        Self {
            lease: StackLease::GoInspired(stack),
        }
    }

    /// Returns stack and guard bounds as process virtual addresses.
    pub fn bounds(&self) -> StackBounds {
        match &self.lease {
            StackLease::Fixed(stack) => stack.bounds(),
            StackLease::GoInspired(stack) => stack.bounds(),
        }
    }

    /// Returns the page-aligned usable stack capacity in bytes.
    pub fn usable_size(&self) -> usize {
        match &self.lease {
            StackLease::Fixed(stack) => stack.usable_size(),
            StackLease::GoInspired(stack) => stack.usable_size(),
        }
    }

    /// Returns total reserved address space, including the guard page.
    pub fn reserved_size(&self) -> usize {
        match &self.lease {
            StackLease::Fixed(stack) => stack.reserved_size(),
            StackLease::GoInspired(stack) => stack.reserved_size(),
        }
    }

    pub(crate) fn release_to_processor_cache(
        &mut self,
        cache: &mut ProcessorStackCache,
    ) -> Result<(), super::error::StackError> {
        match &mut self.lease {
            StackLease::Fixed(_) => Err(super::error::StackError::StackEngineMismatch {
                expected: super::config::StackEngine::GoInspired,
                selected: super::config::StackEngine::Fixed,
            }),
            StackLease::GoInspired(stack) => stack.release_to_cache(cache),
        }
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let engine = match &self.lease {
            StackLease::Fixed(_) => "Fixed",
            StackLease::GoInspired(_) => "GoInspired",
        };
        formatter
            .debug_struct("Stack")
            .field("engine", &engine)
            .field("bounds", &self.bounds())
            .finish()
    }
}
