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
    stack_bounds: StackBounds,
    pub(crate) saved_stack_pointer: Option<NonZeroUsize>,
    pub(crate) machine: MachineContext,
    pub(crate) started: bool,
}

impl RoutineContext {
    pub(super) fn new(bounds: StackBounds) -> Result<Self, RoutineError> {
        if NonZeroUsize::new(bounds.usable_end).is_none() {
            return Err(RoutineError::InvalidContextStackBound {
                value: bounds.usable_end,
            });
        }
        Ok(Self {
            stack_bounds: bounds,
            saved_stack_pointer: None,
            machine: MachineContext::empty(),
            started: false,
        })
    }

    pub(super) fn info(&self) -> RoutineContextInfo {
        RoutineContextInfo {
            stack_upper_bound: self.stack_bounds.usable_end,
            saved_stack_pointer: self.saved_stack_pointer.map(NonZeroUsize::get),
        }
    }

    pub(crate) fn stack_bounds(&self) -> StackBounds {
        self.stack_bounds
    }

    pub(super) fn invalidate_stack(&mut self) {
        self.stack_bounds = StackBounds {
            guard_start: 0,
            guard_end: 0,
            usable_start: 0,
            usable_end: 0,
        };
        self.saved_stack_pointer = None;
        self.machine = MachineContext::empty();
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
