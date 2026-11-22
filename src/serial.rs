//! 16550 UART Serial Port Driver for COM1 (Port 0x3F8).
//!
//! Unlike memory-mapped I/O (like the VGA buffer), x86 processors have a distinct 16-bit
//! I/O port address space accessed through specialized `in` and `out` assembly instructions.
//! COM1 is the primary serial communication port, routed directly to the terminal stdout
//! by QEMU with `-serial stdio`.

use core::arch::asm;
use core::fmt;
use crate::sync::Spinlock;

/// Reads a single byte from a specified x86 16-bit I/O port.
///
/// # Safety
/// Reading from arbitrary hardware I/O ports can trigger unexpected hardware behavior.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // asm!: emits the x86 `in` instruction. Reads from DX (port) into AL (register).
    asm!(
        "in al, dx",
        out("al") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Writes a single byte to a specified x86 16-bit I/O port.
///
/// # Safety
/// Writing to arbitrary hardware I/O ports can alter hardware state.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    // asm!: emits the x86 `out` instruction. Writes from AL (register) to DX (port).
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Reads a 16-bit word from a specified x86 16-bit I/O port.
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    asm!(
        "in ax, dx",
        out("ax") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Writes a 16-bit word to a specified x86 16-bit I/O port.
#[inline]
pub unsafe fn outw(port: u16, value: u16) {
    asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Reads a 32-bit dword from a specified x86 16-bit I/O port.
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    asm!(
        "in eax, dx",
        out("eax") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Writes a 32-bit dword to a specified x86 16-bit I/O port.
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// 16550 UART driver abstraction.
pub struct SerialPort {
    port: u16,
    initialized: bool,
}

// Safety: Port access is synchronized via Spinlock<SerialPort>.
unsafe impl Send for SerialPort {}
unsafe impl Sync for SerialPort {}

impl SerialPort {
    /// Creates a serial port interface bound to the specified base I/O port.
    pub const fn new(port: u16) -> Self {
        Self {
            port,
            initialized: false,
        }
    }

    /// Initializes the 16550 UART hardware controller.
    ///
    /// Sets 115,200 baud rate, 8 data bits, 1 stop bit, no parity (8N1),
    /// enables the FIFO buffers, and sets auxiliary control lines.
    pub fn init(&mut self) {
        if self.initialized {
            return;
        }

        unsafe {
            // Port + 1: Interrupt Enable Register (IER) -> 0x00 disables interrupts
            outb(self.port + 1, 0x00);

            // Port + 3: Line Control Register (LCR) -> 0x80 enables DLAB (Divisor Latch Access Bit)
            outb(self.port + 3, 0x80);

            // Set baud rate divisor to 1 (115,200 baud):
            // Port + 0: Divisor byte low -> 0x01
            outb(self.port + 0, 0x01);
            // Port + 1: Divisor byte high -> 0x00
            outb(self.port + 1, 0x00);

            // Port + 3: 0x03 sets 8 bits, no parity, 1 stop bit (8N1), and clears DLAB
            outb(self.port + 3, 0x03);

            // Port + 2: FIFO Control Register (FCR) -> 0xC7 enables FIFO, clears queues, 14-byte threshold
            outb(self.port + 2, 0xC7);

            // Port + 4: Modem Control Register (MCR) -> 0x0B sets RTS/DSR and auxiliary output 2
            outb(self.port + 4, 0x0B);
        }

        self.initialized = true;
    }

    /// Sends a single raw byte through the serial line.
    /// Polls the Line Status Register (LSR, port + 5) until the transmitter holding register is empty.
    pub fn send_byte(&mut self, byte: u8) {
        if !self.initialized {
            self.init();
        }

        unsafe {
            // Bit 5 (0x20) in Line Status Register indicates Transmitter Holding Register Empty (THRE).
            // Spin until the transmitter buffer is ready to receive a new byte.
            while (inb(self.port + 5) & 0x20) == 0 {
                core::hint::spin_loop();
            }

            // Write the byte into the Data Register (port + 0) to transmit it.
            outb(self.port, byte);
        }
    }

    /// Sends a string through the serial line.
    /// Automatically converts `\n` to `\r\n` (CRLF) for proper newline display in host terminals.
    pub fn send_string(&mut self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.send_byte(b'\r');
            }
            self.send_byte(byte);
        }
    }
}

/// Implement standard `core::fmt::Write` to enable formatted writing via macros.
impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.send_string(s);
        Ok(())
    }
}

/// Global COM1 serial port instance (base address 0x3F8).
/// Protected by a Spinlock for safe concurrent logging.
pub static SERIAL1: Spinlock<SerialPort> = Spinlock::new(SerialPort::new(0x3F8));

/// Backend helper function for `serial_print!` and `serial_println!` macros.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    SERIAL1.lock().write_fmt(args).unwrap();
}

/// Prints formatted text to the COM1 serial line.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::serial::_print(format_args!($($arg)*)));
}

/// Prints formatted text to the COM1 serial line, followed by a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
