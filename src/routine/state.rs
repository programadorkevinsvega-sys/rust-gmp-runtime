/// Lifecycle state of one stackful execution routine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutineState {
    /// Allocated but not yet made runnable.
    Created,
    /// Published and eligible to be claimed by one worker.
    Runnable,
    /// Exclusively owned by one execution permit.
    Running,
    /// Suspended on an operation managed by the runtime.
    Waiting,
    /// Inside an explicitly managed blocking system operation.
    Syscall,
    /// Terminal; it cannot be scheduled again.
    Dead,
}

impl RoutineState {
    pub(crate) fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Runnable)
                | (Self::Created, Self::Dead)
                | (Self::Runnable, Self::Running)
                | (Self::Runnable, Self::Dead)
                | (Self::Running, Self::Runnable)
                | (Self::Running, Self::Waiting)
                | (Self::Running, Self::Syscall)
                | (Self::Running, Self::Dead)
                | (Self::Waiting, Self::Runnable)
                | (Self::Waiting, Self::Dead)
                | (Self::Syscall, Self::Runnable)
                | (Self::Syscall, Self::Dead)
        )
    }
}

/// Terminal disposition recorded when a routine leaves execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutineCompletion {
    /// The execution permit reported normal completion.
    Completed,
    /// The routine panicked and its payload was returned to the caller.
    Panicked,
    /// The execution permit was dropped without reporting an outcome.
    Abandoned,
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::super::RoutineError;
    use super::super::lifecycle::RoutineLifecycle;
    use super::RoutineState;

    #[test]
    fn seeded_lifecycle_sequences_match_the_reference_model() -> Result<(), Box<dyn Error>> {
        let states = [
            RoutineState::Created,
            RoutineState::Runnable,
            RoutineState::Running,
            RoutineState::Waiting,
            RoutineState::Syscall,
            RoutineState::Dead,
        ];
        let mut seed = 0x7d31_4a29_u32;
        let mut state = RoutineState::Created;
        let mut lifecycle = RoutineLifecycle::new();

        for _ in 0..512 {
            seed ^= seed.wrapping_shl(13);
            seed ^= seed.wrapping_shr(17);
            seed ^= seed.wrapping_shl(5);
            let requested = states[(seed as usize) % states.len()];
            let model_accepts = reference_transition(state, requested);

            let actual_accepts = match lifecycle.transition(requested, None, None) {
                Ok(()) => true,
                Err(RoutineError::InvalidTransition { .. }) => false,
                Err(error) => return Err(Box::new(error)),
            };
            assert_eq!(state.can_transition_to(requested), model_accepts);
            assert_eq!(actual_accepts, model_accepts);
            if model_accepts {
                state = requested;
            }
            assert_eq!(lifecycle.state(), state);
        }
        Ok(())
    }

    fn reference_transition(current: RoutineState, next: RoutineState) -> bool {
        match current {
            RoutineState::Created => matches!(next, RoutineState::Runnable | RoutineState::Dead),
            RoutineState::Runnable => matches!(next, RoutineState::Running | RoutineState::Dead),
            RoutineState::Running => matches!(
                next,
                RoutineState::Runnable
                    | RoutineState::Waiting
                    | RoutineState::Syscall
                    | RoutineState::Dead
            ),
            RoutineState::Waiting | RoutineState::Syscall => {
                matches!(next, RoutineState::Runnable | RoutineState::Dead)
            }
            RoutineState::Dead => false,
        }
    }
}
