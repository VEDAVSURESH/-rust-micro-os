#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

pub mod gdt;
pub mod interrupts;
pub mod logger;
pub mod memory;
pub mod pic;
pub mod serial;
pub mod sync;
pub mod timer;
pub mod vga_buffer;

bootloader::entry_point!(kernel_main);

/// Kernel entry point called by the bootloader.
fn kernel_main(boot_info: &'static bootloader::BootInfo) -> ! {
    // 1. Initial banner on serial and VGA
    serial_println!("========================================");
    serial_println!("      JainOS Kernel v0.1.0              ");
    serial_println!("  A Rust-Based Educational Micro-OS     ");
    serial_println!("========================================");
    serial_println!("[SERIAL] COM1 UART 16550 initialized at 115,200 baud.");

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightCyan, vga_buffer::Color::Black);
    println!("========================================");
    println!("      JainOS Kernel v0.1.0              ");
    println!("  A Rust-Based Educational Micro-OS     ");
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
    log_info!("JainOS Kernel v0.1.0 boot complete.");
    log_info!("GDT, TSS, and IDT configured successfully.");
    log_info!("8259 PIC remapped (Master=32, Slave=40). IRQ0 unmasked.");
    log_info!("8254 PIT Channel 0 running at 100 Hz (10 ms per tick).");
    log_info!("Hardware interrupts enabled (CPU IF flag active).");

    // 8. Demonstrate all kernel log levels
    log_trace!("Trace: Hardware timers and interrupt controller online.");
    log_debug!("Debug: PIT divisor = 11931, PIC Master mask = 0xFE.");
    log_warn!("Warn: Legacy 8259 PIC in use (APIC recommended for SMP).");
    log_error!("Error: Non-fatal test error log format verification.");

    // 9. Milestone 5 (Stage 1): Verify unaligned boundary safety and partial frame exclusion
    memory::test_unaligned_boundary_safety();
    log_info!("Unaligned boundary safety test suite PASSED (8/8 boundary cases verified).");

    let mem_info = match memory::analyze_memory_map(&boot_info.memory_map) {
        Ok(info) => info,
        Err(e) => {
            log_error!(
                "CRITICAL: Memory map analysis failed due to arithmetic overflow: {:?}",
                e
            );
            panic!("Memory map analysis arithmetic overflow");
        }
    };
    log_info!(
        "Memory map: {} total regions ({} usable).",
        mem_info.total_regions,
        mem_info.usable_regions
    );
    log_info!(
        "Safe usable physical memory: {} KiB ({} frames, ~{} MiB).",
        mem_info.total_usable_bytes / 1024,
        mem_info.total_usable_frames,
        mem_info.total_usable_bytes / (1024 * 1024)
    );

    let mut frame_allocator = unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    log_info!("Starting physical frame allocation test (requesting 5 frames)...");
    let mut test_frames = [None; 5];
    for i in 0..5 {
        use x86_64::structures::paging::FrameAllocator;
        let frame = frame_allocator.allocate_frame();
        match frame {
            Some(f) => {
                let addr = f.start_address().as_u64();
                // 1. Verify 4 KiB alignment
                assert_eq!(addr % 4096, 0, "Frame is not 4 KiB aligned");
                // 2. Verify frame zero was not returned
                assert_ne!(addr, 0, "Frame zero was allocated");
                // 3. Verify frame is fully within a usable memory region
                assert!(
                    memory::is_frame_in_usable_region(&boot_info.memory_map, f),
                    "Frame is not in usable region"
                );
                log_info!(
                    "  Allocated frame [{}]: {:#010x} (4 KiB aligned, verified usable)",
                    i,
                    addr
                );
                test_frames[i] = Some(f);
            }
            None => {
                log_error!("Failed to allocate physical frame [{}]", i);
                panic!("Physical frame allocation failed");
            }
        }
    }

    // 4. Verify all 5 allocated frames are distinct (no duplicates)
    for i in 0..5 {
        for j in (i + 1)..5 {
            assert_ne!(test_frames[i], test_frames[j], "Duplicate frame allocated");
        }
    }
    log_info!("Physical frame allocation test PASSED (5 distinct usable frames verified).");

    // 10. Normal kernel execution loop
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
