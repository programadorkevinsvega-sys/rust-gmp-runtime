use std::error::Error;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::ptr::NonNull;
use std::sync::Arc;

use super::Stack;
use crate::stack::{StackConfig, StackManager};

const MUNMAP_FAILURE_CHILD: &str = "RUST_GMP_STACK_MUNMAP_FAILURE_CHILD";
#[cfg(feature = "allocator-benchmark")]
const TWO_PHASE_MUNMAP_FAILURE_CHILD: &str = "RUST_GMP_TWO_PHASE_MUNMAP_FAILURE_CHILD";

#[test]
fn munmap_failure_during_drop_aborts_in_an_isolated_process() -> Result<(), Box<dyn Error>> {
    if std::env::var_os(MUNMAP_FAILURE_CHILD).is_some() {
        let manager = StackManager::new(StackConfig::new(4096)?)?;
        drop(invalid_stack(
            &manager,
            false,
            #[cfg(feature = "allocator-benchmark")]
            None,
        ));
        return Err("Drop unexpectedly accepted an invalid mapping".into());
    }

    let output = Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg("stack::mapping::tests::munmap_failure_during_drop_aborts_in_an_isolated_process")
        .arg("--nocapture")
        .env(MUNMAP_FAILURE_CHILD, "1")
        .output()?;
    assert_abort(&output, "Drop");
    Ok(())
}

#[cfg(feature = "allocator-benchmark")]
#[test]
fn two_phase_munmap_failure_during_drop_aborts_in_an_isolated_process() -> Result<(), Box<dyn Error>>
{
    if std::env::var_os(TWO_PHASE_MUNMAP_FAILURE_CHILD).is_some() {
        let (manager, metrics) = StackManager::new_benchmarked_two_phase(StackConfig::new(4096)?)?;
        drop(invalid_stack(&manager, true, Some(metrics)));
        return Err("two-phase Drop unexpectedly accepted an invalid mapping".into());
    }

    let output = Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg("stack::mapping::tests::two_phase_munmap_failure_during_drop_aborts_in_an_isolated_process")
        .arg("--nocapture")
        .env(TWO_PHASE_MUNMAP_FAILURE_CHILD, "1")
        .output()?;
    assert_abort(&output, "two-phase Drop");
    Ok(())
}

fn invalid_stack(
    manager: &StackManager,
    two_phase: bool,
    #[cfg(feature = "allocator-benchmark")] metrics: Option<
        Arc<super::super::benchmark_metrics::StackBenchmarkMetrics>,
    >,
) -> Stack {
    Stack::new(
        NonNull::dangling(),
        manager.inner.reservation_size,
        manager.inner.page_size,
        manager.inner.usable_size,
        Arc::clone(&manager.inner),
        two_phase,
        #[cfg(feature = "allocator-benchmark")]
        metrics,
    )
}

fn assert_abort(output: &std::process::Output, context: &str) {
    assert_eq!(
        output.status.signal(),
        Some(6),
        "{context} did not abort after munmap failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("fatal: failed to release routine stack mapping")
            && stderr.contains("munmap"),
        "child did not report the munmap failure: {stderr}"
    );
}
