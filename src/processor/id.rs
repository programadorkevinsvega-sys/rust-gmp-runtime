/// Stable index of a Processor within its owning pool.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProcessorId(usize);

impl ProcessorId {
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns this Processor's zero-based index in its pool.
    pub const fn index(self) -> usize {
        self.0
    }
}
