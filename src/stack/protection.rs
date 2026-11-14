use super::error::StackError;

pub(super) mod dispatch;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux_x86_64 {
    use std::ffi::{c_int, c_long, c_void};
    use std::io;
    use std::ptr::{self, NonNull};

    use super::StackError;

    const SC_PAGESIZE: c_int = 30;
    const PROT_NONE: c_int = 0;
    const PROT_READ: c_int = 0x1;
    const PROT_WRITE: c_int = 0x2;
    const MAP_PRIVATE: c_int = 0x02;
    const MAP_ANONYMOUS: c_int = 0x20;

    // SAFETY: These ABI declarations match the documented Linux C interfaces;
    // pointer validity and mapping ownership are checked at each call site.
    // FFI contracts: https://man7.org/linux/man-pages/man3/sysconf.3.html,
    // https://man7.org/linux/man-pages/man2/mmap.2.html,
    // https://man7.org/linux/man-pages/man2/mprotect.2.html,
    // https://man7.org/linux/man-pages/man2/munmap.2.html.
    unsafe extern "C" {
        fn sysconf(name: c_int) -> c_long;
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

    pub(super) fn page_size() -> Result<usize, StackError> {
        // SAFETY: sysconf reads the process page-size setting and does not retain pointers.
        let value = unsafe { sysconf(SC_PAGESIZE) };
        if value <= 0 {
            return Err(StackError::InvalidSystemValue {
                operation: "sysconf(_SC_PAGESIZE)",
                value: value as i64,
            });
        }
        match usize::try_from(value) {
            Ok(page_size) => Ok(page_size),
            Err(source) => Err(StackError::IntegerConversion {
                operation: "sysconf(_SC_PAGESIZE) result",
                source,
            }),
        }
    }

    pub(super) fn reserve(length: usize) -> Result<NonNull<u8>, StackError> {
        // SAFETY: mmap is called with a null address hint, a checked nonzero length,
        // anonymous private mapping, and no file descriptor; the returned region is
        // owned by the caller until munmap.
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
                operation: "mmap",
                source: io::Error::last_os_error(),
            });
        }

        match NonNull::new(address.cast::<u8>()) {
            Some(address) => Ok(address),
            None => {
                let error = StackError::InvalidSystemValue {
                    operation: "mmap",
                    value: 0,
                };
                match release_null_mapping(length) {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(StackError::CleanupAfterFailure {
                        original: Box::new(error),
                        cleanup: Box::new(cleanup),
                    }),
                }
            }
        }
    }

    pub(super) fn enable_stack(
        base: NonNull<u8>,
        guard_size: usize,
        usable_size: usize,
    ) -> Result<(), StackError> {
        // SAFETY: base came from a live mmap reservation, guard_size is one complete
        // page, and usable_size is page-rounded and stays within that reservation.
        let address = unsafe { base.as_ptr().add(guard_size) };
        // SAFETY: the computed range is page-aligned, belongs to the live reservation,
        // and mprotect changes only its access permissions.
        let result = unsafe {
            mprotect(
                address.cast::<c_void>(),
                usable_size,
                PROT_READ | PROT_WRITE,
            )
        };
        if result == -1 {
            return Err(StackError::SystemCall {
                operation: "mprotect",
                source: io::Error::last_os_error(),
            });
        }
        if result != 0 {
            return Err(StackError::InvalidSystemValue {
                operation: "mprotect",
                value: i64::from(result),
            });
        }
        Ok(())
    }

    pub(super) fn release(base: NonNull<u8>, length: usize) -> Result<(), StackError> {
        // SAFETY: base and length exactly identify the live reservation returned by
        // mmap; Stack owns it uniquely and calls this function once.
        let result = unsafe { munmap(base.as_ptr().cast::<c_void>(), length) };
        check_release_result(result)
    }

    fn release_null_mapping(length: usize) -> Result<(), StackError> {
        // SAFETY: mmap returned the null address as a valid mapping; this releases the
        // exact length immediately because NonNull cannot represent that address.
        let result = unsafe { munmap(ptr::null_mut(), length) };
        check_release_result(result)
    }

    fn check_release_result(result: c_int) -> Result<(), StackError> {
        if result == 0 {
            return Ok(());
        }
        if result == -1 {
            return Err(StackError::CleanupFailure {
                operation: "munmap",
                source: io::Error::last_os_error(),
            });
        }
        Err(StackError::InvalidSystemValue {
            operation: "munmap",
            value: i64::from(result),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{StackError, check_release_result};

        #[test]
        fn reports_release_syscall_failure() {
            assert!(matches!(
                check_release_result(-1),
                Err(StackError::CleanupFailure {
                    operation: "munmap",
                    ..
                })
            ));
        }

        #[test]
        fn rejects_unexpected_release_result() {
            assert!(matches!(
                check_release_result(1),
                Err(StackError::InvalidSystemValue {
                    operation: "munmap",
                    value: 1
                })
            ));
        }

        #[test]
        fn reports_unaligned_unmap_as_cleanup_failure() {
            // Linux rejects this unaligned address without unmapping any region.
            assert!(matches!(
                super::release(std::ptr::NonNull::dangling(), 4096),
                Err(StackError::CleanupFailure {
                    operation: "munmap",
                    ..
                })
            ));
        }
    }
}
