use std::ffi::{c_int, c_void};
use std::io;
use std::ptr::{self, NonNull};

const PROT_NONE: c_int = 0;
const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_PRIVATE: c_int = 2;
const MAP_ANONYMOUS: c_int = 0x20;

// SAFETY: These declarations match the Linux mmap/mprotect/munmap C ABI. The
// Mapping owner keeps each successful reservation alive through its final use.
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

pub(super) struct Mapping {
    base: NonNull<u8>,
    length: usize,
}

impl Mapping {
    pub(super) fn reserve(length: usize) -> Result<Self, io::Error> {
        // SAFETY: The call requests a nonzero anonymous private mapping; the
        // returned range is transferred to this unique Mapping owner.
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
            return Err(io::Error::last_os_error());
        }
        let base = match NonNull::new(address.cast::<u8>()) {
            Some(base) => base,
            None => {
                // SAFETY: mmap succeeded at address zero; release the exact range
                // before returning because Mapping cannot represent a null owner.
                let result = unsafe { munmap(address, length) };
                if result != 0 {
                    return Err(io::Error::last_os_error());
                }
                return Err(io::Error::other("mmap returned null; mapping released"));
            }
        };
        Ok(Self { base, length })
    }

    pub(super) fn enable(&self, offset: usize, length: usize) -> Result<(), io::Error> {
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= self.length)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range exceeds mapping"))?;
        // SAFETY: The checked range is page-aligned, lies within this live
        // mapping, and only its access permissions are changed.
        let address = unsafe { self.base.as_ptr().add(offset) };
        // SAFETY: address and length describe the checked region in this mapping.
        let result = unsafe {
            mprotect(
                address.cast::<c_void>(),
                end - offset,
                PROT_READ | PROT_WRITE,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: Mapping uniquely owns this exact live reservation.
        let result = unsafe { munmap(self.base.as_ptr().cast::<c_void>(), self.length) };
        if result != 0 {
            eprintln!(
                "fatal: experimental mapping cleanup failed: {}",
                io::Error::last_os_error()
            );
            std::process::abort();
        }
    }
}
