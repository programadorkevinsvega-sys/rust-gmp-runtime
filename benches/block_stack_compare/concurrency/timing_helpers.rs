use super::*;

#[derive(Debug, Default)]
pub(super) struct RefillTimingTotals {
    pub(super) lock_wait_ns: u64,
    pub(super) lock_hold_ns: u64,
    pub(super) mmap_ns: u64,
    pub(super) mprotect_ns: u64,
    pub(super) munmap_ns: u64,
    pub(super) mmap_calls: u64,
    pub(super) mprotect_calls: u64,
    pub(super) munmap_calls: u64,
}

impl RefillTimingTotals {
    pub(super) fn add(
        &mut self,
        snapshot: rust_gmp_runtime::stack::GoInspiredTimingSnapshot,
    ) -> Result<(), Box<dyn Error>> {
        self.lock_wait_ns = add_timing(self.lock_wait_ns, snapshot.lock_wait_ns)?;
        self.lock_hold_ns = add_timing(self.lock_hold_ns, snapshot.lock_hold_ns)?;
        self.mmap_ns = add_timing(self.mmap_ns, snapshot.mmap_ns)?;
        self.mprotect_ns = add_timing(self.mprotect_ns, snapshot.mprotect_ns)?;
        self.munmap_ns = add_timing(self.munmap_ns, snapshot.munmap_ns)?;
        self.mmap_calls = add_timing(self.mmap_calls, snapshot.mmap_calls)?;
        self.mprotect_calls = add_timing(self.mprotect_calls, snapshot.mprotect_calls)?;
        self.munmap_calls = add_timing(self.munmap_calls, snapshot.munmap_calls)?;
        Ok(())
    }
}
pub(super) fn add_timing(total: u64, sample: u64) -> Result<u64, Box<dyn Error>> {
    total
        .checked_add(sample)
        .ok_or_else(|| "aggregated benchmark timing overflow".into())
}
pub(super) fn counter_delta(
    after: Option<u64>,
    before: Option<u64>,
    name: &'static str,
) -> Result<u64, Box<dyn Error>> {
    match (after, before) {
        (Some(after), Some(before)) => delta(after, before, name),
        (None, None) => Ok(0),
        _ => Err(format!("{name} availability changed during benchmark").into()),
    }
}
pub(super) fn delta(after: u64, before: u64, name: &'static str) -> Result<u64, Box<dyn Error>> {
    after
        .checked_sub(before)
        .ok_or_else(|| format!("{name} counter decreased during benchmark").into())
}
