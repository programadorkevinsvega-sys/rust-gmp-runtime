//! System V AMD64 register switch and initial trampoline.

use crate::context::machine::MachineContext;
use std::arch::global_asm;

global_asm!(
    r#"
    .text
    .globl oxido_context_switch
    .type oxido_context_switch,@function
oxido_context_switch:
    fnstcw [rdi + 8]
    stmxcsr [rdi + 12]
    and dword ptr [rdi + 12], 0x0000ffc0
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp

    fldcw [rsi + 8]
    ldmxcsr [rsi + 12]
    mov rsp, [rsi]
    cld
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret
    .size oxido_context_switch, .-oxido_context_switch

    .globl oxido_context_bootstrap
    .type oxido_context_bootstrap,@function
oxido_context_bootstrap:
    mov rdi, r12
    sub rsp, 8
    call oxido_context_enter
    ud2
    .size oxido_context_bootstrap, .-oxido_context_bootstrap
    "#
);

unsafe extern "C" {
    fn oxido_context_switch(from: *mut MachineContext, to: *const MachineContext);
    fn oxido_context_bootstrap();
}

pub(crate) fn bootstrap_address() -> usize {
    oxido_context_bootstrap as usize
}

/// # Safety
///
/// Both pointers must reference valid, nonaliasing contexts, the destination
/// stack and continuation must remain live, and lifecycle state must authorize
/// the transfer.
pub(crate) unsafe fn switch(from: *mut MachineContext, to: *const MachineContext) {
    // SAFETY: The caller guarantees valid, nonaliasing contexts and an active
    // routine permit; the assembly stores only the documented ABI state.
    unsafe {
        oxido_context_switch(from, to);
    }
}

#[unsafe(no_mangle)]
pub(crate) extern "C" fn oxido_context_enter(frame: *mut super::super::switch::SwitchFrame) -> ! {
    super::super::switch::enter_routine(frame)
}
