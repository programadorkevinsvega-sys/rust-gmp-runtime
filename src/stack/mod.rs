//! Stable, guard-protected stack reservations for runtime routines.
#![allow(unsafe_code)]

mod admission;
mod admission_refresh;
mod allocator;
#[cfg(feature = "allocator-benchmark")]
mod benchmark_metrics;
#[cfg(feature = "allocator-benchmark")]
mod block_experimental;
mod config;
mod diagnostics;
mod error;
mod go_inspired;
mod lease;
mod mapping;
mod metrics;
mod process_metrics;
mod protection;
mod size;
mod vma;

mod manager;
#[cfg(feature = "allocator-benchmark")]
pub use benchmark_metrics::{StackBenchmarkMetrics, StackBenchmarkSnapshot};
#[cfg(feature = "allocator-benchmark")]
pub use block_experimental::{
    BlockStack, BlockStackManager, BlockStackMetrics, BlockStackTimingHandle,
    BlockStackTimingSnapshot,
};
pub use config::StackEngine;
pub use config::{DEFAULT_VMA_SAFETY_MARGIN, StackConfig, VmaPreflightConfig};
pub use error::StackError;
#[cfg(feature = "allocator-benchmark")]
pub use go_inspired::GoInspiredTimingSnapshot;
pub use go_inspired::{
    GoInspiredOptions, GoInspiredStack, GoInspiredStackManager, GoInspiredStackMetrics,
    ProcessorStackCache,
};
pub use lease::Stack;
pub use manager::StackManager;
pub use mapping::StackBounds;
pub use metrics::StackMetrics;
pub use vma::VmaSnapshot;

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;
    use std::sync::Arc;
    use std::thread;

    use super::{StackConfig, StackManager};

    const GUARD_TEST_CHILD: &str = "RUST_GMP_STACK_GUARD_TEST_CHILD";

    fn manager(size: usize) -> Result<StackManager, Box<dyn Error>> {
        Ok(StackManager::new(StackConfig::new(size)?)?)
    }

    #[test]
    fn rounds_stack_size_to_page_and_tracks_reservation() -> Result<(), Box<dyn Error>> {
        let manager = manager(1)?;
        let stack = manager.allocate()?;
        let bounds = stack.bounds();

        assert_eq!(bounds.guard_end, bounds.usable_start);
        assert!(bounds.usable_start < bounds.usable_end);
        assert_eq!(bounds.guard_start % manager.page_size(), 0);
        assert_eq!(bounds.usable_start % manager.page_size(), 0);
        assert_eq!(bounds.usable_end % manager.page_size(), 0);
        assert_eq!(stack.usable_size(), manager.page_size());

        let metrics = manager.metrics()?;
        assert_eq!(metrics.active_stacks, 1);
        assert_eq!(metrics.active_mappings, 1);
        assert_eq!(
            metrics.reserved_virtual_bytes,
            (manager.page_size() * 2) as u64
        );
        assert_eq!(metrics.accessible_virtual_bytes, manager.page_size() as u64);
        assert_eq!(metrics.successful_allocations, 1);
        assert!(metrics.process_vma_count.is_some());

        drop(stack);
        let metrics = manager.metrics()?;
        assert_eq!(metrics.active_stacks, 0);
        assert_eq!(metrics.active_mappings, 0);
        assert_eq!(metrics.reserved_virtual_bytes, 0);
        assert_eq!(metrics.accessible_virtual_bytes, 0);
        assert_eq!(metrics.released_stacks, 1);
        Ok(())
    }

    #[test]
    fn rejects_zero_and_overflowing_sizes() {
        assert!(StackConfig::new(0).is_err());
        assert!(manager(usize::MAX).is_err());
    }

    #[test]
    fn stacks_are_send_and_release_on_the_owning_thread() -> Result<(), Box<dyn Error>> {
        let manager = manager(4096)?;
        let stack = manager.allocate()?;
        let worker = thread::spawn(move || drop(stack));
        if let Err(payload) = worker.join() {
            std::panic::resume_unwind(payload);
        }
        assert_eq!(manager.metrics()?.active_stacks, 0);
        Ok(())
    }

    #[test]
    fn manager_clones_share_accounting() -> Result<(), Box<dyn Error>> {
        let manager = manager(4096)?;
        let clone = Arc::new(manager.clone());
        let stack = clone.allocate()?;
        assert_eq!(manager.metrics()?.active_stacks, 1);
        drop(stack);
        assert_eq!(manager.metrics()?.active_stacks, 0);
        Ok(())
    }

    #[test]
    fn guard_page_faults_in_a_child_process() -> Result<(), Box<dyn Error>> {
        if std::env::var_os(GUARD_TEST_CHILD).is_some() {
            let manager = manager(4096)?;
            let stack = manager.allocate()?;
            // SAFETY: This child test deliberately writes to the inaccessible guard page
            // to verify that the operating system terminates the process.
            unsafe {
                std::ptr::write_volatile(stack.bounds().guard_start as *mut u8, 1);
            }
            return Err("guard page unexpectedly accepted a write".into());
        }

        let executable = std::env::current_exe()?;
        let output = Command::new(executable)
            .arg("--exact")
            .arg("stack::tests::guard_page_faults_in_a_child_process")
            .arg("--nocapture")
            .env(GUARD_TEST_CHILD, "1")
            .output()?;
        assert_eq!(
            output.status.signal(),
            Some(11),
            "child did not terminate with SIGSEGV: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "performance_tests.rs"]
mod performance_tests;

#[cfg(test)]
#[path = "performance_scale_tests.rs"]
mod performance_scale_tests;

#[cfg(test)]
#[path = "vma_performance_tests.rs"]
mod vma_performance_tests;
