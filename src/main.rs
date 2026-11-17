#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

pub mod gdt;
pub mod interrupts;
pub mod serial;
pub mod sync;
pub mod vga_buffer;

/// Kernel entry point called by the bootloader.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize & announce to serial port (WSL2 terminal output)
    serial_println!("========================================");
    serial_println!("      Rust Micro-OS Kernel v0.1.0       ");
    serial_println!("========================================");
    serial_println!("[SERIAL] COM1 UART 16550 initialized at 115,200 baud.");

    // Print banner to VGA text buffer
    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightCyan, vga_buffer::Color::Black);
    println!("========================================");
    println!("      Rust Micro-OS Kernel v0.1.0       ");
    println!("========================================");

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::White, vga_buffer::Color::Black);
    println!("Kernel console initialized.");
    println!("Screen: 80x25 VGA Text Buffer @ 0xb8000");

    // Initialize CPU structures: Global Descriptor Table & Task State Segment
    gdt::init();
    serial_println!("[GDT] Initialized with dedicated Double Fault IST stack.");
    println!("[GDT] Initialized with dedicated Double Fault stack.");

    // Initialize Interrupt Descriptor Table & CPU exception handlers
    interrupts::init_idt();
    serial_println!("[IDT] Initialized with exception handlers (Breakpoint, Double Fault, Page Fault, GPF).");
    println!("[IDT] Initialized with exception handlers.");

    // Safely verify CPU exception handling via a Breakpoint trap (INT 3)
    serial_println!("[TEST] Triggering software breakpoint exception (INT 3)...");
    x86_64::instructions::interrupts::int3();
    serial_println!("[OK] Successfully returned from breakpoint exception handler!");
    println!("[OK] Breakpoint exception handled; execution resumed!");

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightGreen, vga_buffer::Color::Black);
    println!("\n[OK] Core CPU structures operational!");
    serial_println!("[OK] Core CPU structures operational!");

    // To test panic output, uncomment the line below:
    // panic!("Verification of enhanced panic handler!");

    loop {
        core::hint::spin_loop();
    }
}

/// Enhanced panic handler that prints location and message to both VGA and Serial.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Print panic message in bright red on the VGA display
    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightRed, vga_buffer::Color::Black);
    println!("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    println!("             KERNEL PANIC!              ");
    println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    println!("{}", info);

    // Print panic message to serial console (WSL2 terminal)
    serial_println!("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    serial_println!("             KERNEL PANIC!              ");
    serial_println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    serial_println!("{}", info);

    loop {
        core::hint::spin_loop();
    }
}
