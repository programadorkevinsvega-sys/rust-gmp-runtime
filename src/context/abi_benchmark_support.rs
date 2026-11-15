use std::arch::global_asm;

global_asm!(
    r#"
    .text
    .globl oxido_test_legacy_context_switch
    .type oxido_test_legacy_context_switch,@function
oxido_test_legacy_context_switch:
    mov [rdi + 0], rsp
    lea rax, [rip + .Loxido_legacy_context_resume]
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
.Loxido_legacy_context_resume:
    ret
    .size oxido_test_legacy_context_switch, .-oxido_test_legacy_context_switch

    .globl oxido_test_legacy_context_bootstrap
    .type oxido_test_legacy_context_bootstrap,@function
oxido_test_legacy_context_bootstrap:
.Loxido_legacy_context_loop:
    mov rdi, [r12 + 8]
    mov rsi, [r12]
    sub rsp, 8
    call oxido_test_legacy_context_switch
    add rsp, 8
    jmp .Loxido_legacy_context_loop
    .size oxido_test_legacy_context_bootstrap, .-oxido_test_legacy_context_bootstrap

    .globl oxido_test_new_context_bootstrap
    .type oxido_test_new_context_bootstrap,@function
oxido_test_new_context_bootstrap:
.Loxido_new_context_loop:
    mov rdi, [r12 + 8]
    mov rsi, [r12]
    sub rsp, 8
    call oxido_context_switch
    add rsp, 8
    jmp .Loxido_new_context_loop
    .size oxido_test_new_context_bootstrap, .-oxido_test_new_context_bootstrap
    "#
);

unsafe extern "C" {
    pub(super) fn oxido_test_legacy_context_switch(from: *mut (), to: *const ());
    fn oxido_test_legacy_context_bootstrap();
    fn oxido_test_new_context_bootstrap();
}

pub(super) fn legacy_context_bootstrap_address() -> usize {
    oxido_test_legacy_context_bootstrap as usize
}

pub(super) fn new_context_benchmark_bootstrap_address() -> usize {
    oxido_test_new_context_bootstrap as usize
}
