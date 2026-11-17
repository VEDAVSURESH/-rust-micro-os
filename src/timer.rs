//! 8254 Programmable Interval Timer (PIT) Driver.
//!
//! The 8254 PIT contains an internal oscillator operating at 1,193,182 Hz.
//! We configure Channel 0 to generate periodic square-wave interrupts (Mode 3) at ~100 Hz.
//! Divisor = 1,193,182 / 100 = 11,931 (0x2E9B).

use core::sync::atomic::{AtomicU64, Ordering};
use crate::serial::outb;

/// Hardware PIT I/O ports.
const PIT_CHANNEL_0_DATA: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

/// Target frequency in Hertz (100 ticks per second = 10 ms per tick).
pub const TIMER_FREQ_HZ: usize = 100;
const PIT_BASE_FREQUENCY_HZ: usize = 1_193_182;

/// Global atomic tick counter incremented on every hardware timer interrupt.
pub static TICKS: AtomicU64 = AtomicU64::new(0);

/// Initializes PIT Channel 0 to generate periodic timer interrupts at 100 Hz.
pub fn init() {
    let divisor = (PIT_BASE_FREQUENCY_HZ / TIMER_FREQ_HZ) as u16;

    // Command byte: 0b0011_0110 (0x36)
    // Bits 7-6: 00 = Channel 0
    // Bits 5-4: 11 = Access mode: lobyte/hibyte
    // Bits 3-1: 011 = Mode 3 (Square wave generator)
    // Bit 0:    0 = 16-bit binary mode
    unsafe {
        outb(PIT_COMMAND, 0x36);
        // Write low byte of divisor
        outb(PIT_CHANNEL_0_DATA, (divisor & 0xFF) as u8);
        // Write high byte of divisor
        outb(PIT_CHANNEL_0_DATA, ((divisor >> 8) & 0xFF) as u8);
    }
}

/// Returns the current number of timer ticks elapsed since boot.
#[inline]
pub fn current_ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Returns elapsed time in milliseconds (10 ms per tick at 100 Hz).
#[inline]
pub fn uptime_ms() -> u64 {
    current_ticks() * 10
}
