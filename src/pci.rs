//! PCI (Peripheral Component Interconnect) Bus Enumerator & Device Driver.
//!
//! Scans the PCI bus using standard Configuration Mechanism #1 (Ports 0xCF8 / 0xCFC),
//! detects all installed hardware, identifies vendors and device classes,
//! and enables bus mastering and MMIO for network adapters.

use alloc::vec::Vec;
use crate::serial::{inl, outl};
use crate::sync::Spinlock;

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// Reads a 32-bit dword from PCI configuration space.
pub fn pci_read_32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let address = (1u32 << 31)
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    unsafe {
        outl(PCI_CONFIG_ADDRESS, address);
        inl(PCI_CONFIG_DATA)
    }
}

/// Writes a 32-bit dword to PCI configuration space.
pub fn pci_write_32(bus: u8, slot: u8, func: u8, offset: u8, value: u32) {
    let address = (1u32 << 31)
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    unsafe {
        outl(PCI_CONFIG_ADDRESS, address);
        outl(PCI_CONFIG_DATA, value);
    }
}

/// Reads a 16-bit word from PCI configuration space.
pub fn pci_read_16(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    let dword = pci_read_32(bus, slot, func, offset & !3);
    ((dword >> (((offset & 2) as u32) * 8)) & 0xFFFF) as u16
}

/// Writes a 16-bit word to PCI configuration space.
pub fn pci_write_16(bus: u8, slot: u8, func: u8, offset: u8, value: u16) {
    let shift = ((offset & 2) as u32) * 8;
    let mask = !(0xFFFFu32 << shift);
    let current = pci_read_32(bus, slot, func, offset & !3);
    let new_val = (current & mask) | (((value as u32) & 0xFFFF) << shift);
    pci_write_32(bus, slot, func, offset & !3, new_val);
}

/// Represents a detected PCI device.
#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub slot: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_id: u8,
    pub subclass_id: u8,
    pub prog_if: u8,
    pub bar0: u32,
    pub irq: u8,
}

impl PciDevice {
    /// Returns a human-readable description of device class and function.
    pub fn class_name(&self) -> &'static str {
        match (self.class_id, self.subclass_id) {
            (0x01, 0x01) => "IDE Controller",
            (0x01, 0x06) => "SATA Controller (AHCI)",
            (0x01, _) => "Mass Storage Controller",
            (0x02, 0x00) => "Ethernet Controller",
            (0x02, _) => "Network Controller",
            (0x03, 0x00) => "VGA Compatible Controller",
            (0x03, _) => "Display Controller",
            (0x04, _) => "Multimedia Controller",
            (0x05, _) => "Memory Controller",
            (0x06, 0x00) => "Host Bridge",
            (0x06, 0x01) => "ISA Bridge",
            (0x06, 0x04) => "PCI-to-PCI Bridge",
            (0x06, _) => "Bridge Device",
            (0x0C, 0x03) => "USB Controller",
            (0x0C, _) => "Serial Bus Controller",
            _ => "Unknown Hardware Device",
        }
    }

    /// Returns human-readable vendor and product name if recognized.
    pub fn device_name(&self) -> &'static str {
        match (self.vendor_id, self.device_id) {
            (0x8086, 0x100E) => "Intel 82540EM Gigabit Ethernet (PRO/1000 MT)",
            (0x8086, 0x100F) => "Intel 82545EM Gigabit Ethernet (PRO/1000 MT)",
            (0x8086, 0x1004) => "Intel 82543GC Gigabit Ethernet",
            (0x8086, 0x1079) => "Intel 82546EB Dual-Port Gigabit Ethernet",
            (0x8086, 0x1237) => "Intel 440FX PMC Host Bridge",
            (0x8086, 0x7000) => "Intel 82371SB PIIX3 ISA Bridge",
            (0x8086, 0x7010) => "Intel 82371SB PIIX3 IDE Controller",
            (0x8086, 0x7113) => "Intel 82371AB/EB/MB PIIX4 ACPI",
            (0x1234, 0x1111) => "QEMU Standard VGA Display Adapter",
            (0x80EE, 0xBEEF) => "VirtualBox Guest Integration Device",
            (0x80EE, 0xCAFE) => "VirtualBox Guest Video Adapter",
            _ => "Generic PCI Device",
        }
    }

    /// Enables Memory Space (bit 1) and Bus Master (bit 2) in the PCI Command Register.
    pub fn enable_bus_mastering(&self) {
        let cmd = pci_read_16(self.bus, self.slot, self.func, 0x04);
        // Bit 0: I/O Space, Bit 1: Memory Space, Bit 2: Bus Master
        let new_cmd = cmd | 0x0006;
        pci_write_16(self.bus, self.slot, self.func, 0x04, new_cmd);
    }
}

