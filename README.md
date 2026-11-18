# JainOS — A Rust-Based Educational Operating System

![Rust](https://img.shields.io/badge/Language-Rust%20Nightly-orange.svg)
![Target](https://img.shields.io/badge/Target-x86__64--unknown--none-blue.svg)
![License](https://img.shields.io/badge/License-MIT%2FApache--2.0-green.svg)
![Image Size](https://img.shields.io/badge/Image%20Size-0.24%20MB%20%2F%20100%20MB-brightgreen.svg)

**JainOS** is a type-safe, freestanding x86_64 micro-operating system written in pure `#![no_std]` Rust for educational and systems-programming research.

JainOS enforces a **strict zero-simulation policy**: every driver, interrupt handler, CPU structure, memory manager, scheduler, and network protocol interacts directly with real hardware registers, memory-mapped I/O (MMIO), port I/O, or QEMU-emulated hardware devices.

---

## Architecture Overview

JainOS operates in 64-bit Long Mode on bare metal without an underlying OS runtime or standard C library.

```
+-------------------------------------------------------------------------+
|                              JainOS Kernel                              |
+-------------------------------------------------------------------------+
|  Applications & Demonstrations (Shell, Benchmarks, Network Diagnostics) |
+-------------------------------------------------------------------------+
|  Network Stack (DHCP, DNS, TCP, UDP, IPv4, ICMP, ARP, Ethernet)        |
+-------------------------------------------------------------------------+
|  Multitasking Engine (TCB, Ready Queue, Round-Robin Preemptive Sched)   |
+-------------------------------------------------------------------------+
|  Memory Subsystem (4-Level Paging, Frame Allocator, Kernel Heap)        |
+-------------------------------------------------------------------------+
|  Core Services:                                                         |
|  - Structured Logger (TRACE, DEBUG, INFO, WARN, ERROR)                  |
|  - Hardware Timer (8254 PIT @ 100 Hz, Atomic Ticks, HLT Sleep Loop)     |
|  - Interrupt Controller (8259 PIC Remapped to Vectors 32..47)           |
|  - CPU Tables (GDT, TSS with Dedicated Double-Fault IST Stack, IDT)     |
|  - Synchronization (Interrupt-Safe TTAS Spinlock<T>)                    |
|  - Console Drivers (80x25 VGA MMIO @ 0xb8000, 16550 UART COM1 @ 0x3F8)  |
+-------------------------------------------------------------------------+
|                       Hardware / QEMU Emulator                          |
+-------------------------------------------------------------------------+
```

### Physical Memory Layout
| Address Range | Description | Usage |
| :--- | :--- | :--- |
| `0x0000_0000 - 0x000F_FFFF` | Lower 1 MB (Real Mode / BIOS Area) | Reserved by BIOS & Hardware |
| `0x000B_8000 - 0x000B_8F9F` | VGA Text Buffer (80×25 characters) | Memory-Mapped I/O for Console Display |
| `0x0020_0000 - ...` | 2 MB+ (Static Relocation Base) | JainOS Kernel Code (`.text`), Read-Only Data (`.rodata`), Data (`.data`, `.bss`) |
| Top of Allocated Stack | Dedicated Double-Fault Stack (20 KiB) | Interrupt Stack Table (`IST[0]`) in TSS |

---

## Technologies Used

- **Language**: Rust (Nightly Channel, `edition = "2021"`)
  - Features: `#![no_std]`, `#![no_main]`, `#![feature(abi_x86_interrupt)]`
  - Core libraries: `core`, `compiler_builtins` (with `compiler-builtins-mem` for raw memory primitives)
- **Target Specification**: Built-in `x86_64-unknown-none` (soft-float, disabled redzone, static relocation)
- **Bootloader**: `bootloader` (v0.9.35)
- **Hardware Abstraction**: `x86_64` (v0.14.12, stripped of unstable step traits for rustc 1.100 compatibility)
- **Emulation & Virtualization**: QEMU (`qemu-system-x86_64` v6.2.0)
- **Tooling & Debugging**: Cargo, `cargo-bootimage`, GDB, LLVM `llvm-tools`, `rust-lld`, `readelf`, `objdump`
- **Version Control**: Git (hosted on GitHub)

---

## Completed Milestones (1–4)

### Milestone 1: Freestanding Bootable Kernel (`f2bc947`)
- Established freestanding bare-metal environment (`#![no_std]`, `#![no_main]`).
- Implemented kernel entry point `_start() -> !` following System V AMD64 ABI.
- Integrated `bootloader` v0.9 to transition CPU from Real Mode $\rightarrow$ 32-bit Protected Mode $\rightarrow$ 64-bit Long Mode with initial page tables.
- Configured automated image building via `bootimage runner`.

### Milestone 2: Kernel Console & Serial Output (`8cdb4ca`)
- **VGA Text Buffer Driver (`src/vga_buffer.rs`)**:
  - Memory-Mapped I/O (MMIO) at `0xb8000`.
  - Color attributes (16 foreground & background colors).
  - Volatile reads and writes (`core::ptr::write_volatile`) to prevent compiler optimization.
  - Automatic 80-column line wrapping and 25-row screen scrolling.
  - Standard `print!` and `println!` macros.
- **16550 UART Serial Driver (`src/serial.rs`)**:
  - Hardware port I/O via inline assembly (`inb` and `outb`).
  - Configured COM1 (`0x3F8`) at 115,200 baud, 8N1 (8 data bits, no parity, 1 stop bit).
  - Transmitter status polling (`LSR` bit 5) and automatic `\r\n` line ending translation.
  - `serial_print!` and `serial_println!` macros for direct host terminal output.
- **Enhanced Panic Handler (`src/main.rs`)**:
  - Catches kernel panics and prints diagnostic banners, file names, and line numbers to both VGA and serial consoles in bright red.

### Milestone 3: Core CPU Structures — GDT, TSS & IDT (`d8567ff`)
- **Global Descriptor Table (GDT) & Task State Segment (TSS) (`src/gdt.rs`)**:
  - Configured 64-bit kernel code segment (selector `0x08`).
  - Configured TSS descriptor (selector `0x10`) loaded via the CPU's Task Register (`ltr`).
  - Allocated a dedicated 20 KiB stack assigned to Interrupt Stack Table entry 0 (`IST[0]`).
- **Interrupt Descriptor Table (IDT) (`src/interrupts.rs`)**:
  - Loaded 256-entry table via CPU's `lidt` instruction.
  - Implemented CPU exception handlers using `extern "x86-interrupt"` ABI.
  - Double Fault handler configured with `set_stack_index(0)` to switch stacks on stack overflow, completely preventing fatal Triple Fault reboots.
  - Handlers for Breakpoint (`int3`), Page Fault (reading `CR2`), and General Protection Fault.
- **Interrupt-Safe Spinlock (`src/sync.rs`)**:
  - Atomic `AtomicBool` Test-and-Test-and-Set (TTAS) spinlock with `Acquire`/`Release` memory ordering.
  - Automatically records whether CPU interrupts were enabled, disables interrupts (`cli`) during lock acquisition, and restores the prior interrupt flag on guard drop.

### Milestone 4: Hardware Timer & Structured Kernel Logging (`de2ab75`)
- **8259 PIC Remapping (`src/pic.rs`)**:
  - Remapped legacy IBM PC PIC from vectors `8..15` to vectors `32..47` (Master: `32..39`, Slave: `40..47`), resolving collisions with CPU exceptions.
  - Unmasked only IRQ0 (Timer) while keeping unhandled lines safely masked.
  - Implemented correct End Of Interrupt (EOI) acknowledgement (`0x20` to command port).
- **8254 Programmable Interval Timer (PIT) (`src/timer.rs`)**:
  - Configured Channel 0 in Mode 3 (Square-wave generator) using command byte `0x36`.
  - Programmed divisor to `11,931` ($1,193,182 / 100$), generating exactly 100 hardware interrupts per second (~10 ms per tick).
- **Non-Blocking Timer ISR**:
  - Increments `static TICKS: AtomicU64` with `Ordering::Relaxed` and acknowledges PIC.
  - Zero blocking locks or console prints inside the ISR, completely eliminating interrupt deadlocks.
- **Structured Kernel Logger (`src/logger.rs`)**:
  - 5 severity levels: `TRACE` (DarkGray), `DEBUG` (LightCyan), `INFO` (LightGreen), `WARN` (Yellow), and `ERROR` (LightRed).
  - Formatted uptime timestamps: `[{seconds}.{centis}s]`.
  - Macros: `log_trace!`, `log_debug!`, `log_info!`, `log_warn!`, `log_error!`.
- **Power-Saving Idle Loop**:
  - Main loop monitors tick milestones and executes `x86_64::instructions::hlt()`, suspending the CPU until the next hardware timer tick wakes it.

---

## 13-Milestone Project Roadmap

```
+---------------------------------------------------------------------------------------------------+
|                                JainOS Full Development Roadmap                                    |
+----+----------------------------------+-------------------------------------------------+---------+
| #  | Milestone Title                  | Core Deliverables                               | Status  |
+----+----------------------------------+-------------------------------------------------+---------+
| 01 | Bootable Rust Kernel             | Freestanding #![no_std] binary, bootloader, QEMU | DONE    |
| 02 | Kernel Console Output            | 80x25 VGA driver, 16550 UART serial, panic      | DONE    |
| 03 | Core CPU Structures (GDT & IDT)  | GDT, TSS with IST double-fault stack, IDT       | DONE    |
| 04 | Hardware Timer & Kernel Logging  | 8259 PIC remap, 8254 PIT (100 Hz), 5 log levels | DONE    |
| 05 | Memory Allocator                 | Frame allocator, 4-level paging, kernel heap    | NEXT    |
| 06 | Basic Task Representation        | Task Control Block (TCB), stacks, context switch| Queued  |
| 07 | Ready Queue & Scheduling         | FIFO Ready queue, cooperative multitasking      | Queued  |
| 08 | Preemptive Timer Scheduling      | PIT timer-driven preemptive time-slicing        | Queued  |
| 09 | Basic Synchronization            | Semaphores, Mutexes, Condition Variables        | Queued  |
| 10 | PCI Bus & Network NIC Driver     | PCI scan, Intel 82540EM (e1000) MMIO & DMA rings| Queued  |
| 11 | Core Network Stack               | Ethernet framing, ARP cache, IPv4, ICMP, UDP    | Queued  |
| 12 | Live Internet Connectivity       | DHCP lease, DNS resolution, TCP stream sockets  | Queued  |
| 13 | Integrated Demo & Shell          | Interactive shell, task metrics, live internet  | Queued  |
+----+----------------------------------+-------------------------------------------------+---------+
```

---

## Future Genuine Internet Connectivity Objectives

JainOS will achieve **genuine, unsimulated internet connectivity** through QEMU's virtual network interface (`-netdev user,id=net0 -device e1000,netdev=net0`):

1. **PCI Bus Enumeration (`src/pci.rs`)**:
   - Query ports `0xCF8`/`0xCFC` to scan buses `0..255`, devices `0..31`, and functions `0..7`.
   - Discover the **Intel 82540EM** network interface card (`Vendor ID: 0x8086`, `Device ID: 0x100E`).
   - Extract BAR0 MMIO physical address and enable PCI Bus Mastering for DMA.
2. **Intel E1000 Driver (`src/net/e1000.rs`)**:
   - Read hardware MAC address from EEPROM.
   - Allocate circular DMA rings for Transmit (TX) and Receive (RX) descriptors.
   - Handle packet reception interrupts.
3. **Network Protocols**:
   - **Layer 2**: Ethernet frame serializer/deserializer.
   - **Layer 2.5**: ARP (Address Resolution Protocol) cache and request/reply engine.
   - **Layer 3**: IPv4 packet routing and Internet Checksum validation; ICMP Ping client.
   - **Layer 4**: UDP datagrams; TCP stream state machine (`SYN`, `SYN-ACK`, `ACK`, data transfer, `FIN`).
4. **Network Services & Internet Access**:
   - **DHCP**: Request and bind IP configuration from QEMU's SLIRP DHCP service (`IP: 10.0.2.15`, `Gateway: 10.0.2.2`, `DNS: 10.0.2.3`).
   - **DNS**: Resolve public hostnames via UDP queries to `10.0.2.3:53`.
   - **Live Internet Test**: Open a TCP socket to an external web server and fetch HTTP payload data through the host network connection.

---

## Building and Running JainOS

### Prerequisites
- Ubuntu 22.04 LTS (native or through WSL2 on Windows 11).
- Rust Nightly (`rustup default nightly`).
- Bare-metal target and standard library source:
  ```bash
  rustup target add x86_64-unknown-none --toolchain nightly
  rustup component add rust-src llvm-tools --toolchain nightly
  ```
- Bootimage utility:
  ```bash
  cargo install bootimage
  ```
- QEMU emulator:
  ```bash
  sudo apt install qemu-system-x86
  ```

### Build the Kernel & Bootable Image
```bash
cd /home/vedav/rust-micro-os
cargo build
cargo bootimage
```
The bootable raw disk image is created at:
```text
target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin
```

### Run in QEMU
To launch JainOS with VGA display and serial redirected to your terminal:
```bash
cargo run
```
Or run directly via QEMU:
```bash
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin \
    -serial stdio
```

To run in headless mode (terminal-only output):
```bash
qemu-system-x86_64 \
    -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-rust-micro-os.bin \
    -serial stdio \
    -display none
```

---

## Repository Information

- **Project Name**: JainOS — A Rust-Based Educational Operating System
- **Repository**: [https://github.com/VEDAVSURESH/-rust-micro-os.git](https://github.com/VEDAVSURESH/-rust-micro-os.git)
- **Author**: Vedav Suresh
- **Primary Branch**: `main`
- **Image Size Compliance**: Strict $< 100\text{ MB}$ budget (Current: `~0.24 MB`, utilizing $< 0.25\%$).
