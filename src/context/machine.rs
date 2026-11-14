//! Register state exchanged by the private switch backend.

use crate::routine::RoutineError;
use crate::stack::StackBounds;

#[repr(C, align(16))]
#[derive(Debug)]
pub(crate) struct MachineContext {
    stack_pointer: usize,
    resume_address: usize,
    rbx: usize,
    rbp: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    x87_control: u16,
    _reserved: u16,
    mxcsr_control: u32,
    _padding: [u8; 8],
}

impl MachineContext {
    pub(crate) const fn empty() -> Self {
        Self {
            stack_pointer: 0,
            resume_address: 0,
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            x87_control: 0x037f,
            _reserved: 0,
            mxcsr_control: 0x1f80,
            _padding: [0; 8],
        }
    }

    pub(crate) fn initial_stack_pointer(bounds: StackBounds) -> Result<usize, RoutineError> {
        if bounds.usable_start >= bounds.usable_end {
            return Err(RoutineError::InvalidContextStackBounds);
        }
        let aligned_top = bounds.usable_end & !15;
        let stack_pointer = aligned_top
            .checked_sub(8)
            .ok_or(RoutineError::InsufficientContextStack)?;
        let entry_stack_pointer = stack_pointer
            .checked_sub(16)
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
    pub(crate) fn initialize(&mut self, stack_pointer: usize, frame: *mut ()) {
        self.stack_pointer = stack_pointer;
        self.resume_address = super::arch::x86_64::bootstrap_address();
        self.r12 = frame as usize;
    }

    pub(crate) fn saved_stack_pointer(&self) -> Option<usize> {
        (self.stack_pointer != 0).then_some(self.stack_pointer)
    }
}

const _: () = {
    assert!(std::mem::offset_of!(MachineContext, stack_pointer) == 0);
    assert!(std::mem::offset_of!(MachineContext, resume_address) == 8);
    assert!(std::mem::offset_of!(MachineContext, rbx) == 16);
    assert!(std::mem::offset_of!(MachineContext, rbp) == 24);
    assert!(std::mem::offset_of!(MachineContext, r12) == 32);
    assert!(std::mem::offset_of!(MachineContext, r13) == 40);
    assert!(std::mem::offset_of!(MachineContext, r14) == 48);
    assert!(std::mem::offset_of!(MachineContext, r15) == 56);
    assert!(std::mem::offset_of!(MachineContext, x87_control) == 64);
    assert!(std::mem::offset_of!(MachineContext, mxcsr_control) == 68);
    assert!(std::mem::size_of::<MachineContext>() == 80);
};
