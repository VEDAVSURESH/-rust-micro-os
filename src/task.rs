//! Cooperative Task Switching & Context Management for JainOS.
//!
//! This module implements genuine x86_64 low-level cooperative context switching
//! adhering to the System V AMD64 ABI.
//!
//! It provides:
//! - `switch_context(old_rsp: *mut usize, new_rsp: usize)` implemented in assembly.
//! - Preserves callee-saved registers (RBP, RBX, R12..R15) and stack pointers (RSP).
//! - Mathematically verified 16-byte stack alignment.
//! - Verification test demonstrating independent stacks and state retention.

use core::arch::global_asm;
use crate::log_info;

// Global assembly implementation of switch_context
global_asm!(
    r#"
    .global switch_context
    .type switch_context, @function
    switch_context:
        # Save callee-saved registers onto the current task's stack
        push r15
        push r14
        push r13
        push r12
        push rbx
        push rbp

        # Save current RSP through old_rsp (RDI contains old_rsp)
        mov [rdi], rsp

        # Load new task's RSP (RSI contains new_rsp)
        mov rsp, rsi

        # Restore callee-saved registers from the new task's stack
        pop rbp
        pop rbx
        pop r12
        pop r13
        pop r14
        pop r15

        # Return into the resumed execution flow of the new task
        ret
    "#
);

extern "C" {
    /// Low-level assembly routine performing a cooperative context switch.
    ///
    /// # Safety
    /// - `old_rsp` must be a valid, writable pointer to store the current stack pointer.
    /// - `new_rsp` must be a valid stack pointer for an initialized or suspended task stack.
    pub fn switch_context(old_rsp: *mut usize, new_rsp: usize);
}

/// Stack size for cooperative test tasks (16 KiB = 4 pages).
pub const TASK_STACK_SIZE: usize = 4096 * 4;

/// Static storage for Task 1's dedicated stack.
#[repr(align(16))]
pub struct TaskStack(pub [u8; TASK_STACK_SIZE]);

static mut TASK1_STACK: TaskStack = TaskStack([0; TASK_STACK_SIZE]);
static mut TASK2_STACK: TaskStack = TaskStack([0; TASK_STACK_SIZE]);

static mut MAIN_RSP: usize = 0;
static mut TASK1_RSP: usize = 0;
static mut TASK2_RSP: usize = 0;

static mut TASK1_FINAL: u64 = 0;
static mut TASK2_FINAL: u64 = 0;

/// Initializes a task stack to the exact layout expected by `switch_context`.
///
/// # Initial Stack Layout (System V AMD64 ABI):
///
/// Stack grows downwards from `stack_top` (16-byte aligned).
///
/// `[stack_top - 0x08]`: 0 (dummy return address to terminate stack traces)
/// `[stack_top - 0x10]`: `entry_fn` function pointer (popped into RIP by `ret`)
/// `[stack_top - 0x18]`: r15 = 0
/// `[stack_top - 0x20]`: r14 = 0
/// `[stack_top - 0x28]`: r13 = 0
/// `[stack_top - 0x30]`: r12 = 0
/// `[stack_top - 0x38]`: rbx = 0
/// `[stack_top - 0x40]`: rbp = 0
///
/// Returns `initial_rsp = stack_top - 0x40`.
/// On entry to `entry_fn`, `RSP % 16 == 8`, identical to being called via `call`.
pub unsafe fn create_initial_stack(stack: *mut TaskStack, entry_fn: extern "C" fn()) -> usize {
    let stack_bottom = (*stack).0.as_mut_ptr() as usize;
    let stack_top = stack_bottom + TASK_STACK_SIZE;
    let stack_top_aligned = stack_top & !0xF;

    let stack_ptr = stack_top_aligned as *mut usize;

    // [stack_top - 0x08]: Dummy return address
    stack_ptr.offset(-1).write(0);
    // [stack_top - 0x10]: Function pointer for ret
    stack_ptr.offset(-2).write(entry_fn as usize);
    // [stack_top - 0x18] .. [stack_top - 0x40]: Callee-saved registers (R15..RBP)
    stack_ptr.offset(-3).write(0); // r15
    stack_ptr.offset(-4).write(0); // r14
    stack_ptr.offset(-5).write(0); // r13
    stack_ptr.offset(-6).write(0); // r12
    stack_ptr.offset(-7).write(0); // rbx
    stack_ptr.offset(-8).write(0); // rbp

    stack_top_aligned - 64
}

