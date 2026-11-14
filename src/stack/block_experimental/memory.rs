use crate::stack::StackError;
use std::ffi::{c_int, c_void};
use std::io;
use std::ptr;

const PROT_NONE: c_int = 0;
const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_PRIVATE: c_int = 2;
const MAP_ANONYMOUS: c_int = 0x20;
const MADV_DONTNEED: c_int = 4;

// SAFETY: Linux ABI declarations. Mapping owns every successful range; methods
// validate offsets and lengths before passing addresses to the kernel.
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
    fn madvise(address: *mut c_void, length: usize, advice: c_int) -> c_int;
    fn munmap(address: *mut c_void, length: usize) -> c_int;
}

#[derive(Debug)]
pub(super) struct Mapping {
    base: usize,
    length: usize,
}

impl Mapping {
    pub(super) fn reserve(length: usize) -> Result<Self, StackError> {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(StackError::UnsupportedPlatform);
        }
        // SAFETY: A nonzero checked arena size requests an anonymous private
        // reservation; the returned range is transferred to Mapping.
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
        if address as usize == usize::MAX {
            return Err(StackError::SystemCall {
                operation: "mmap(block arena)",
                source: io::Error::last_os_error(),
            });
        }
        let base = address as usize;
        if base == 0 {
            // SAFETY: mmap succeeded at null, so release exactly the range it returned.
            let result = unsafe { munmap(address, length) };
            if result != 0 {
                return Err(StackError::CleanupAfterFailure {
                    original: Box::new(StackError::InvalidSystemValue {
                        operation: "mmap(block arena)",
                        value: 0,
                    }),
                    cleanup: Box::new(release_error("munmap(block arena)", result)),
                });
            }
            return Err(StackError::InvalidSystemValue {
                operation: "mmap(block arena)",
                value: 0,
            });
        }
        Ok(Self { base, length })
    }

    pub(super) fn release(&mut self) -> Result<(), StackError> {
        if self.base == 0 {
            return Err(StackError::MetricsOverflow);
        }
        // SAFETY: Mapping uniquely owns this exact live reservation.
        let result = unsafe { munmap(self.base as *mut c_void, self.length) };
        if result == 0 {
            self.base = 0;
            return Ok(());
        }
        Err(release_error("munmap(block arena)", result))
    }

    pub(super) fn address(&self, offset: usize, length: usize) -> Result<usize, StackError> {
        self.checked_address(offset, length)
    }

    pub(super) fn enable_at(address: usize, length: usize) -> Result<(), StackError> {
        Self::protect_at(
            address,
            length,
            PROT_READ | PROT_WRITE,
            "mprotect(enable block slot)",
        )
    }

    pub(super) fn disable_at(address: usize, length: usize) -> Result<(), StackError> {
        Self::protect_at(address, length, PROT_NONE, "mprotect(disable block slot)")
    }

    pub(super) fn discard_at(address: usize, length: usize) -> Result<(), StackError> {
        if address == 0 || length == 0 {
            return Err(StackError::SizeOverflow);
        }
        // SAFETY: The caller derives this page-aligned address and length from a
        // live owned arena after the slot has been protected.
        let result = unsafe { madvise(address as *mut c_void, length, MADV_DONTNEED) };
        if result == 0 {
            return Ok(());
        }
        Err(StackError::SystemCall {
            operation: "madvise(MADV_DONTNEED)",
            source: io::Error::last_os_error(),
        })
    }

    fn protect_at(
        address: usize,
        length: usize,
        permissions: c_int,
        operation: &'static str,
    ) -> Result<(), StackError> {
        if address == 0 || length == 0 {
            return Err(StackError::SizeOverflow);
        }
        // SAFETY: The address and length identify a page-aligned live slot in an
        // arena owned by the manager; its transition state excludes other users.
        let result = unsafe { mprotect(address as *mut c_void, length, permissions) };
        if result == 0 {
            return Ok(());
        }
        Err(StackError::SystemCall {
            operation,
            source: io::Error::last_os_error(),
        })
    }

    fn checked_address(&self, offset: usize, length: usize) -> Result<usize, StackError> {
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= self.length)
            .ok_or(StackError::SizeOverflow)?;
        self.base
            .checked_add(offset)
            .filter(|_| end > offset)
            .ok_or(StackError::SizeOverflow)
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        if self.base == 0 {
            return;
        }
        // SAFETY: Mapping uniquely owns this exact live reservation.
        if let Err(error) = self.release() {
            eprintln!("fatal: failed to release experimental block arena: {error}");
            std::process::abort();
        }
    }
}

fn release_error(operation: &'static str, result: c_int) -> StackError {
    if result == -1 {
        StackError::CleanupFailure {
            operation,
            source: io::Error::last_os_error(),
        }
    } else {
        StackError::InvalidSystemValue {
            operation,
            value: i64::from(result),
        }
    }
}
