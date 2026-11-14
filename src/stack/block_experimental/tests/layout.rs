use super::manager;
use std::error::Error;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

const GUARD_CHILD: &str = "RUST_GMP_BLOCK_GUARD_CHILD";
const RELEASED_CHILD: &str = "RUST_GMP_BLOCK_RELEASED_CHILD";

#[test]
fn each_stack_has_a_protected_guard_page() -> Result<(), Box<dyn Error>> {
    if std::env::var_os(GUARD_CHILD).is_some() {
        let manager = manager(4)?;
        let stack = manager.allocate()?;
        // SAFETY: The child deliberately writes to the inaccessible guard page.
        unsafe { std::ptr::write_volatile(stack.bounds().0 as *mut u8, 1) };
        return Err("guard page unexpectedly accepted a write".into());
    }
    let output = child(GUARD_CHILD, "each_stack_has_a_protected_guard_page")?;
    assert_eq!(output.status.signal(), Some(11), "guard did not fault");
    Ok(())
}

#[test]
fn released_slot_is_protected_before_reuse() -> Result<(), Box<dyn Error>> {
    if std::env::var_os(RELEASED_CHILD).is_some() {
        let manager = manager(1)?;
        let stack = manager.allocate()?;
        let address = stack.bounds().1;
        drop(stack);
        // SAFETY: The child deliberately writes to a released, PROT_NONE slot.
        unsafe { std::ptr::write_volatile(address as *mut u8, 1) };
        return Err("released slot unexpectedly remained writable".into());
    }
    let output = child(RELEASED_CHILD, "released_slot_is_protected_before_reuse")?;
    assert_eq!(
        output.status.signal(),
        Some(11),
        "released slot did not fault"
    );
    Ok(())
}

fn child(variable: &str, test_name: &str) -> Result<std::process::Output, Box<dyn Error>> {
    Ok(Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg(format!(
            "stack::block_experimental::tests::layout::{test_name}"
        ))
        .arg("--nocapture")
        .env(variable, "1")
        .output()?)
}
