#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

pub mod gdt;
pub mod interrupts;
pub mod logger;
pub mod pic;
pub mod serial;
pub mod sync;
pub mod timer;
pub mod vga_buffer;

/// Kernel entry point called by the bootloader.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 1. Initial banner on serial and VGA
    serial_println!("========================================");
    serial_println!("      Rust Micro-OS Kernel v0.1.0       ");
    serial_println!("========================================");
    serial_println!("[SERIAL] COM1 UART 16550 initialized at 115,200 baud.");

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightCyan, vga_buffer::Color::Black);
    println!("========================================");
    println!("      Rust Micro-OS Kernel v0.1.0       ");
    println!("========================================");

    // 2. Initialize GDT & TSS (dedicated Double Fault stack)
    gdt::init();

    // 3. Initialize IDT (exceptions and timer vector)
    interrupts::init_idt();

    // 4. Remap 8259 PIC (Master to 32, Slave to 40) and unmask IRQ0
    unsafe {
        pic::PICS.lock().initialize();
    }

    // 5. Initialize 8254 PIT (Channel 0 at 100 Hz, ~10ms per tick)
    timer::init();

    // 6. Safely enable hardware CPU interrupts (sti)
    x86_64::instructions::interrupts::enable();

    // 7. Announce initialization through structured logger
    log_info!("Rust Micro-OS Kernel v0.1.0 boot complete.");
    log_info!("GDT, TSS, and IDT configured successfully.");
    log_info!("8259 PIC remapped (Master=32, Slave=40). IRQ0 unmasked.");
    log_info!("8254 PIT Channel 0 running at 100 Hz (10 ms per tick).");
    log_info!("Hardware interrupts enabled (CPU IF flag active).");

    // 8. Demonstrate all kernel log levels
    log_trace!("Trace: Hardware timers and interrupt controller online.");
    log_debug!("Debug: PIT divisor = 11931, PIC Master mask = 0xFE.");
    log_warn!("Warn: Legacy 8259 PIC in use (APIC recommended for SMP).");
    log_error!("Error: Non-fatal test error log format verification.");

    // 9. Normal kernel execution loop
    // Prints timer milestone updates safely from outside the interrupt handler.
    let mut last_reported_sec = 0;

    loop {
        let ticks = timer::current_ticks();
        let seconds = ticks / 100;

        if seconds > last_reported_sec {
            last_reported_sec = seconds;
            log_info!("Timer heartbeat: {}s elapsed (total ticks: {})", seconds, ticks);
        }

        // Put the CPU to sleep until the next hardware interrupt arrives.
        // Saves host CPU cycles and demonstrates real interrupt wakeups.
        x86_64::instructions::hlt();
    }
}

/// Enhanced panic handler that prints location and message to both VGA and Serial.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightRed, vga_buffer::Color::Black);
    println!("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    println!("             KERNEL PANIC!              ");
    println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    println!("{}", info);

    serial_println!("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    serial_println!("             KERNEL PANIC!              ");
    serial_println!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
    serial_println!("{}", info);

    loop {
        core::hint::spin_loop();
    }
}
