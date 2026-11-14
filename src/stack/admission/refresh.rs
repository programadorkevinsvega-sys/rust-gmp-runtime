use super::super::error::StackError;
use super::super::vma::VmaSnapshot;
use super::{RefreshToken, VmaAdmission};

impl VmaAdmission {
    #[cfg(test)]
    pub(in crate::stack) fn refresh_waiters(&self) -> usize {
        self.refresh_waiters
    }

    #[cfg(test)]
    pub(in crate::stack) fn add_refresh_waiter(&mut self) -> Result<(), StackError> {
        self.refresh_waiters = self
            .refresh_waiters
            .checked_add(1)
            .ok_or(StackError::MetricsOverflow)?;
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::stack) fn remove_refresh_waiter(&mut self) -> Result<(), StackError> {
        self.refresh_waiters = self
            .refresh_waiters
            .checked_sub(1)
            .ok_or(StackError::MetricsOverflow)?;
        Ok(())
    }

    pub(in crate::stack) fn begin_refresh(&mut self) -> Option<RefreshToken> {
        if self.refresh_in_progress {
            return None;
        }
        self.refresh_in_progress = true;
        Some(RefreshToken {
            allocation_vma_total: self.allocation_vma_total,
            operations_since_refresh: self.operations_since_refresh,
        })
    }

    pub(in crate::stack) fn finish_refresh(
        &mut self,
        snapshot: Result<VmaSnapshot, StackError>,
        token: RefreshToken,
    ) -> Result<(), StackError> {
        let snapshot = match snapshot {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.refresh_in_progress = false;
                return Err(error);
            }
        };
        let next_generation = match self.snapshot_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                self.refresh_in_progress = false;
                return Err(StackError::MetricsOverflow);
            }
        };
        let operations_since_refresh = match self
            .operations_since_refresh
            .checked_sub(token.operations_since_refresh)
        {
            Some(operations) => operations,
            None => {
                self.refresh_in_progress = false;
                return Err(StackError::MetricsOverflow);
            }
        };
        self.refresh_in_progress = false;
        self.snapshot = Some(snapshot);
        self.observed_allocation_vma_total = token.allocation_vma_total;
        self.operations_since_refresh = operations_since_refresh;
        self.snapshot_generation = next_generation;
        Ok(())
    }
}
