//! Real-Time Clock (RTC) CMOS Driver for JainOS.
//!
//! Reads date and time from the PC AT CMOS registers via I/O ports 0x70 and 0x71.

use crate::serial::{inb, outb};

const CMOS_ADDR_PORT: u16 = 0x70;
const CMOS_DATA_PORT: u16 = 0x71;

/// Date and time structure read from hardware RTC CMOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
}

/// Reads a single CMOS register by index.
unsafe fn read_cmos_register(reg: u8) -> u8 {
    outb(CMOS_ADDR_PORT, reg);
    inb(CMOS_DATA_PORT)
}

/// Checks if CMOS update is in progress (Status Register A bit 7).
unsafe fn is_update_in_progress() -> bool {
    (read_cmos_register(0x0A) & 0x80) != 0
}

/// Reads the current UTC date and time from CMOS RTC.
pub fn read_datetime() -> RtcDateTime {
    unsafe {
        // Wait until RTC update is not in progress
        while is_update_in_progress() {
            core::hint::spin_loop();
        }

        let mut sec = read_cmos_register(0x00);
        let mut min = read_cmos_register(0x02);
        let mut hour = read_cmos_register(0x04);
        let mut day = read_cmos_register(0x07);
        let mut month = read_cmos_register(0x08);
        let mut year = read_cmos_register(0x09) as u16;

        let register_b = read_cmos_register(0x0B);

        // Convert BCD to binary if BCD mode is enabled (bit 2 of Reg B is 0)
        let is_bcd = (register_b & 0x04) == 0;
        if is_bcd {
            sec = ((sec >> 4) * 10) + (sec & 0x0F);
            min = ((min >> 4) * 10) + (min & 0x0F);
            hour = ((hour & 0x0F) + (((hour & 0x70) >> 4) * 10)) | (hour & 0x80);
            day = ((day >> 4) * 10) + (day & 0x0F);
            month = ((month >> 4) * 10) + (month & 0x0F);
            year = ((year >> 4) * 10) + (year & 0x0F);
        }

        // Convert 12 hour clock to 24 hour clock if bit 1 of Reg B is 0
        if (register_b & 0x02) == 0 && (hour & 0x80) != 0 {
            hour = ((hour & 0x7F) + 12) % 24;
        }

        // Compute full year (assumes 2000s)
        let full_year = 2000 + year;

        RtcDateTime {
            year: full_year,
            month,
            day,
            hours: hour,
            minutes: min,
            seconds: sec,
        }
    }
}
