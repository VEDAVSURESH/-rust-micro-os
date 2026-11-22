//! Interactive Shell & Terminal for JainOS.
//!
//! Provides a responsive command-line interface with custom branding,
//! system inspection tools, memory reporting, hardware diagnostics,
//! and network utilities.

use alloc::string::String;
use alloc::vec::Vec;
use crate::keyboard;
use crate::{serial_print, serial_println, vga_buffer};

macro_rules! term_print {
    ($($arg:tt)*) => ({
        $crate::print!($($arg)*);
        $crate::serial_print!($($arg)*);
    });
}

macro_rules! term_println {
    () => ({
        $crate::println!();
        $crate::serial_println!();
    });
    ($($arg:tt)*) => ({
        $crate::println!($($arg)*);
        $crate::serial_println!($($arg)*);
    });
}

/// Banner displayed at terminal startup.
pub const WELCOME_BANNER: &str = "\
========================================
             WELCOME TO JAINOS
========================================
       JainOS — Rust Micro Operating System
       Developed by Vedav Suresh
       User: vedav
========================================";

/// System prompt.
pub const PROMPT: &str = "[JainOS] vedav@kernel:~$ ";

/// Initializes and prints the initial welcome screen.
pub fn init_terminal() {
    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::LightCyan, vga_buffer::Color::Black);
    term_println!("{}", WELCOME_BANNER);
    term_println!();

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::Yellow, vga_buffer::Color::Black);
    term_print!("{}", PROMPT);

    vga_buffer::WRITER
        .lock()
        .set_color(vga_buffer::Color::White, vga_buffer::Color::Black);
}

/// Executes a single command string.
pub fn execute_command(line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }

    let mut parts = trimmed.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let args: Vec<&str> = parts.collect();

    match cmd {
        "whoami" => {
            term_println!("vedav");
        }

        "help" => {
            term_println!("JainOS Built-in Commands:");
            term_println!("  whoami       - Display current active user identity");
            term_println!("  help         - Display this command reference");
            term_println!("  clear        - Clear the VGA terminal display");
            term_println!("  mem          - Display physical frame & heap allocator statistics");
            term_println!("  tasks        - Display cooperative scheduler tasks and CPU metrics");
            term_println!("  uname        - Print operating system & kernel architecture");
            term_println!("  version      - Display JainOS release details and credits");
            term_println!("  date         - Read current hardware RTC CMOS date and time");
            term_println!("  pci          - Enumerate and display PCI bus hardware devices");
            term_println!("  ifconfig     - Display network interface configuration (IP/MAC)");
            term_println!("  ping <host>  - Send ICMP echo requests to test network connectivity");
            term_println!("  curl <url>   - Fetch web page over HTTP/TCP");
            term_println!("  reboot       - Reboot the computer via 8042 controller");
        }

        "clear" => {
            vga_buffer::WRITER.lock().clear_screen();
            serial_print!("\x1B[2J\x1B[H");
        }

        "mem" => {
            cmd_mem();
        }

        "tasks" => {
            cmd_tasks();
        }

        "uname" => {
            term_println!("JainOS 0.1.0 x86_64 bare-metal microkernel");
        }

        "version" => {
            term_println!("JainOS v0.1.0 (Rust Micro Operating System)");
            term_println!("Author: Vedav Suresh (USN: 25BTRCL280, Section: CSE AIML-E)");
            term_println!("Target: x86_64-unknown-none (Freestanding Bare-Metal)");
            term_println!("Architecture: Pure #![no_std] Rust, Chained 8259 PIC, 8254 PIT, 16550 UART");
        }

        "date" => {
            let dt = crate::cmos::read_datetime();
            term_println!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                dt.year, dt.month, dt.day, dt.hours, dt.minutes, dt.seconds
            );
        }

        "pci" => {
            cmd_pci();
        }

        "ifconfig" => {
            cmd_ifconfig();
        }

        "ping" => {
            let target = args.first().copied().unwrap_or("10.0.2.2");
            cmd_ping(target);
        }

        "curl" => {
            let url = args.first().copied().unwrap_or("http://10.0.2.2/");
            cmd_curl(url);
        }

        "reboot" => {
            term_println!("Rebooting JainOS...");
            unsafe {
                // Pulse CPU reset line via PS/2 controller port 0x64
                crate::serial::outb(0x64, 0xFE);
            }
            loop {
                core::hint::spin_loop();
            }
        }

        unknown => {
            vga_buffer::WRITER
                .lock()
                .set_color(vga_buffer::Color::LightRed, vga_buffer::Color::Black);
            term_println!(
                "JainOS: command not found: '{}'. Type 'help' for available commands.",
                unknown
            );
            vga_buffer::WRITER
                .lock()
                .set_color(vga_buffer::Color::White, vga_buffer::Color::Black);
        }
    }
}

