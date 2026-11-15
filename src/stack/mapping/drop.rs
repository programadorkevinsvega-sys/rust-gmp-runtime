#[cfg(feature = "allocator-benchmark")]
use super::super::benchmark_metrics::TimingKind;
use super::super::protection;
use super::FixedStack;

impl Drop for FixedStack {
    fn drop(&mut self) {
        if self.two_phase_syscalls {
            self.drop_two_phase();
            return;
        }
        #[cfg(feature = "allocator-benchmark")]
        let wait_started = self
            .benchmark_metrics
            .as_ref()
            .map(|_| std::time::Instant::now());
        let mut admission = match self.manager.admission.lock() {
            Ok(admission) => admission,
            Err(error) => {
                eprintln!("fatal: stack allocation admission lock was poisoned: {error}");
                std::process::abort();
            }
        };
        #[cfg(feature = "allocator-benchmark")]
        if let (Some(metrics), Some(started)) = (&self.benchmark_metrics, wait_started) {
            metrics.record_lock_wait(started);
        }
        #[cfg(feature = "allocator-benchmark")]
        let hold_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        #[cfg(feature = "allocator-benchmark")]
        let munmap_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Munmap));
        let release = protection::dispatch::release(self.base, self.reservation_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(munmap_timer);
        if let Err(error) = release {
            self.abort_after_cleanup_failure(error);
        }
        if let Err(error) = self
            .manager
            .record_release(self.reservation_size, self.usable_size)
        {
            eprintln!("fatal: failed to update stack release metrics: {error}");
            std::process::abort();
        }
        if let Err(error) = admission.note_release() {
            eprintln!("fatal: failed to update VMA admission estimate on stack release: {error}");
            std::process::abort();
        }
        drop(admission);
        #[cfg(feature = "allocator-benchmark")]
        drop(hold_timer);
    }
}

impl FixedStack {
    fn drop_two_phase(&mut self) {
        #[cfg(feature = "allocator-benchmark")]
        let wait_started = self
            .benchmark_metrics
            .as_ref()
            .map(|_| std::time::Instant::now());
        let mut admission = match self.manager.admission.lock() {
            Ok(admission) => admission,
            Err(error) => {
                eprintln!("fatal: stack allocation admission lock was poisoned: {error}");
                std::process::abort();
            }
        };
        #[cfg(feature = "allocator-benchmark")]
        if let (Some(metrics), Some(started)) = (&self.benchmark_metrics, wait_started) {
            metrics.record_lock_wait(started);
        }
        #[cfg(feature = "allocator-benchmark")]
        let hold_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        if let Err(error) = admission.begin_release() {
            eprintln!("fatal: failed to mark stack mapping pending release: {error}");
            std::process::abort();
        }
        drop(admission);
        #[cfg(feature = "allocator-benchmark")]
        drop(hold_timer);

        #[cfg(feature = "allocator-benchmark")]
        let munmap_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::Munmap));
        let release = protection::dispatch::release(self.base, self.reservation_size);
        #[cfg(feature = "allocator-benchmark")]
        drop(munmap_timer);
        if let Err(error) = release {
            self.abort_after_cleanup_failure(error);
        }

        let mut admission = match self.manager.admission.lock() {
            Ok(admission) => admission,
            Err(error) => {
                eprintln!("fatal: stack allocation admission lock was poisoned: {error}");
                std::process::abort();
            }
        };
        #[cfg(feature = "allocator-benchmark")]
        let hold_timer = self
            .benchmark_metrics
            .as_ref()
            .map(|metrics| metrics.time(TimingKind::LockHold));
        if let Err(error) = admission.finish_release() {
            eprintln!("fatal: failed to confirm stack mapping release: {error}");
            std::process::abort();
        }
        drop(admission);
        #[cfg(feature = "allocator-benchmark")]
        drop(hold_timer);
        if let Err(error) = self
            .manager
            .record_release(self.reservation_size, self.usable_size)
        {
            eprintln!("fatal: failed to update stack release metrics: {error}");
            std::process::abort();
        }
    }

    fn abort_after_cleanup_failure(&self, error: crate::stack::StackError) -> ! {
        if let Err(metrics_error) = self.manager.record_cleanup_failure() {
            eprintln!("fatal: failed to record stack cleanup failure: {metrics_error}");
        }
        eprintln!("fatal: failed to release routine stack mapping: {error}");
        std::process::abort();
    }
}
