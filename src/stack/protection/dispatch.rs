use std::ptr::NonNull;

use super::StackError;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(in crate::stack) fn page_size() -> Result<usize, StackError> {
    super::linux_x86_64::page_size()
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(in crate::stack) fn page_size() -> Result<usize, StackError> {
    Err(StackError::UnsupportedPlatform)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(in crate::stack) fn reserve(length: usize) -> Result<NonNull<u8>, StackError> {
    super::linux_x86_64::reserve(length)
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(in crate::stack) fn reserve(_length: usize) -> Result<NonNull<u8>, StackError> {
    Err(StackError::UnsupportedPlatform)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(in crate::stack) fn enable_stack(
    base: NonNull<u8>,
    guard_size: usize,
    usable_size: usize,
) -> Result<(), StackError> {
    super::linux_x86_64::enable_stack(base, guard_size, usable_size)
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(in crate::stack) fn enable_stack(
    _base: NonNull<u8>,
    _guard_size: usize,
    _usable_size: usize,
) -> Result<(), StackError> {
    Err(StackError::UnsupportedPlatform)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(in crate::stack) fn release(base: NonNull<u8>, length: usize) -> Result<(), StackError> {
    super::linux_x86_64::release(base, length)
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub(in crate::stack) fn release(_base: NonNull<u8>, _length: usize) -> Result<(), StackError> {
    Err(StackError::UnsupportedPlatform)
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::{enable_stack, page_size, release, reserve};
    use crate::stack::StackError;

    #[test]
    fn reports_mprotect_failure_for_unaligned_range() -> Result<(), StackError> {
        let page_size = page_size()?;
        let reservation_size = page_size * 2;
        let base = reserve(reservation_size)?;
        let result = enable_stack(base, 1, page_size);
        release(base, reservation_size)?;

        assert!(matches!(
            result,
            Err(StackError::SystemCall {
                operation: "mprotect",
                ..
            })
        ));
        Ok(())
    }
}