/// Entry point for Task 1 in the cooperative switching test.
extern "C" fn task1_entry() {
    let mut task1_counter: u64 = 100;

    for iter in 1..=3 {
        log_info!(
            "  [Task 1] Iteration {} | local counter = {} | yielding to Task 2...",
            iter,
            task1_counter
        );
        task1_counter += 10;

        // Yield to Task 2
        unsafe {
            switch_context(&raw mut TASK1_RSP, TASK2_RSP);
        }

        log_info!(
            "  [Task 1] Resumed! Local counter retained value = {}",
            task1_counter
        );
    }

    // Yield to Task 2 so it can complete its final iteration
    unsafe {
        switch_context(&raw mut TASK1_RSP, TASK2_RSP);
    }

    unsafe {
        core::ptr::write_volatile(&raw mut TASK1_FINAL, task1_counter);
        // All iterations complete; return control to Main Kernel context
        log_info!("  [Task 1] Completed all iterations! Yielding back to Main Kernel...");
        switch_context(&raw mut TASK1_RSP, MAIN_RSP);
    }

    loop {
        core::hint::spin_loop();
    }
}

/// Entry point for Task 2 in the cooperative switching test.
extern "C" fn task2_entry() {
    let mut task2_counter: u64 = 2000;

    for iter in 1..=3 {
        log_info!(
            "  [Task 2] Iteration {} | local counter = {} | yielding to Task 1...",
            iter,
            task2_counter
        );
        task2_counter += 50;

        // Yield to Task 1
        unsafe {
            switch_context(&raw mut TASK2_RSP, TASK1_RSP);
        }

        log_info!(
            "  [Task 2] Resumed! Local counter retained value = {}",
            task2_counter
        );
    }

    unsafe {
        core::ptr::write_volatile(&raw mut TASK2_FINAL, task2_counter);
        // All iterations complete; yield to Task 1 so it can finalize and return to Main
        log_info!("  [Task 2] Completed all iterations! Yielding to Task 1 for final return...");
        switch_context(&raw mut TASK2_RSP, TASK1_RSP);
    }

    loop {
        core::hint::spin_loop();
    }
}

/// Runs a verification test of genuine cooperative context switching.
///
/// Demonstrates:
/// 1. Main Kernel context yields to Task 1.
/// 2. Task 1 yields to Task 2 across 3 round-trip iterations.
/// 3. Both tasks maintain distinct local stack variables (`task1_counter`, `task2_counter`)
///    across yields without corruption.
/// 4. Task 1 and Task 2 complete and return cleanly to the Main Kernel context.
pub fn test_cooperative_context_switching() {
    log_info!("Initializing cooperative task switching test (Task 1 <-> Task 2)...");

    let final1: u64;
    let final2: u64;

    unsafe {
        TASK1_FINAL = 0;
        TASK2_FINAL = 0;
        TASK1_RSP = create_initial_stack(&raw mut TASK1_STACK, task1_entry);
        TASK2_RSP = create_initial_stack(&raw mut TASK2_STACK, task2_entry);

        // Switch from Main Kernel context to Task 1
        switch_context(&raw mut MAIN_RSP, TASK1_RSP);

        final1 = core::ptr::read_volatile(&raw const TASK1_FINAL);
        final2 = core::ptr::read_volatile(&raw const TASK2_FINAL);
        // Verification after both tasks finish and return to Main
        assert_eq!(
            final1, 130,
            "Task 1 local variable corrupted across yields"
        );
        assert_eq!(
            final2, 2150,
            "Task 2 local variable corrupted across yields"
        );
    }

    log_info!(
        "Cooperative context switching test PASSED (Task 1 final={}, Task 2 final={}).",
        final1,
        final2
    );
}
