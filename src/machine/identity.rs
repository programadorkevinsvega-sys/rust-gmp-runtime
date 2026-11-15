/// Stable identifier assigned to one worker Machine by its manager.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineId(usize);

impl MachineId {
    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns this Machine's zero-based index in its manager.
    pub const fn index(self) -> usize {
        self.0
    }
}
