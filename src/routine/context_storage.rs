use std::num::NonZeroUsize;

use crate::context::machine::MachineContext;
use crate::stack::StackBounds;

use super::error::RoutineError;

/// Read-only observation of the context storage owned by a routine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoutineContextInfo {
    /// Exclusive upper boundary used to prepare the initial stack pointer.
    pub stack_upper_bound: usize,
    /// Stack pointer saved by the most recent return to the scheduler.
    pub saved_stack_pointer: Option<usize>,
}

#[derive(Debug)]
pub(crate) struct RoutineContext {
    stack_upper_bound: NonZeroUsize,
    pub(crate) saved_stack_pointer: Option<NonZeroUsize>,
    pub(crate) machine: MachineContext,
    pub(crate) started: bool,
}

impl RoutineContext {
    pub(super) fn new(bounds: StackBounds) -> Result<Self, RoutineError> {
        let stack_upper_bound = match NonZeroUsize::new(bounds.usable_end) {
            Some(pointer) => pointer,
            None => {
                return Err(RoutineError::InvalidContextStackBound {
                    value: bounds.usable_end,
                });
            }
        };
        Ok(Self {
            stack_upper_bound,
            saved_stack_pointer: None,
            machine: MachineContext::empty(),
            started: false,
        })
    }

    pub(super) fn info(&self) -> RoutineContextInfo {
        RoutineContextInfo {
            stack_upper_bound: self.stack_upper_bound.get(),
            saved_stack_pointer: self.saved_stack_pointer.map(NonZeroUsize::get),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RoutineContext;
    use crate::routine::RoutineError;
    use crate::stack::StackBounds;

    #[test]
    fn rejects_a_zero_context_stack_bound() {
        let bounds = StackBounds {
            guard_start: 0,
            guard_end: 0,
            usable_start: 0,
            usable_end: 0,
        };
        assert!(matches!(
            RoutineContext::new(bounds),
            Err(RoutineError::InvalidContextStackBound { value: 0 })
        ));
    }
}
