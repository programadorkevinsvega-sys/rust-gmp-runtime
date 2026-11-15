use std::arch::{asm, global_asm};

global_asm!(
    r#"
    .text
    .globl oxido_test_stack_pointer
    .type oxido_test_stack_pointer,@function
oxido_test_stack_pointer:
    mov rax, rsp
    ret
    .size oxido_test_stack_pointer, .-oxido_test_stack_pointer

    .globl oxido_test_run_context_probe
    .type oxido_test_run_context_probe,@function
oxido_test_run_context_probe:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    sub rsp, 8
    mov r10, rdi
    mov r11, rsi
    movabs rbx, 0x1122334455667788
    movabs rbp, 0x2233445566778899
    movabs r12, 0x33445566778899aa
    movabs r13, 0x445566778899aabb
    movabs r14, 0x5566778899aabbcc
    movabs r15, 0x66778899aabbccdd
    mov rdi, r10
    mov rsi, r11
    call oxido_context_switch
    xor eax, eax
    movabs r10, 0x1122334455667788
    cmp rbx, r10
    jne 1f
    or eax, 1
1:
    movabs r10, 0x2233445566778899
    cmp rbp, r10
    jne 2f
    or eax, 2
2:
    movabs r10, 0x33445566778899aa
    cmp r12, r10
    jne 3f
    or eax, 4
3:
    movabs r10, 0x445566778899aabb
    cmp r13, r10
    jne 4f
    or eax, 8
4:
    movabs r10, 0x5566778899aabbcc
    cmp r14, r10
    jne 5f
    or eax, 16
5:
    movabs r10, 0x66778899aabbccdd
    cmp r15, r10
    jne 6f
    or eax, 32
6:
    add rsp, 8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
    .size oxido_test_run_context_probe, .-oxido_test_run_context_probe

    .globl oxido_test_context_bootstrap
    .type oxido_test_context_bootstrap,@function
oxido_test_context_bootstrap:
    mov rdi, [r12 + 8]
    mov rsi, [r12]
    sub rsp, 8
    call oxido_context_switch
    ud2
    .size oxido_test_context_bootstrap, .-oxido_test_context_bootstrap

    "#
);

unsafe extern "C" {
    pub(super) fn oxido_test_stack_pointer() -> usize;
    pub(super) fn oxido_test_run_context_probe(
        scheduler: *mut super::machine::MachineContext,
        routine: *const super::machine::MachineContext,
    ) -> u32;
    fn oxido_test_context_bootstrap();
}

pub(super) fn context_bootstrap_address() -> usize {
    oxido_test_context_bootstrap as usize
}

pub(super) struct FpControlGuard {
    original_x87: u16,
    original_mxcsr: u32,
}

impl FpControlGuard {
    pub(super) fn with_rounding_toward_zero() -> Self {
        let mut original_x87 = 0_u16;
        let mut original_mxcsr = 0_u32;
        // SAFETY: The instruction writes one fixed-size value to a valid local.
        unsafe {
            asm!(
                "fnstcw [{pointer}]",
                pointer = in(reg) &mut original_x87,
                options(nostack, preserves_flags)
            );
        }
        // SAFETY: The instruction writes one fixed-size value to a valid local.
        unsafe {
            asm!(
                "stmxcsr [{pointer}]",
                pointer = in(reg) &mut original_mxcsr,
                options(nostack, preserves_flags)
            );
        }
        let test_x87 = (original_x87 & !0x0c00) | 0x0c00;
        let test_mxcsr = (original_mxcsr & !0x6000) | 0x6000;
        // SAFETY: This valid control word changes only the current thread's
        // x87 rounding mode.
        unsafe {
            asm!(
                "fldcw [{pointer}]",
                pointer = in(reg) &test_x87,
                options(nostack, preserves_flags)
            );
        }
        // SAFETY: This valid control word changes only the current thread's
        // MXCSR rounding mode.
        unsafe {
            asm!(
                "ldmxcsr [{pointer}]",
                pointer = in(reg) &test_mxcsr,
                options(nostack, preserves_flags)
            );
        }
        Self {
            original_x87,
            original_mxcsr,
        }
    }

    pub(super) fn current_control_words() -> (u16, u32) {
        let mut x87 = 0_u16;
        let mut mxcsr = 0_u32;
        // SAFETY: The instruction writes one fixed-size value to a valid local.
        unsafe {
            asm!(
                "fnstcw [{pointer}]",
                pointer = in(reg) &mut x87,
                options(nostack, preserves_flags)
            );
        }
        // SAFETY: The instruction writes one fixed-size value to a valid local.
        unsafe {
            asm!(
                "stmxcsr [{pointer}]",
                pointer = in(reg) &mut mxcsr,
                options(nostack, preserves_flags)
            );
        }
        (x87, mxcsr)
    }
}

impl Drop for FpControlGuard {
    fn drop(&mut self) {
        // SAFETY: This is the x87 control word saved from this thread.
        unsafe {
            asm!(
                "fldcw [{pointer}]",
                pointer = in(reg) &self.original_x87,
                options(nostack, preserves_flags)
            );
        }
        // SAFETY: This is the MXCSR control word saved from this thread.
        unsafe {
            asm!(
                "ldmxcsr [{pointer}]",
                pointer = in(reg) &self.original_mxcsr,
                options(nostack, preserves_flags)
            );
        }
    }
}
