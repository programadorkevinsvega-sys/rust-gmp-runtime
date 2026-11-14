//! Private architecture-specific context switching.

#![allow(unsafe_code)]

pub(crate) mod machine;
pub(crate) mod switch;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) mod arch;

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
#[path = "tests.rs"]
mod tests;
