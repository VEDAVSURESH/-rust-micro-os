//! VGA Text Buffer Driver for 80x25 character display.
//!
//! On x86 PC architecture, the video card maps text buffer memory at physical address 0xb8000.
//! Each character is represented by 2 bytes:
//! - Byte 0: ASCII code
//! - Byte 1: Color attribute (4 bits background, 4 bits foreground)

use core::fmt;
use crate::sync::Spinlock;

/// Standard 16 VGA colors.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// Encapsulates a full color code byte (background in upper 4 bits, foreground in lower 4 bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    pub const fn new(foreground: Color, background: Color) -> Self {
        ColorCode(((background as u8) << 4) | (foreground as u8))
    }
}

/// A single cell in the VGA text buffer: 1 byte ASCII character + 1 byte color code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ScreenChar {
    pub ascii_character: u8,
    pub color_code: ColorCode,
}

/// Dimensions of standard VGA text mode.
pub const BUFFER_HEIGHT: usize = 25;
pub const BUFFER_WIDTH: usize = 80;

/// Memory layout of the 80x25 VGA buffer.
#[repr(transparent)]
pub struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

/// Text writer that tracks screen cursor position and handles scrolling.
pub struct Writer {
    pub column_position: usize,
    pub color_code: ColorCode,
    buffer: *mut Buffer,
}

// Safety: The buffer pointer 0xb8000 is static memory mapped by hardware.
// Concurrent access is synchronized via Spinlock<Writer>.
unsafe impl Send for Writer {}
unsafe impl Sync for Writer {}

impl Writer {
    /// Creates a writer pointing to physical VGA address 0xb8000.
    pub const fn new() -> Self {
        Self {
            column_position: 0,
            color_code: ColorCode::new(Color::White, Color::Black),
            buffer: 0xb8000 as *mut Buffer,
        }
    }

    /// Sets the active text colors for subsequent writes.
    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color_code = ColorCode::new(foreground, background);
    }

    /// Deletes the last character written, moving the cursor back one column.
    pub fn backspace(&mut self) {
        if self.column_position > 0 {
            self.column_position -= 1;
            let row = BUFFER_HEIGHT - 1;
            let col = self.column_position;
            let color_code = self.color_code;
            unsafe {
                let char_ptr = &mut (*self.buffer).chars[row][col] as *mut ScreenChar;
                core::ptr::write_volatile(
                    char_ptr,
                    ScreenChar {
                        ascii_character: b' ',
                        color_code,
                    },
                );
            }
        }
    }

    /// Writes a single byte to the screen. Handles newlines, backspaces, and line wrapping.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            0x08 => self.backspace(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;
                let color_code = self.color_code;

                // Safety: We use `write_volatile` because 0xb8000 is Memory-Mapped I/O (MMIO).
                // Without volatile writes, the Rust/LLVM optimizer might assume this is regular RAM
                // and optimize away consecutive writes, causing characters not to appear on screen.
                unsafe {
                    let char_ptr = &mut (*self.buffer).chars[row][col] as *mut ScreenChar;
                    core::ptr::write_volatile(
                        char_ptr,
                        ScreenChar {
                            ascii_character: byte,
                            color_code,
                        },
                    );
                }
                self.column_position += 1;
            }
        }
    }

    /// Writes an ASCII/UTF-8 string slice to the buffer.
    /// Unprintable characters outside ASCII range 0x20..=0x7e (except newline & backspace) are replaced by 0xfe.
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // Printable ASCII, newline, or backspace
                0x20..=0x7e | b'\n' | 0x08 => self.write_byte(byte),
                // Fallback for non-ASCII or unprintable characters
                _ => self.write_byte(0xfe),
            }
        }
    }

    /// Shifts all text lines up by one row, scrolling the top line off-screen,
    /// and clears the bottom line.
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                // Safety: Read each character from the row below and write it to the row above.
                // Both reads and writes are volatile to prevent compiler reordering.
                unsafe {
                    let src = &(*self.buffer).chars[row][col] as *const ScreenChar;
                    let character = core::ptr::read_volatile(src);
                    let dst = &mut (*self.buffer).chars[row - 1][col] as *mut ScreenChar;
                    core::ptr::write_volatile(dst, character);
                }
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    /// Clears a specific row by overwriting every character with a space character.
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            unsafe {
                let dst = &mut (*self.buffer).chars[row][col] as *mut ScreenChar;
                core::ptr::write_volatile(dst, blank);
            }
        }
    }

    /// Clears the entire 80x25 screen.
    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.column_position = 0;
    }
}

/// Implement standard formatting trait `core::fmt::Write` so Rust's `write!` macro works.
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Global synchronized VGA buffer writer.
/// Protected by a Spinlock so that tasks and interrupts can safely output text
/// without garbling the screen.
pub static WRITER: Spinlock<Writer> = Spinlock::new(Writer::new());

/// Backend helper function for the `print!` and `println!` macros.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}

/// Prints formatted text to the VGA display.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

/// Prints formatted text to the VGA display, followed by a newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
