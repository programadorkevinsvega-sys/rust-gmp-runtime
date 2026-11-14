//! Uniquely assigned routine identifiers.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Process-unique, nonzero routine identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RoutineId(NonZeroU64);

impl RoutineId {
    /// Allocates a unique identifier or reports exhaustion.
    ///
    /// # Errors
    ///
    /// Returns [`RoutineIdError::Exhausted`] if advancing the counter would wrap,
    /// or [`RoutineIdError::InvalidZero`] if the reserved zero value is observed.
    pub fn allocate() -> Result<Self, RoutineIdError> {
        allocate_from(&NEXT_ID)
    }

    /// Returns the nonzero numeric identifier.
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

fn allocate_from(counter: &AtomicU64) -> Result<RoutineId, RoutineIdError> {
    let value = match counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        current.checked_add(1)
    }) {
        Ok(value) => value,
        Err(counter_value) => return Err(RoutineIdError::Exhausted { counter_value }),
    };
    match NonZeroU64::new(value) {
        Some(value) => Ok(RoutineId(value)),
        None => Err(RoutineIdError::InvalidZero),
    }
}

/// Failure to issue another unique routine identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutineIdError {
    /// The process-wide identifier counter reached its maximum value.
    Exhausted {
        /// Counter value that could not be advanced without wrapping.
        counter_value: u64,
    },
    /// The identifier counter contained the reserved zero value.
    InvalidZero,
}

impl fmt::Display for RoutineIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted { counter_value } => write!(
                formatter,
                "routine identifier counter cannot advance from {counter_value}"
            ),
            Self::InvalidZero => {
                formatter.write_str("routine identifier counter contained reserved value zero")
            }
        }
    }
}

impl Error for RoutineIdError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU64;
    use std::thread;

    use super::{RoutineId, RoutineIdError, allocate_from};

    #[test]
    fn identifiers_are_nonzero_and_exhaustion_does_not_wrap() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(
            allocate_from(&counter),
            Err(RoutineIdError::Exhausted {
                counter_value: u64::MAX
            })
        );
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), u64::MAX);
    }

    #[test]
    fn concurrent_allocations_are_unique() -> Result<(), Box<dyn std::error::Error>> {
        let workers = 8;
        let allocations_per_worker = 128;
        let mut handles = Vec::with_capacity(workers);

        for _ in 0..workers {
            handles.push(thread::spawn(move || {
                (0..allocations_per_worker)
                    .map(|_| RoutineId::allocate())
                    .collect::<Result<Vec<_>, _>>()
            }));
        }

        let mut identifiers = Vec::with_capacity(workers * allocations_per_worker);
        for handle in handles {
            let values = match handle.join() {
                Ok(Ok(values)) => values,
                Ok(Err(error)) => return Err(Box::new(error)),
                Err(payload) => std::panic::resume_unwind(payload),
            };
            identifiers.extend(values.into_iter().map(RoutineId::get));
        }

        identifiers.sort_unstable();
        identifiers.dedup();
        assert_eq!(identifiers.len(), workers * allocations_per_worker);
        assert!(identifiers.iter().all(|identifier| *identifier != 0));
        Ok(())
    }
}
