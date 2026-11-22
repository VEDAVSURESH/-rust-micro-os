//! Bochs Graphics Adapter (BGA) & VBE Linear Framebuffer Driver.
//!
//! Provides linear framebuffer graphics for QEMU and VirtualBox,
//! supporting 1024x768 and 800x600 in 32-bit True Color (ARGB/XRGB).

use crate::serial::{inw, outw};
use crate::sync::Spinlock;

pub const VBE_DISPI_IOPORT_INDEX: u16 = 0x01CE;
pub const VBE_DISPI_IOPORT_DATA: u16 = 0x01CF;

pub const VBE_DISPI_INDEX_ID: u16 = 0;
pub const VBE_DISPI_INDEX_XRES: u16 = 1;
pub const VBE_DISPI_INDEX_YRES: u16 = 2;
pub const VBE_DISPI_INDEX_BPP: u16 = 3;
pub const VBE_DISPI_INDEX_ENABLE: u16 = 4;
pub const VBE_DISPI_INDEX_BANK: u16 = 5;
pub const VBE_DISPI_INDEX_VIRT_WIDTH: u16 = 6;
pub const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 7;
pub const VBE_DISPI_INDEX_X_OFFSET: u16 = 8;
pub const VBE_DISPI_INDEX_Y_OFFSET: u16 = 9;

pub const VBE_DISPI_DISABLED: u16 = 0x00;
pub const VBE_DISPI_ENABLED: u16 = 0x01;
pub const VBE_DISPI_LFB_ENABLED: u16 = 0x40;
pub const VBE_DISPI_NOCLEARMEM: u16 = 0x80;

/// Writes a 16-bit value to a BGA register.
pub fn bga_write_reg(index: u16, value: u16) {
    unsafe {
        outw(VBE_DISPI_IOPORT_INDEX, index);
        outw(VBE_DISPI_IOPORT_DATA, value);
    }
}

/// Reads a 16-bit value from a BGA register.
pub fn bga_read_reg(index: u16) -> u16 {
    unsafe {
        outw(VBE_DISPI_IOPORT_INDEX, index);
        inw(VBE_DISPI_IOPORT_DATA)
    }
}

/// Checks whether the Bochs Graphics Adapter is present and responsive.
pub fn is_available() -> bool {
    let id = bga_read_reg(VBE_DISPI_INDEX_ID);
    if id >= 0xB0C0 && id <= 0xB0C6 {
        return true;
    }
    // Attempt writing ID5 and reading back to negotiate protocol version
    bga_write_reg(VBE_DISPI_INDEX_ID, 0xB0C5);
    let check = bga_read_reg(VBE_DISPI_INDEX_ID);
    check >= 0xB0C0 && check <= 0xB0C6
}

/// Information about active graphical display mode and virtual memory buffer.
#[derive(Debug, Clone, Copy)]
pub struct DisplayInfo {
    pub width: usize,
    pub height: usize,
    pub bpp: usize,
    pub pitch: usize,
    pub lfb_virt_addr: usize,
    pub lfb_phys_addr: u64,
}

pub static DISPLAY: Spinlock<Option<DisplayInfo>> = Spinlock::new(None);

/// Switches hardware into specified graphics mode and maps the linear framebuffer into virtual memory.
pub fn set_video_mode(width: usize, height: usize, bpp: usize) -> Result<(), &'static str> {
    if !is_available() {
        return Err("Bochs Graphics Adapter (BGA) hardware not detected");
    }

    // 1. Disable graphics while updating registers
    bga_write_reg(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);
    bga_write_reg(VBE_DISPI_INDEX_XRES, width as u16);
    bga_write_reg(VBE_DISPI_INDEX_YRES, height as u16);
    bga_write_reg(VBE_DISPI_INDEX_BPP, bpp as u16);
    bga_write_reg(
        VBE_DISPI_INDEX_ENABLE,
        VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED,
    );

    // 2. Query physical address of LFB from PCI enumeration
    let lfb_phys = crate::pci::get_framebuffer_phys_addr();
    let total_bytes = width * height * (bpp / 8);

    crate::serial_println!(
        "[BGA] Mode {}x{}x{} set. LFB physical address: {:#010x}, size: {} KiB",
        width,
        height,
        bpp,
        lfb_phys,
        total_bytes / 1024
    );

    // 3. Map physical framebuffer range into active page table
    let virt_addr = crate::memory::map_mmio_range(lfb_phys, total_bytes)
        .map_err(|_| "Failed to map framebuffer MMIO range into page table")?;

    let lfb_virt = virt_addr.as_u64() as usize;

    *DISPLAY.lock() = Some(DisplayInfo {
        width,
        height,
        bpp,
        pitch: width * (bpp / 8),
        lfb_virt_addr: lfb_virt,
        lfb_phys_addr: lfb_phys,
    });

    // Also update mouse boundary limits
    crate::mouse::set_screen_bounds(width as i32, height as i32);

    Ok(())
}

/// Disables graphics mode and returns hardware to standard text console mode.
pub fn disable_graphics() {
    bga_write_reg(VBE_DISPI_INDEX_ENABLE, VBE_DISPI_DISABLED);
    *DISPLAY.lock() = None;
}
