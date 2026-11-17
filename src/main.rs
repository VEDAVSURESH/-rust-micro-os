#![no_std]
#![no_main]

use core::panic::PanicInfo;

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

    // Demonstrate formatted printing with variables
    let version_major = 0;
    let version_minor = 1;
    let version_patch = 0;
    println!("Version: {}.{}.{}", version_major, version_minor, version_patch);
    println!("Base address: 0x{:x}", 0xb8000usize);

    // Demonstrate automatic scrolling: write 30 numbered lines to trigger VGA scrolling
    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightGreen, vga_buffer::Color::Black);
    println!("[TEST] Demonstrating automatic VGA line scrolling:");
    for i in 1..=30 {
        println!("  -> Scroll test line {:02}/30", i);
        serial_println!("[SCROLL TEST] Line {:02}/30 printed.", i);
    }

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::Yellow, vga_buffer::Color::Black);
    println!("[OK] Console and Serial drivers operational!");
    serial_println!("[OK] Console and Serial drivers operational!");

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
