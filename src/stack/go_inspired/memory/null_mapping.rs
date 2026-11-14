use super::*;

pub(super) fn release_null_mapping(length: usize) -> StackError {
    // SAFETY: mmap returned null as a successful mapping, so release exactly that
    // range before reporting that this allocator cannot represent its base.
    let result = unsafe { munmap(ptr::null_mut(), length) };
    if result == 0 {
        return StackError::InvalidSystemValue {
            operation: "mmap(Go-inspired stack span)",
            value: 0,
        };
    }
    StackError::CleanupAfterFailure {
        original: Box::new(StackError::InvalidSystemValue {
            operation: "mmap(Go-inspired stack span)",
            value: 0,
        }),
        cleanup: Box::new(StackError::CleanupFailure {
            operation: "munmap(null Go-inspired stack span)",
            source: io::Error::last_os_error(),
        }),
    }
}
