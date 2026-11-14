use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Debug, Default)]
/// Opt-in timing counters used only by allocator benchmarks.
pub struct StackBenchmarkMetrics {
    lock_wait_ns: AtomicU64,
    lock_hold_ns: AtomicU64,
    mmap_ns: AtomicU64,
    mprotect_ns: AtomicU64,
    munmap_ns: AtomicU64,
    lock_acquisitions: AtomicU64,
    mmap_calls: AtomicU64,
    mprotect_calls: AtomicU64,
    munmap_calls: AtomicU64,
    lock_hold_segments: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default)]
/// Timing totals and operation counts since the previous reset.
pub struct StackBenchmarkSnapshot {
    /// Nanoseconds waiting to acquire the admission mutex.
    pub lock_wait_ns: u64,
    /// Nanoseconds spent in measured admission critical sections.
    pub lock_hold_ns: u64,
    /// Total time spent in `mmap`.
    pub mmap_ns: u64,
    /// Total time spent in `mprotect`.
    pub mprotect_ns: u64,
    /// Total time spent in `munmap`.
    pub munmap_ns: u64,
    /// Measured admission-mutex acquisitions.
    pub lock_acquisitions: u64,
    /// Number of timed `mmap` calls.
    pub mmap_calls: u64,
    /// Number of timed `mprotect` calls.
    pub mprotect_calls: u64,
    /// Number of timed `munmap` calls.
    pub munmap_calls: u64,
    /// Number of measured lock-held code segments.
    pub lock_hold_segments: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum TimingKind {
    LockHold,
    Mmap,
    Mprotect,
    Munmap,
}

pub(crate) struct TimingGuard<'a> {
    metrics: &'a StackBenchmarkMetrics,
    started: Instant,
    kind: TimingKind,
}

impl StackBenchmarkMetrics {
    pub(crate) fn record_lock_wait(&self, started: Instant) {
        add(&self.lock_wait_ns, started.elapsed());
        self.lock_acquisitions.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn time(&self, kind: TimingKind) -> TimingGuard<'_> {
        TimingGuard {
            metrics: self,
            started: Instant::now(),
            kind,
        }
    }

    fn increment_call(&self, kind: TimingKind) {
        let counter = match kind {
            TimingKind::LockHold => &self.lock_hold_segments,
            TimingKind::Mmap => &self.mmap_calls,
            TimingKind::Mprotect => &self.mprotect_calls,
            TimingKind::Munmap => &self.munmap_calls,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns and clears the timing totals and syscall counts.
    pub fn snapshot_and_reset(&self) -> StackBenchmarkSnapshot {
        StackBenchmarkSnapshot {
            lock_wait_ns: self.lock_wait_ns.swap(0, Ordering::Relaxed),
            lock_hold_ns: self.lock_hold_ns.swap(0, Ordering::Relaxed),
            mmap_ns: self.mmap_ns.swap(0, Ordering::Relaxed),
            mprotect_ns: self.mprotect_ns.swap(0, Ordering::Relaxed),
            munmap_ns: self.munmap_ns.swap(0, Ordering::Relaxed),
            lock_acquisitions: self.lock_acquisitions.swap(0, Ordering::Relaxed),
            mmap_calls: self.mmap_calls.swap(0, Ordering::Relaxed),
            mprotect_calls: self.mprotect_calls.swap(0, Ordering::Relaxed),
            munmap_calls: self.munmap_calls.swap(0, Ordering::Relaxed),
            lock_hold_segments: self.lock_hold_segments.swap(0, Ordering::Relaxed),
        }
    }
}

impl Drop for TimingGuard<'_> {
    fn drop(&mut self) {
        self.metrics.increment_call(self.kind);
        let counter = match self.kind {
            TimingKind::LockHold => &self.metrics.lock_hold_ns,
            TimingKind::Mmap => &self.metrics.mmap_ns,
            TimingKind::Mprotect => &self.metrics.mprotect_ns,
            TimingKind::Munmap => &self.metrics.munmap_ns,
        };
        add(counter, self.started.elapsed());
    }
}

fn add(counter: &AtomicU64, duration: std::time::Duration) {
    counter.fetch_add(
        u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
}
