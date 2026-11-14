//! System V AMD64 register switch and initial trampoline.

use crate::context::machine::MachineContext;
use std::arch::global_asm;

global_asm!(
    r#"
    .text
    .globl oxido_context_switch
    .type oxido_context_switch,@function
oxido_context_switch:
    mov [rdi + 0], rsp
    lea rax, [rip + .Loxido_context_resume]
    mov [rdi + 8], rax
    mov [rdi + 16], rbx
    mov [rdi + 24], rbp
    mov [rdi + 32], r12
    mov [rdi + 40], r13
    mov [rdi + 48], r14
    mov [rdi + 56], r15
    fnstcw [rdi + 64]
    stmxcsr [rdi + 68]
    mov eax, [rdi + 68]
    and eax, 0x0000ffc0
    mov [rdi + 68], eax

    mov rbx, [rsi + 16]
    mov rbp, [rsi + 24]
    mov r12, [rsi + 32]
    mov r13, [rsi + 40]
    mov r14, [rsi + 48]
    mov r15, [rsi + 56]
    fldcw [rsi + 64]
    ldmxcsr [rsi + 68]
    mov rsp, [rsi + 0]
    cld
    jmp [rsi + 8]
.Loxido_context_resume:
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
