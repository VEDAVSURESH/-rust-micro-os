#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::panic::PanicInfo;

pub mod allocator;
pub mod bga;
pub mod cmos;
pub mod e1000;
pub mod gdt;
pub mod gui;
pub mod interrupts;
pub mod keyboard;
pub mod logger;
pub mod memory;
pub mod mouse;
pub mod net;
pub mod pci;
pub mod pic;
pub mod serial;
pub mod sync;
pub mod task;
pub mod terminal;
pub mod timer;
pub mod vga_buffer;

bootloader::entry_point!(kernel_main);

/// Kernel entry point called by the bootloader.
fn kernel_main(boot_info: &'static bootloader::BootInfo) -> ! {
    // 1. Initial banner on serial and VGA
    serial_println!("========================================");
    serial_println!("      JainOS Kernel v0.3.4              ");
    serial_println!("  A Rust-Based Educational Micro-OS     ");
    serial_println!("========================================");
    serial_println!("[SERIAL] COM1 UART 16550 initialized at 115,200 baud.");

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightCyan, vga_buffer::Color::Black);
    println!("========================================");
    println!("      JainOS Kernel v0.3.3              ");
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

    // 6. Initialize 8042 PS/2 Keyboard Controller & Auxiliary PS/2 Mouse
    keyboard::init();
    mouse::init();

    // 7. Safely enable hardware CPU interrupts (sti)
    x86_64::instructions::interrupts::enable();

    // 8. Announce initialization through structured logger
    log_info!("JainOS Kernel v0.3.1 boot complete.");
    log_info!("GDT, TSS, and IDT configured successfully.");
    log_info!("8259 PIC remapped (Master=32, Slave=40). IRQ0 unmasked.");
    log_info!("8254 PIT Channel 0 running at 100 Hz (10 ms per tick).");
    log_info!("8042 PS/2 keyboard controller initialized & translation enabled.");
    log_info!("Hardware interrupts enabled (CPU IF flag active).");

    // 9. Demonstrate all kernel log levels
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
    memory::set_physical_memory_info(mem_info);
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

    // 10. Milestone 5 (Stage 2): Paging & Kernel Heap Initialization
    let raw_offset = boot_info.physical_memory_offset;
    memory::set_phys_mem_offset(raw_offset);
    if raw_offset == 0 || raw_offset == u64::MAX {
        log_error!(
            "CRITICAL: Invalid physical memory offset from bootloader: {:#018x}",
            raw_offset
        );
        panic!("Invalid physical memory offset: bootloader map_physical_memory feature unpopulated");
    }
    let phys_mem_offset = match x86_64::VirtAddr::try_new(raw_offset) {
        Ok(addr) => addr,
        Err(_) => {
            log_error!(
                "CRITICAL: Physical memory offset is not canonical: {:#018x}",
                raw_offset
            );
            panic!("Non-canonical physical memory offset");
        }
    };
    let mut mapper = unsafe { memory::init_offset_page_table(phys_mem_offset) };

    match allocator::init_heap(&mut mapper, &mut frame_allocator) {
        Ok(heap_size) => {
            log_info!(
                "Kernel heap initialized at {:#x} (size: {} KiB).",
                allocator::HEAP_START,
                heap_size / 1024
            );
        }
        Err(e) => {
            log_error!("Failed to initialize kernel heap: {:?}", e);
            panic!("Heap initialization failed");
        }
    }

    let initial_stats = allocator::ALLOCATOR.stats();
    log_info!(
        "Initial heap stats: {} B total, {} B used, {} B free | Allocs: {}, Frees: {}, Fails: {}",
        initial_stats.heap_size,
        initial_stats.total_allocated_bytes,
        initial_stats.total_free_bytes,
        initial_stats.alloc_count,
        initial_stats.dealloc_count,
        initial_stats.alloc_failures
    );

    log_info!("Running kernel heap allocator test suite...");
    allocator::test_heap_allocations();
    log_info!("Kernel heap allocator test suite PASSED (14/14 test scenarios verified).");

    let post_stats = allocator::ALLOCATOR.stats();
    log_info!(
        "Post-test heap stats: {} B total, {} B used, {} B free | Allocs: {}, Frees: {}, Fails: {}",
        post_stats.heap_size,
        post_stats.total_allocated_bytes,
        post_stats.total_free_bytes,
        post_stats.alloc_count,
        post_stats.dealloc_count,
        post_stats.alloc_failures
    );
    assert_eq!(
        post_stats.total_allocated_bytes, 0,
        "Memory leak detected: active allocations remain after test suite"
    );
    memory::set_frame_allocator(frame_allocator);

    // 11. Milestone 6 (Stage 1): Cooperative Context Switching Test
    task::test_cooperative_context_switching();

    // 12. Milestone 6 (Stage 2B): Scheduler Structure & Ready Queue Test
    task::test_scheduler_structure();

    // 13. Milestone 6 (Stage 2C): Dynamic Task Creation Test
    task::test_dynamic_task_creation();

    // 14. Milestone 6 (Stage 2D): Scheduler-Driven Context Switching Demonstration
    task::test_scheduler_context_switching();

    // 15. PCI Hardware Bus Enumeration
    pci::enumerate_pci();
    log_info!("PCI bus enumeration complete: {} hardware devices detected.", pci::PCI_DEVICES.lock().len());

    // 16. Intel e1000 Gigabit Ethernet Initialization
    let has_net = e1000::init();

    // 17. Native Internet & DHCP Subsystem
    if has_net {
        net::init();
    }

    // 18. Launch Graphical Desktop or Interactive JainOS Terminal
    logger::set_vga_logging(false);
    vga_buffer::WRITER.lock().clear_screen();

    if bga::is_available() {
        match gui::init_desktop() {
            Ok(()) => {
                task::spawn("Desktop GUI", gui::desktop_task).expect("Failed to spawn Desktop GUI task");
                log_info!("JainOS Native Graphical Desktop launched successfully.");
            }
            Err(e) => {
                log_warn!("GUI initialization failed ({}), falling back to text terminal.", e);
                task::spawn("Terminal", terminal::terminal_task).expect("Failed to spawn Terminal task");
            }
        }
    } else {
        log_warn!("BGA graphics hardware unavailable, falling back to text terminal.");
        task::spawn("Terminal", terminal::terminal_task).expect("Failed to spawn Terminal task");
    }

    // 19. Normal kernel execution loop
    // Prints timer milestone updates safely from outside the interrupt handler to serial.
    let mut last_reported_sec = 0;

    loop {
        let ticks = timer::current_ticks();
        let seconds = ticks / 100;

        if seconds > last_reported_sec {
            last_reported_sec = seconds;
            log_info!("Timer heartbeat: {}s elapsed (total ticks: {})", seconds, ticks);
        }

        // Process incoming network packets
        if has_net {
            net::poll();
        }

        // Cooperatively yield CPU to ready background tasks (Terminal, Net, etc.)
        task::yield_now();

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

/// Handler called when a heap allocation cannot be satisfied by the global allocator.
#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("kernel heap allocation error: {:?}", layout);
}
