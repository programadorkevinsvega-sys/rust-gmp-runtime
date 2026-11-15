//! Register state exchanged by the private switch backend.

use crate::routine::RoutineError;
use crate::stack::StackBounds;

#[repr(C, align(16))]
#[derive(Debug)]
pub(crate) struct MachineContext {
    stack_pointer: usize,
    x87_control: u16,
    _reserved: u16,
    mxcsr_control: u32,
}

impl MachineContext {
    pub(crate) const fn empty() -> Self {
        Self {
            stack_pointer: 0,
            x87_control: 0x037f,
            _reserved: 0,
            mxcsr_control: 0x1f80,
        }
    }

    pub(crate) fn initial_stack_pointer(bounds: StackBounds) -> Result<usize, RoutineError> {
        if bounds.usable_start >= bounds.usable_end {
            return Err(RoutineError::InvalidContextStackBounds);
        }
        let aligned_top = bounds.usable_end & !15;
        let stack_pointer = aligned_top
            .checked_sub(64)
            .ok_or(RoutineError::InsufficientContextStack)?;
        let entry_stack_pointer = aligned_top
            .checked_sub(8)
            .ok_or(RoutineError::InsufficientContextStack)?;
        let red_zone_start = entry_stack_pointer
            .checked_sub(128)
            .ok_or(RoutineError::InsufficientContextStack)?;
        if red_zone_start < bounds.usable_start {
            return Err(RoutineError::InsufficientContextStack);
        }
        Ok(stack_pointer)
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    ///
    /// # Safety
    ///
    /// `stack_pointer` must address at least seven writable `usize` slots in a
    /// live, writable stack mapping.
    pub(crate) unsafe fn initialize(&mut self, stack_pointer: usize, frame: *mut ()) {
        self.stack_pointer = stack_pointer;
        // SAFETY: The caller guarantees the initialized range is writable.
        unsafe {
            initialize_stack(
                stack_pointer,
                super::arch::x86_64::bootstrap_address(),
                frame,
            );
        }
    }

    #[cfg(test)]
    ///
    /// # Safety
    ///
    /// `stack_pointer` must address at least seven writable `usize` slots in a
    /// live, writable stack mapping.
    pub(crate) unsafe fn initialize_test(
        &mut self,
        stack_pointer: usize,
        resume_address: usize,
        frame: *mut (),
    ) {
        self.stack_pointer = stack_pointer;
        // SAFETY: The caller guarantees the initialized range is writable.
        unsafe {
            initialize_stack(stack_pointer, resume_address, frame);
        }
    }

    pub(crate) fn saved_stack_pointer(&self) -> Option<usize> {
        (self.stack_pointer != 0).then_some(self.stack_pointer)
    }
}

unsafe fn initialize_stack(stack_pointer: usize, resume_address: usize, frame: *mut ()) {
    let initial_registers = [0_usize, 0, 0, frame as usize, 0, 0, resume_address];
    for (index, value) in initial_registers.into_iter().enumerate() {
        // SAFETY: The caller guarantees seven writable stack slots from this address.
        let slot = unsafe { (stack_pointer as *mut usize).add(index) };
        // SAFETY: `slot` points to one of the writable slots guaranteed above.
        unsafe { slot.write_volatile(value) };
    }
}

const _: () = {
    assert!(std::mem::offset_of!(MachineContext, stack_pointer) == 0);
    assert!(std::mem::offset_of!(MachineContext, x87_control) == 8);
    assert!(std::mem::offset_of!(MachineContext, mxcsr_control) == 12);
    assert!(std::mem::size_of::<MachineContext>() == 16);
};
