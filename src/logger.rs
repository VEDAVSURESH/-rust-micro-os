//! Kernel Logging Subsystem.
//!
//! Provides structured logging levels:
//! - TRACE
//! - DEBUG
//! - INFO
//! - WARN
//! - ERROR
//!
//! Log entries are formatted with uptime timestamps and sent to both
//! the VGA text buffer (with level-specific colors) and the COM1 serial console.

use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};
use crate::vga_buffer::{Color, WRITER};
use crate::{serial_print, serial_println};
use crate::timer;

/// Global flag controlling whether logs are written to the VGA screen.
static VGA_LOGGING_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enables or disables output of log messages to the VGA display.
pub fn set_vga_logging(enabled: bool) {
    VGA_LOGGING_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Logging severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info  = 2,
    Warn  = 3,
    Error = 4,
}

impl LogLevel {
    /// Returns standard text string for the level.
    pub const fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info  => "INFO ",
            LogLevel::Warn  => "WARN ",
            LogLevel::Error => "ERROR",
        }
    }

    /// Returns corresponding display color on the VGA screen.
    pub const fn vga_color(&self) -> Color {
        match self {
            LogLevel::Trace => Color::DarkGray,
            LogLevel::Debug => Color::LightCyan,
            LogLevel::Info  => Color::LightGreen,
            LogLevel::Warn  => Color::Yellow,
            LogLevel::Error => Color::LightRed,
        }
    }
}

/// Internal logging backend function.
#[doc(hidden)]
pub fn _log(level: LogLevel, args: fmt::Arguments) {
    use fmt::Write;

    let ticks = timer::current_ticks();
    let seconds = ticks / 100;
    let centis = ticks % 100;

    // 1. Output to Serial console (WSL2 terminal)
    serial_print!("[{:03}.{:02}s] [{}] ", seconds, centis, level.as_str());
    crate::serial::SERIAL1.lock().write_fmt(args).unwrap();
    serial_println!();

    // 2. Output to VGA text display with level-specific color formatting (if enabled)
    if VGA_LOGGING_ENABLED.load(Ordering::Relaxed) {
        let mut writer = WRITER.lock();
        writer.set_color(Color::DarkGray, Color::Black);
        write!(writer, "[{:03}.{:02}s] [", seconds, centis).unwrap();
        writer.set_color(level.vga_color(), Color::Black);
        write!(writer, "{}", level.as_str()).unwrap();
        writer.set_color(Color::DarkGray, Color::Black);
        write!(writer, "] ").unwrap();
        writer.set_color(Color::White, Color::Black);
        writer.write_fmt(args).unwrap();
        writer.write_byte(b'\n');
    }
}

/// Logs a message at TRACE severity.
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => ($crate::logger::_log($crate::logger::LogLevel::Trace, format_args!($($arg)*)));
}

/// Logs a message at DEBUG severity.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => ($crate::logger::_log($crate::logger::LogLevel::Debug, format_args!($($arg)*)));
}

/// Logs a message at INFO severity.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => ($crate::logger::_log($crate::logger::LogLevel::Info, format_args!($($arg)*)));
}

/// Logs a message at WARN severity.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => ($crate::logger::_log($crate::logger::LogLevel::Warn, format_args!($($arg)*)));
}

/// Logs a message at ERROR severity.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => ($crate::logger::_log($crate::logger::LogLevel::Error, format_args!($($arg)*)));
}