/// Global list of enumerated PCI devices.
pub static PCI_DEVICES: Spinlock<Vec<PciDevice>> = Spinlock::new(Vec::new());

/// Scans all PCI buses (0..8) and devices (0..32), populating `PCI_DEVICES`.
pub fn enumerate_pci() {
    let mut devices = Vec::new();

    for bus in 0..8 {
        for slot in 0..32 {
            let vendor_id = pci_read_16(bus, slot, 0, 0x00);
            if vendor_id == 0xFFFF || vendor_id == 0x0000 {
                continue;
            }

            let header_type = (pci_read_32(bus, slot, 0, 0x0C) >> 16) as u8;
            let num_functions = if (header_type & 0x80) != 0 { 8 } else { 1 };

            for func in 0..num_functions {
                let v_id = pci_read_16(bus, slot, func, 0x00);
                if v_id == 0xFFFF || v_id == 0x0000 {
                    continue;
                }

                let d_id = pci_read_16(bus, slot, func, 0x02);
                let class_subclass = pci_read_32(bus, slot, func, 0x08);
                let prog_if = ((class_subclass >> 8) & 0xFF) as u8;
                let subclass_id = ((class_subclass >> 16) & 0xFF) as u8;
                let class_id = ((class_subclass >> 24) & 0xFF) as u8;

                let bar0 = pci_read_32(bus, slot, func, 0x10);
                let irq_line = (pci_read_32(bus, slot, func, 0x3C) & 0xFF) as u8;

                let dev = PciDevice {
                    bus,
                    slot,
                    func,
                    vendor_id: v_id,
                    device_id: d_id,
                    class_id,
                    subclass_id,
                    prog_if,
                    bar0,
                    irq: irq_line,
                };

                devices.push(dev);
            }
        }
    }

    *PCI_DEVICES.lock() = devices;
}

/// Finds the first network adapter (e.g. Intel 82540EM / PRO/1000 MT or any Ethernet class).
pub fn find_nic() -> Option<PciDevice> {
    let devs = PCI_DEVICES.lock();
    // 1. Look for Intel e1000 specifically (0x8086:0x100E or 0x100F or 0x1004)
    for dev in devs.iter() {
        if dev.vendor_id == 0x8086 && (dev.device_id == 0x100E || dev.device_id == 0x100F || dev.device_id == 0x1004 || dev.device_id == 0x1079) {
            return Some(*dev);
        }
    }
    // 2. Fall back to any Ethernet Network Controller (Class 0x02, Subclass 0x00)
    for dev in devs.iter() {
        if dev.class_id == 0x02 && dev.subclass_id == 0x00 {
            return Some(*dev);
        }
    }
    None
}

/// Displays all enumerated PCI hardware devices.
pub fn print_pci_devices() {
    let devs = PCI_DEVICES.lock();
    crate::println!("--- PCI Bus Hardware Enumeration ---");
    crate::serial_println!("--- PCI Bus Hardware Enumeration ---");
    crate::println!("B:S.F  | Vendor:Device | BAR0       | IRQ | Device Description");
    crate::serial_println!("B:S.F  | Vendor:Device | BAR0       | IRQ | Device Description");
    crate::println!("-------+---------------+------------+-----+--------------------------------");
    crate::serial_println!("-------+---------------+------------+-----+--------------------------------");

    for d in devs.iter() {
        crate::println!(
            "{:02x}:{:02x}.{} | {:04x}:{:04x}     | {:#010x} | {:<3} | {}",
            d.bus, d.slot, d.func, d.vendor_id, d.device_id, d.bar0, d.irq, d.device_name()
        );
        crate::serial_println!(
            "{:02x}:{:02x}.{} | {:04x}:{:04x}     | {:#010x} | {:<3} | {}",
            d.bus, d.slot, d.func, d.vendor_id, d.device_id, d.bar0, d.irq, d.device_name()
        );
    }
}
