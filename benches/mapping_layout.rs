//! Compares individual mappings with a grouped virtual-memory reservation.
#![allow(unsafe_code)]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("mapping_layout requires Linux x86_64");

#[path = "mapping_layout/mod.rs"]
mod mapping_layout;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    mapping_layout::run()
}
