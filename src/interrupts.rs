//! Interrupt Descriptor Table (IDT) and CPU Exception/Interrupt Handlers.
//!
//! The IDT is an array of 256 descriptor entries telling the CPU what code
//! to execute when a hardware interrupt or software exception fires:
//! - Vectors 0..=31: CPU Exceptions (Breakpoint, Double Fault, Page Fault, GPF)
//! - Vectors 32..=47: Hardware IRQs remapped from 8259 PIC (Vector 32 = IRQ 0 Timer)

use core::cell::UnsafeCell;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use crate::{println, serial_println};
use crate::gdt;
use crate::pic;

/// Wrapper providing thread-safe interior mutability for static IDT storage.
struct IdtStorage(UnsafeCell<InterruptDescriptorTable>);
unsafe impl Sync for IdtStorage {}

static IDT: IdtStorage = IdtStorage(UnsafeCell::new(InterruptDescriptorTable::new()));

/// Initializes the Interrupt Descriptor Table and loads it into the CPU's IDTR register.
pub fn init_idt() {
    // Safety: Only called once during single-threaded kernel boot.
    let idt = unsafe { &mut *IDT.0.get() };

    // Exception 3: Breakpoint (Trap)
    idt.breakpoint.set_handler_fn(breakpoint_handler);

    // Exception 8: Double Fault (Abort)
    // Configured to switch to the dedicated IST stack to prevent triple faults on stack overflow.
    let double_fault = idt.double_fault.set_handler_fn(double_fault_handler);
    unsafe {
        double_fault.set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    }

    // Exception 14: Page Fault (Fault)
    idt.page_fault.set_handler_fn(page_fault_handler);

    // Exception 13: General Protection Fault (Fault)
    idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);

    // Hardware IRQ0: Timer Interrupt (Vector 32)
    idt[pic::PIC_1_OFFSET as usize].set_handler_fn(timer_interrupt_handler);

    // Hardware IRQ1: Keyboard Interrupt (Vector 33)
    idt[(pic::PIC_1_OFFSET + 1) as usize].set_handler_fn(keyboard_interrupt_handler);

    // Hardware IRQ12: PS/2 Mouse Interrupt (Vector 44)
    idt[(pic::PIC_2_OFFSET + 4) as usize].set_handler_fn(mouse_interrupt_handler);

    // Load IDT into CPU IDTR register
    idt.load();
}

/// Handler for Hardware Timer Interrupt (IRQ 0 / Vector 32).
///
/// SAFETY:
/// - Must NOT perform blocking prints or acquire print locks.
/// - Increments the global atomic tick counter.
/// - Acknowledges the 8259 PIC by sending End Of Interrupt (EOI).
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::timer::TICKS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    unsafe {
        pic::PICS.lock().notify_end_of_interrupt(pic::PIC_1_OFFSET);
    }
}

/// Handler for Hardware Keyboard Interrupt (IRQ 1 / Vector 33).
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::keyboard::handle_interrupt();

    unsafe {
        pic::PICS.lock().notify_end_of_interrupt(pic::PIC_1_OFFSET + 1);
    }
}

/// Handler for Hardware PS/2 Mouse Interrupt (IRQ 12 / Vector 44).
extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    crate::mouse::handle_interrupt();

    unsafe {
        pic::PICS.lock().notify_end_of_interrupt(pic::PIC_2_OFFSET + 4);
    }
}

/// Handler for Breakpoint exceptions (INT 3).
///
/// This is a harmless trap used by debuggers to inspect program state.
/// Execution resumes seamlessly after the INT 3 instruction.
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("[EXCEPTION] Breakpoint Exception triggered!");
    serial_println!("{:#?}", stack_frame);

    println!("[EXCEPTION] Breakpoint Exception triggered!");
    println!("{:#?}", stack_frame);
}

/// Handler for Double Fault exceptions.
///
/// Occurs when an unhandled exception triggers a secondary exception.
/// Executed on the dedicated IST stack.
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    serial_println!("        FATAL: DOUBLE FAULT!            ");
    serial_println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    serial_println!("{:#?}", stack_frame);

    println!("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    println!("        FATAL: DOUBLE FAULT!            ");
    println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    println!("{:#?}", stack_frame);

    loop {
        core::hint::spin_loop();
    }
}

/// Handler for Page Fault exceptions.
///
/// Occurs on invalid virtual memory accesses (e.g. unmapped page or protection violation).
/// The accessed linear address is stored in the CR2 register.
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    let accessed_address = Cr2::read();

    serial_println!("\n[EXCEPTION] Page Fault!");
    serial_println!("Accessed Address: {:?}", accessed_address);
    serial_println!("Error Code: {:?}", error_code);
    serial_println!("{:#?}", stack_frame);

    println!("\n[EXCEPTION] Page Fault!");
    println!("Accessed Address: {:?}", accessed_address);
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);

    loop {
        core::hint::spin_loop();
    }
}

/// Handler for General Protection Faults (GPF).
extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_println!("\n[EXCEPTION] General Protection Fault!");
    serial_println!("Error Code: {:#x}", error_code);
    serial_println!("{:#?}", stack_frame);

    println!("\n[EXCEPTION] General Protection Fault!");
    println!("Error Code: {:#x}", error_code);
    println!("{:#?}", stack_frame);

    loop {
        core::hint::spin_loop();
    }
}
