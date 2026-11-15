use crate::stack::{StackError, VmaSnapshot};

use super::Admission;

impl Admission {
    pub fn preflight_slots(&self, slots: usize) -> Result<VmaSnapshot, StackError> {
        let snapshot = self.snapshot()?;
        let Some(margin) = self.margin else {
            return Ok(snapshot);
        };
        let mut state = self.lock()?;
        state.check_cached(slots, margin)
    }
}
