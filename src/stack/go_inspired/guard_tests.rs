use crate::stack::{GoInspiredOptions, GoInspiredStackManager, StackConfig};
use std::error::Error;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

const CHILD_MARKER: &str = "RUST_GMP_GO_INSPIRED_GUARD_CHILD";

#[test]
fn guard_page_faults_in_child_process() -> Result<(), Box<dyn Error>> {
    if std::env::var_os(CHILD_MARKER).is_some() {
        let manager = GoInspiredStackManager::with_options(
            StackConfig::new(8 * 1024)?,
            GoInspiredOptions::default(),
        )?;
        let stack = manager.allocate()?;
        // SAFETY: The child deliberately writes to its inaccessible guard page.
        unsafe {
            std::ptr::write_volatile(stack.bounds().guard_start as *mut u8, 1);
        }
        return Err("guard page unexpectedly accepted a write".into());
    }

    let output = Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg("stack::go_inspired::tests::guard_tests::guard_page_faults_in_child_process")
        .env(CHILD_MARKER, "1")
        .output()?;
    assert_eq!(
        output.status.signal(),
        Some(11),
        "child did not terminate with SIGSEGV: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
