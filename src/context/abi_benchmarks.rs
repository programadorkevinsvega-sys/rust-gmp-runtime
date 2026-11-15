use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use super::abi_benchmark_support::{
    legacy_context_bootstrap_address, new_context_benchmark_bootstrap_address,
    oxido_test_legacy_context_switch,
};
use super::machine::MachineContext;
use crate::stack::{StackConfig, StackManager};

#[path = "abi_benchmarks/stats.rs"]
mod stats;

const ROUNDS: usize = 1_000_000;
const SAMPLES: usize = 20;

#[repr(C, align(16))]
struct LegacyMachineContext {
    stack_pointer: usize,
    resume_address: usize,
    rbx: usize,
    rbp: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    x87_control: u16,
    reserved: u16,
    mxcsr_control: u32,
    padding: [u8; 8],
}

impl LegacyMachineContext {
    fn empty() -> Self {
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
            reserved: 0,
            mxcsr_control: 0x1f80,
            padding: [0; 8],
        }
    }
}

#[repr(C)]
struct ContextPointers {
    scheduler: *mut (),
    routine: *mut (),
}

#[test]
#[ignore = "manual interleaved old/new context-switch benchmark"]
fn interleaved_context_switch_backend_comparison() -> Result<(), Box<dyn Error>> {
    let manager = StackManager::new(StackConfig::new(32 * 1024)?)?;
    let old_stack = manager.allocate()?;
    let new_stack = manager.allocate()?;
    prefault_stack(old_stack.bounds(), manager.page_size())?;
    prefault_stack(new_stack.bounds(), manager.page_size())?;

    let old_sp = (old_stack.bounds().usable_end & !15)
        .checked_sub(8)
        .ok_or("legacy stack pointer underflow")?;
    let new_sp = MachineContext::initial_stack_pointer(new_stack.bounds())?;
    let mut old_scheduler = LegacyMachineContext::empty();
    let mut old_routine = LegacyMachineContext::empty();
    let mut new_scheduler = MachineContext::empty();
    let mut new_routine = MachineContext::empty();
    let mut old_pointers = ContextPointers {
        scheduler: (&mut old_scheduler as *mut LegacyMachineContext).cast(),
        routine: (&mut old_routine as *mut LegacyMachineContext).cast(),
    };
    let mut new_pointers = ContextPointers {
        scheduler: (&mut new_scheduler as *mut MachineContext).cast(),
        routine: (&mut new_routine as *mut MachineContext).cast(),
    };
    old_routine.stack_pointer = old_sp;
    old_routine.resume_address = legacy_context_bootstrap_address();
    old_routine.r12 = (&mut old_pointers as *mut ContextPointers) as usize;
    let old_routine_pointer = (&mut old_routine as *mut LegacyMachineContext).cast();
    let new_routine_pointer = (&mut new_routine as *mut MachineContext).cast();
    // SAFETY: The new stack pointer was computed from its live writable mapping.
    unsafe {
        new_routine.initialize_test(
            new_sp,
            new_context_benchmark_bootstrap_address(),
            (&mut new_pointers as *mut ContextPointers).cast(),
        );
    }

    let mut old_samples = Vec::with_capacity(SAMPLES);
    let mut new_samples = Vec::with_capacity(SAMPLES);
    println!(
        "context_switch_ab target=linux-x86_64 samples={SAMPLES} roundtrips_per_sample={ROUNDS} stacks=prefaulted old_context_bytes={} new_context_bytes={}",
        std::mem::size_of::<LegacyMachineContext>(),
        std::mem::size_of::<MachineContext>()
    );
    let mut paired_differences = Vec::with_capacity(SAMPLES);
    for sample in 0..SAMPLES {
        let (old, new) = if sample % 2 == 0 {
            (
                measure_old(&mut old_scheduler, old_routine_pointer),
                measure_new(&mut new_scheduler, new_routine_pointer),
            )
        } else {
            let new = measure_new(&mut new_scheduler, new_routine_pointer);
            let old = measure_old(&mut old_scheduler, old_routine_pointer);
            (old, new)
        };
        old_samples.push(old);
        new_samples.push(new);
        paired_differences.push(new - old);
        println!(
            "context_switch_ab sample={sample} old_ns_per_transfer={old:.3} new_ns_per_transfer={new:.3} new_minus_old_ns={:.3}",
            new - old
        );
    }
    black_box((
        old_routine.rbx,
        old_routine.rbp,
        old_routine.r12,
        old_routine.r13,
        old_routine.r14,
        old_routine.r15,
        old_routine.x87_control,
        old_routine.reserved,
        old_routine.mxcsr_control,
        old_routine.padding,
        old_pointers.scheduler,
        old_pointers.routine,
        new_pointers.scheduler,
        new_pointers.routine,
    ));
    report("old", &mut old_samples);
    report("new", &mut new_samples);
    report("paired_new_minus_old", &mut paired_differences);
    Ok(())
}

fn measure_old(scheduler: *mut LegacyMachineContext, routine: *mut LegacyMachineContext) -> f64 {
    let started = Instant::now();
    for _ in 0..ROUNDS {
        // SAFETY: Contexts/stacks are live and the trampoline returns ownership.
        unsafe {
            oxido_test_legacy_context_switch(scheduler.cast(), routine.cast());
        }
        black_box(scheduler);
    }
    started.elapsed().as_secs_f64() * 1_000_000_000.0 / (ROUNDS as f64 * 2.0)
}

fn measure_new(scheduler: *mut MachineContext, routine: *mut MachineContext) -> f64 {
    let started = Instant::now();
    for _ in 0..ROUNDS {
        // SAFETY: Contexts/stacks are live and the trampoline returns ownership.
        unsafe {
            super::arch::x86_64::switch(scheduler, routine.cast_const());
        }
        black_box(scheduler);
    }
    started.elapsed().as_secs_f64() * 1_000_000_000.0 / (ROUNDS as f64 * 2.0)
}

fn prefault_stack(
    bounds: crate::stack::StackBounds,
    page_size: usize,
) -> Result<(), Box<dyn Error>> {
    let mut address = bounds.usable_start;
    while address < bounds.usable_end {
        // SAFETY: The stack is owned here and no context is executing on it.
        unsafe { (address as *mut u8).write_volatile(0) };
        address = address
            .checked_add(page_size)
            .ok_or("stack prefault address overflow")?;
    }
    Ok(())
}

fn report(label: &str, samples: &mut [f64]) {
    samples.sort_by(f64::total_cmp);
    let p50 = stats::median(samples);
    let min = samples[0];
    let max = samples[samples.len() - 1];
    println!(
        "context_switch_ab summary={label} unit=ns_per_transfer p50={p50:.3} min={min:.3} max={max:.3} range={:.3}",
        max - min
    );
}