/// Handler for `mem` command.
fn cmd_mem() {
    term_println!("--- JainOS Memory Diagnostics ---");
    if let Some(info) = crate::memory::get_physical_memory_info() {
        term_println!(
            "Physical Memory Regions: {} total ({} usable)",
            info.total_regions, info.usable_regions
        );
        term_println!(
            "Safe Usable Physical RAM: {} KiB (~{} MiB, {} frames)",
            info.total_usable_bytes / 1024,
            info.total_usable_bytes / (1024 * 1024),
            info.total_usable_frames
        );
    } else {
        term_println!("Physical memory map: Not captured");
    }

    let stats = crate::allocator::ALLOCATOR.stats();
    term_println!("Kernel Heap Configuration:");
    term_println!("  Virtual Base:   {:#014x}", stats.heap_start);
    term_println!("  Total Heap:     {} B ({} KiB)", stats.heap_size, stats.heap_size / 1024);
    term_println!("  Allocated:      {} B", stats.total_allocated_bytes);
    term_println!("  Available Free: {} B", stats.total_free_bytes);
    term_println!("  Allocs / Frees: {} / {}", stats.alloc_count, stats.dealloc_count);
    term_println!("  Alloc Failures: {}", stats.alloc_failures);
}

/// Handler for `tasks` command.
fn cmd_tasks() {
    let sched = crate::task::SCHEDULER.lock();
    term_println!("--- JainOS Cooperative Task Scheduler ---");
    let current_str = match sched.current_task_id() {
        Some(id) => id.0,
        None => 0,
    };
    term_println!(
        "Active Tasks: {} | Current Running PID: {}",
        sched.task_count(),
        current_str
    );
    term_println!("  PID  | Name             | State      | Switches | Ticks");
    term_println!("-------+------------------+------------+----------+-------");

    sched.for_each_task(|tcb| {
        let state_str = match tcb.state() {
            crate::task::TaskState::Ready => "Ready     ",
            crate::task::TaskState::Running => "Running   ",
            crate::task::TaskState::Blocked => "Blocked   ",
            crate::task::TaskState::Terminated => "Terminated",
        };
        term_println!(
            "  {:<4} | {:<16} | {} | {:<8} | {}",
            tcb.id.0, tcb.name, state_str, tcb.switch_count, tcb.total_ticks
        );
    });
}

/// Handler for `pci` command.
fn cmd_pci() {
    term_println!("PCI Bus enumeration initializing...");
}

/// Handler for `ifconfig` command.
fn cmd_ifconfig() {
    term_println!("Network interface initializing...");
}

/// Handler for `ping` command.
fn cmd_ping(_target: &str) {
    term_println!("Network ping subsystem initializing...");
}

/// Handler for `curl` command.
fn cmd_curl(_url: &str) {
    term_println!("HTTP curl subsystem initializing...");
}

/// Interactive terminal loop task.
/// Continuously processes keyboard input and runs commands.
pub extern "C" fn terminal_task() {
    init_terminal();

    let mut line = String::new();

    loop {
        if let Some(c) = keyboard::read_char() {
            match c {
                '\n' => {
                    term_println!();
                    execute_command(&line);
                    line.clear();

                    vga_buffer::WRITER
                        .lock()
                        .set_color(vga_buffer::Color::Yellow, vga_buffer::Color::Black);
                    term_print!("{}", PROMPT);
                    vga_buffer::WRITER
                        .lock()
                        .set_color(vga_buffer::Color::White, vga_buffer::Color::Black);
                }
                '\x08' => {
                    if !line.is_empty() {
                        line.pop();
                        crate::print!("\x08");
                        serial_print!("\x08 \x08");
                    }
                }
                c => {
                    if line.len() < 120 && (c.is_ascii_graphic() || c == ' ') {
                        line.push(c);
                        term_print!("{}", c);
                    }
                }
            }
        }

        crate::task::yield_now();
        x86_64::instructions::hlt();
    }
}
