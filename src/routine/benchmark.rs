use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Per-routine timing collected only with `runtime-benchmark-instrumentation`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RoutineBenchmarkTiming {
    /// Full `Routine::new` duration, including stack acquisition and context setup.
    pub creation_ns: u64,
    /// Time spent in the stack manager's allocation call.
    pub stack_acquisition_ns: u64,
    /// Full worker-side `run_once_with_permit` duration.
    pub run_once_with_permit_ns: u64,
    /// Time spent completing the execution permit after returning to scheduler stack.
    pub finalization_ns: u64,
}

#[derive(Debug, Default)]
pub(crate) struct RoutineBenchmark {
    creation_ns: AtomicU64,
    stack_acquisition_ns: AtomicU64,
    run_once_with_permit_ns: AtomicU64,
    finalization_ns: AtomicU64,
}

impl RoutineBenchmark {
    pub(crate) fn record_creation(&self, creation: Duration, stack_acquisition: Duration) {
        self.creation_ns
            .store(duration_ns(creation), Ordering::Relaxed);
        self.stack_acquisition_ns
            .store(duration_ns(stack_acquisition), Ordering::Relaxed);
    }

    pub(crate) fn record_run_once_with_permit(&self, elapsed: Duration) {
        self.run_once_with_permit_ns
            .store(duration_ns(elapsed), Ordering::Relaxed);
    }

    pub(crate) fn record_finalization(&self, elapsed: Duration) {
        self.finalization_ns
            .store(duration_ns(elapsed), Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> RoutineBenchmarkTiming {
        RoutineBenchmarkTiming {
            creation_ns: self.creation_ns.load(Ordering::Relaxed),
            stack_acquisition_ns: self.stack_acquisition_ns.load(Ordering::Relaxed),
            run_once_with_permit_ns: self.run_once_with_permit_ns.load(Ordering::Relaxed),
            finalization_ns: self.finalization_ns.load(Ordering::Relaxed),
        }
    }
}

fn duration_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}
