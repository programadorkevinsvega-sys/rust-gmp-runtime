use super::error::StackError;
use super::vma::VmaSnapshot;

const REFRESH_EVERY_OPERATIONS: u32 = 262_144;
pub(super) const MAX_VMA_COST_PER_STACK: u64 = 2;

#[path = "admission/refresh.rs"]
mod refresh;

#[derive(Debug, Default)]
pub(super) struct VmaAdmission {
    snapshot: Option<VmaSnapshot>,
    observed_allocation_vma_total: u64,
    allocation_vma_total: u64,
    pending_allocation_vma: u64,
    pending_release_vma: u64,
    operations_since_refresh: u32,
    refresh_in_progress: bool,
    snapshot_generation: u64,
    #[cfg(test)]
    refresh_waiters: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RefreshToken {
    allocation_vma_total: u64,
    operations_since_refresh: u32,
}

impl VmaAdmission {
    #[cfg(test)]
    pub(super) fn observe(
        &mut self,
        snapshot: VmaSnapshot,
        allocation_vma_total: u64,
    ) -> Result<(), StackError> {
        let next_generation = match self.snapshot_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                self.refresh_in_progress = false;
                return Err(StackError::MetricsOverflow);
            }
        };
        self.observed_allocation_vma_total = allocation_vma_total;
        self.allocation_vma_total = allocation_vma_total;
        self.snapshot = Some(snapshot);
        self.operations_since_refresh = 0;
        self.refresh_in_progress = false;
        self.snapshot_generation = next_generation;
        Ok(())
    }

    pub(super) fn snapshot(&self) -> Option<VmaSnapshot> {
        self.snapshot
    }

    pub(super) fn snapshot_generation(&self) -> u64 {
        self.snapshot_generation
    }

    pub(super) fn refresh_due(&self) -> bool {
        self.snapshot.is_none() || self.operations_since_refresh >= REFRESH_EVERY_OPERATIONS
    }

    #[cfg(test)]
    pub(super) fn expire_snapshot(&mut self) {
        self.operations_since_refresh = REFRESH_EVERY_OPERATIONS;
    }

    pub(super) fn is_refreshing(&self) -> bool {
        self.refresh_in_progress
    }

    pub(super) fn check_cached(
        &mut self,
        additional_stacks: usize,
        safety_margin: u64,
    ) -> Result<VmaSnapshot, StackError> {
        let snapshot = match self.snapshot {
            Some(snapshot) => snapshot,
            None => return Err(StackError::MalformedVmaLimit),
        };
        let cost = u64::try_from(additional_stacks)
            .ok()
            .and_then(|count| count.checked_mul(MAX_VMA_COST_PER_STACK));
        let allocations_during_refresh = self
            .allocation_vma_total
            .checked_sub(self.observed_allocation_vma_total)
            .ok_or(StackError::MetricsOverflow)?;
        let current = snapshot
            .current
            .checked_add(allocations_during_refresh)
            .and_then(|current| current.checked_add(self.pending_allocation_vma))
            .and_then(|current| current.checked_add(self.pending_release_vma))
            .unwrap_or(u64::MAX)
            .max(snapshot.current);
        let capacity = cost
            .and_then(|required| current.checked_add(required))
            .and_then(|needed| needed.checked_add(safety_margin));
        self.operations_since_refresh = self.operations_since_refresh.saturating_add(1);
        if capacity.is_none_or(|needed| needed > snapshot.limit) {
            let required = cost.unwrap_or(u64::MAX);
            return Err(StackError::VmaPreflightRejected {
                limit: snapshot.limit,
                current,
                required,
                safety_margin,
            });
        }
        Ok(snapshot)
    }

    pub(super) fn note_operation(&mut self) {
        self.operations_since_refresh = self.operations_since_refresh.saturating_add(1);
    }

    pub(super) fn begin_allocation(&mut self) -> Result<(), StackError> {
        let pending = self
            .pending_allocation_vma
            .checked_add(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        self.allocation_vma_total
            .checked_add(pending)
            .ok_or(StackError::MetricsOverflow)?;
        self.pending_allocation_vma = pending;
        Ok(())
    }

    pub(super) fn note_allocation(&mut self) -> Result<(), StackError> {
        self.allocation_vma_total = self
            .allocation_vma_total
            .checked_add(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        Ok(())
    }

    pub(super) fn note_release(&mut self) -> Result<(), StackError> {
        self.note_operation();
        Ok(())
    }

    pub(super) fn finish_allocation(&mut self) -> Result<(), StackError> {
        let allocation_vma_total = self
            .allocation_vma_total
            .checked_add(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        let pending_allocation_vma = self
            .pending_allocation_vma
            .checked_sub(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        self.allocation_vma_total = allocation_vma_total;
        self.pending_allocation_vma = pending_allocation_vma;
        self.note_operation();
        Ok(())
    }

    pub(super) fn cancel_allocation(&mut self) -> Result<(), StackError> {
        self.pending_allocation_vma = self
            .pending_allocation_vma
            .checked_sub(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        self.note_operation();
        Ok(())
    }

    pub(super) fn begin_release(&mut self) -> Result<(), StackError> {
        self.pending_release_vma = self
            .pending_release_vma
            .checked_add(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        Ok(())
    }

    pub(super) fn finish_release(&mut self) -> Result<(), StackError> {
        self.pending_release_vma = self
            .pending_release_vma
            .checked_sub(MAX_VMA_COST_PER_STACK)
            .ok_or(StackError::MetricsOverflow)?;
        self.note_operation();
        Ok(())
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
