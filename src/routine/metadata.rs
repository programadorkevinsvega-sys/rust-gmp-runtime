//! Immutable metadata associated with each routine.

use std::time::Instant;

/// Creation metadata that does not change during a routine's lifetime.
#[derive(Clone, Debug)]
pub struct RoutineMetadata {
    name: Option<Box<str>>,
    created_at: Instant,
}

impl RoutineMetadata {
    /// Creates unnamed metadata with the current monotonic timestamp.
    pub fn new() -> Self {
        Self {
            name: None,
            created_at: Instant::now(),
        }
    }

    /// Sets an optional diagnostic name.
    pub fn with_name(mut self, name: impl Into<Box<str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Returns the optional diagnostic name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the time at which this metadata was created.
    pub fn created_at(&self) -> Instant {
        self.created_at
    }
}

impl Default for RoutineMetadata {
    fn default() -> Self {
        Self::new()
    }
}
