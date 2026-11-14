use crate::stack::{GoInspiredOptions, GoInspiredStackManager, StackConfig, StackError};
use std::collections::BTreeSet;
use std::error::Error;
use std::sync::Arc;
use std::thread;

#[path = "failure_tests.rs"]
mod failure_tests;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "guard_tests.rs"]
mod guard_tests;
#[path = "snapshot_tests.rs"]
mod snapshot_tests;

fn manager() -> Result<GoInspiredStackManager, Box<dyn Error>> {
    let options = GoInspiredOptions::default()
        .with_slots_per_span(8)
        .with_retained_spans(0);
    Ok(GoInspiredStackManager::with_options(
        StackConfig::new(8 * 1024)?,
        options,
    )?)
}

#[path = "tests/cache_tests.rs"]
mod cache_tests;
#[path = "tests/recovery_tests.rs"]
mod recovery_tests;
#[path = "tests/trim_tests.rs"]
mod trim_tests;
