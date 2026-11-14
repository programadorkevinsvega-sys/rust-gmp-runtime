use crate::stack::StackError;
use std::ffi::{c_int, c_void};
use std::io;
use std::ptr;
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::metrics::AtomicMetrics;

const PROT_NONE: c_int = 0;
const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_PRIVATE: c_int = 2;
const MAP_ANONYMOUS: c_int = 0x20;

// SAFETY: Linux mmap/mprotect/munmap signatures follow their documented C ABI.
// The mapping owner validates ranges and releases only its exact reservation.
// Source: https://man7.org/linux/man-pages/man2/mmap.2.html
unsafe extern "C" {
    fn mmap(
        address: *mut c_void,
        length: usize,
        protection: c_int,
        flags: c_int,
        file_descriptor: c_int,
        offset: i64,
    ) -> *mut c_void;
    fn mprotect(address: *mut c_void, length: usize, protection: c_int) -> c_int;
    fn munmap(address: *mut c_void, length: usize) -> c_int;
}

#[derive(Debug)]
pub(super) struct SpanMapping {
    base: usize,
    length: usize,
    metrics: Arc<AtomicMetrics>,
    instrumented: bool,
    #[cfg(test)]
    fail_release: AtomicBool,
}

impl SpanMapping {
    #[cfg(test)]
    pub(super) fn fail_next_release_for_test(&self) {
        self.fail_release.store(true, Ordering::SeqCst);
    }

    pub(super) fn reserve(
        length: usize,
        metrics: Arc<AtomicMetrics>,
        instrumented: bool,
    ) -> Result<Self, StackError> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(StackError::UnsupportedPlatform);
        }
        if length == 0 {
            return Err(StackError::InvalidSize {
                requested_bytes: length,
            });
        }
        let started = instrumented.then(Instant::now);
        // SAFETY: The nonzero length is checked and the anonymous private mapping
        // is initially inaccessible; ownership is transferred to SpanMapping.
        let address = unsafe {
            mmap(
                ptr::null_mut(),
                length,
                PROT_NONE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if let Some(started) = started {
            AtomicMetrics::record_elapsed(&metrics.mmap_ns, started);
            AtomicMetrics::record_call(&metrics.mmap_calls);
        }
        if address as usize == usize::MAX {
            return Err(StackError::SystemCall {
                operation: "mmap(Go-inspired stack span)",
                source: io::Error::last_os_error(),
            });
        }
        if address.is_null() {
            return Err(release_null_mapping(length));
        }
        Ok(Self {
            base: address as usize,
            length,
            metrics,
            instrumented,
            #[cfg(test)]
            fail_release: AtomicBool::new(false),
        })
    }

    pub(super) fn slot_start(&self, index: usize, stride: usize) -> Result<usize, StackError> {
        let offset = index.checked_mul(stride).ok_or(StackError::SizeOverflow)?;
        let end = offset.checked_add(stride).ok_or(StackError::SizeOverflow)?;
        if end > self.length {
            return Err(StackError::SizeOverflow);
        }
        self.base
            .checked_add(offset)
            .ok_or(StackError::SizeOverflow)
    }

    pub(super) fn enable(
        &self,
        slot_start: usize,
        guard_size: usize,
        usable_size: usize,
    ) -> Result<(), StackError> {
        let start = slot_start
            .checked_add(guard_size)
            .ok_or(StackError::SizeOverflow)?;
        let end = start
            .checked_add(usable_size)
            .ok_or(StackError::SizeOverflow)?;
        let mapping_end = self
            .base
            .checked_add(self.length)
            .ok_or(StackError::SizeOverflow)?;
        if slot_start < self.base || end > mapping_end {
            return Err(StackError::SizeOverflow);
        }
        let started = self.instrumented.then(Instant::now);
        // SAFETY: This page-aligned usable range lies within the live PROT_NONE
        // span; its leading guard remains untouched and inaccessible.
        let result = unsafe { mprotect(start as *mut c_void, usable_size, PROT_READ | PROT_WRITE) };
        if let Some(started) = started {
            AtomicMetrics::record_elapsed(&self.metrics.mprotect_ns, started);
            AtomicMetrics::record_call(&self.metrics.mprotect_calls);
        }
        if result == 0 {
            return Ok(());
        }
        Err(StackError::SystemCall {
            operation: "mprotect(Go-inspired stack slot)",
            source: io::Error::last_os_error(),
        })
    }

    pub(super) fn release(&mut self) -> Result<(), StackError> {
        if self.base == 0 {
            return Ok(());
        }
        #[cfg(test)]
        if self.fail_release.swap(false, Ordering::SeqCst) {
            return Err(StackError::SystemCall {
                operation: "munmap(Go-inspired stack span)",
                source: io::Error::other("injected test failure"),
            });
        }
        let started = self.instrumented.then(Instant::now);
        // SAFETY: `base` and `length` are the exact reservation owned by `self`.
        let result = unsafe { munmap(self.base as *mut c_void, self.length) };
        if let Some(started) = started {
            AtomicMetrics::record_elapsed(&self.metrics.munmap_ns, started);
            AtomicMetrics::record_call(&self.metrics.munmap_calls);
        }
        if result == 0 {
            self.base = 0;
            return Ok(());
        }
        Err(StackError::CleanupFailure {
            operation: "munmap(Go-inspired stack span)",
            source: io::Error::last_os_error(),
        })
    }
}

impl Drop for SpanMapping {
    fn drop(&mut self) {
        if let Err(error) = self.release() {
            if let Err(metrics_error) = AtomicMetrics::add(&self.metrics.cleanup_failures, 1) {
                eprintln!(
                    "fatal: failed to account for Go-inspired span cleanup failure: {metrics_error}"
                );
            }
            eprintln!("fatal: failed to release Go-inspired stack span: {error}");
            std::process::abort();
        }
    }
}

mod null_mapping;
use null_mapping::release_null_mapping;
