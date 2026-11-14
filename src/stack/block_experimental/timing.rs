use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Syscall and mutex timings collected by an instrumented benchmark manager.
pub struct BlockStackTimingSnapshot {
    /// Nanoseconds spent waiting to acquire allocator state mutexes.
    pub lock_wait_ns: u64,
    /// Nanoseconds spent waiting for an in-progress condition-variable event.
    pub condvar_wait_ns: u64,
    /// Number of condition-variable waits during allocation or release.
    pub condvar_waits: u64,
    /// Nanoseconds measured while allocator state was locked.
    pub lock_hold_ns: u64,
    /// Total time in arena `mmap` calls.
    pub mmap_ns: u64,
    /// Total time in slot `mprotect` calls.
    pub mprotect_ns: u64,
    /// Total time in arena `munmap` calls.
    pub munmap_ns: u64,
    /// Total time in `madvise(MADV_DONTNEED)` calls.
    pub madvise_ns: u64,
    /// Number of measured allocator-lock acquisitions.
    pub lock_acquisitions: u64,
    /// Number of timed arena `mmap` calls.
    pub mmap_calls: u64,
    /// Number of timed slot `mprotect` calls.
    pub mprotect_calls: u64,
    /// Number of timed arena `munmap` calls.
    pub munmap_calls: u64,
    /// Number of timed `madvise` calls.
    pub madvise_calls: u64,
}

/// Cloneable access to timing counters after the manager itself is dropped.
#[derive(Clone)]
pub struct BlockStackTimingHandle {
    timing: Arc<Timing>,
}

#[derive(Default)]
pub(super) struct Timing {
    lock_wait_ns: AtomicU64,
    condvar_wait_ns: AtomicU64,
    condvar_waits: AtomicU64,
    lock_hold_ns: AtomicU64,
    mmap_ns: AtomicU64,
    mprotect_ns: AtomicU64,
    munmap_ns: AtomicU64,
    madvise_ns: AtomicU64,
    lock_acquisitions: AtomicU64,
    mmap_calls: AtomicU64,
    mprotect_calls: AtomicU64,
    munmap_calls: AtomicU64,
    madvise_calls: AtomicU64,
    enabled: bool,
}

#[derive(Clone, Copy)]
pub(super) enum TimingKind {
    LockHold,
    Mmap,
    Mprotect,
    Munmap,
    Madvise,
}

pub(super) struct Timer<'a> {
    timing: &'a Timing,
    started: Option<Instant>,
    kind: TimingKind,
}

impl Timing {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    pub(super) fn started(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    pub(super) fn record_lock_wait(&self, started: Option<Instant>) {
        if let Some(started) = started {
            add(&self.lock_wait_ns, started.elapsed());
            self.lock_acquisitions.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn record_condvar_wait(&self, started: Option<Instant>) {
        if let Some(started) = started {
            add(&self.condvar_wait_ns, started.elapsed());
            self.condvar_waits.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn time(&self, kind: TimingKind) -> Timer<'_> {
        Timer {
            timing: self,
            started: self.started(),
            kind,
        }
    }

    pub(super) fn snapshot_and_reset(&self) -> BlockStackTimingSnapshot {
        BlockStackTimingSnapshot {
            lock_wait_ns: take(&self.lock_wait_ns),
            condvar_wait_ns: take(&self.condvar_wait_ns),
            condvar_waits: take(&self.condvar_waits),
            lock_hold_ns: take(&self.lock_hold_ns),
            mmap_ns: take(&self.mmap_ns),
            mprotect_ns: take(&self.mprotect_ns),
            munmap_ns: take(&self.munmap_ns),
            madvise_ns: take(&self.madvise_ns),
            lock_acquisitions: take(&self.lock_acquisitions),
            mmap_calls: take(&self.mmap_calls),
            mprotect_calls: take(&self.mprotect_calls),
            munmap_calls: take(&self.munmap_calls),
            madvise_calls: take(&self.madvise_calls),
        }
    }

    fn record(&self, kind: TimingKind, duration: Duration) {
        let (duration_counter, call_counter) = match kind {
            TimingKind::LockHold => (&self.lock_hold_ns, None),
            TimingKind::Mmap => (&self.mmap_ns, Some(&self.mmap_calls)),
            TimingKind::Mprotect => (&self.mprotect_ns, Some(&self.mprotect_calls)),
            TimingKind::Munmap => (&self.munmap_ns, Some(&self.munmap_calls)),
            TimingKind::Madvise => (&self.madvise_ns, Some(&self.madvise_calls)),
        };
        add(duration_counter, duration);
        if let Some(counter) = call_counter {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl BlockStackTimingHandle {
    pub(super) fn new(timing: Arc<Timing>) -> Self {
        Self { timing }
    }

    /// Returns and clears the timing totals and syscall counts.
    pub fn snapshot_and_reset(&self) -> BlockStackTimingSnapshot {
        self.timing.snapshot_and_reset()
    }
}

impl Drop for Timer<'_> {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            self.timing.record(self.kind, started.elapsed());
        }
    }
}

fn add(counter: &AtomicU64, duration: Duration) {
    let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
    counter.fetch_add(nanos, Ordering::Relaxed);
}

fn take(counter: &AtomicU64) -> u64 {
    counter.swap(0, Ordering::Relaxed)
}
