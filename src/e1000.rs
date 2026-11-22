//! Intel PRO/1000 MT Desktop (82540EM / e1000) Gigabit Ethernet Network Driver.
//!
//! Provides bare-metal support for:
//! - BAR0 MMIO register access
//! - Hardware MAC address detection (RAL/RAH and EEPROM)
//! - Circular DMA Receive Descriptor Ring with dedicated physical frames
//! - Circular DMA Transmit Descriptor Ring with dedicated physical frames
//! - Hardware packet reception with CRC stripping
//! - Hardware packet transmission with automatic padding and CRC insertion
//! - Real-time link status detection and statistics

use alloc::vec::Vec;
use crate::pci::PciDevice;
use crate::sync::Spinlock;

/// Number of receive descriptors (must be a multiple of 8, max aligned to 128 bytes).
pub const NUM_RX_DESCRIPTORS: usize = 32;

/// Number of transmit descriptors (must be a multiple of 8, max aligned to 128 bytes).
pub const NUM_TX_DESCRIPTORS: usize = 16;

/// Standard buffer size for incoming and outgoing Ethernet frames (2048 bytes).
pub const BUFFER_SIZE: usize = 2048;

/// e1000 Receive Descriptor (Legacy format, 16 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RxDescriptor {
    pub buffer_addr: u64,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

/// e1000 Transmit Descriptor (Legacy format, 16 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct TxDescriptor {
    pub buffer_addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

// e1000 MMIO Register Offsets
const REG_CTRL: usize = 0x0000;
const REG_STATUS: usize = 0x0008;
const REG_EERD: usize = 0x0014;
const REG_ICR: usize = 0x00C0;
const REG_IMC: usize = 0x00D0;
const REG_RCTL: usize = 0x0100;
const REG_TCTL: usize = 0x0400;
const REG_TIPG: usize = 0x0410;
const REG_RDBAL: usize = 0x2800;
const REG_RDBAH: usize = 0x2804;
const REG_RDLEN: usize = 0x2808;
const REG_RDH: usize = 0x2810;
const REG_RDT: usize = 0x2818;
const REG_TDBAL: usize = 0x3800;
const REG_TDBAH: usize = 0x3804;
const REG_TDLEN: usize = 0x3808;
const REG_TDH: usize = 0x3810;
const REG_TDT: usize = 0x3818;
const REG_MTA: usize = 0x5200;
const REG_RAL: usize = 0x5400;
const REG_RAH: usize = 0x5404;

/// e1000 Network Interface Card driver instance.
pub struct E1000Device {
    mmio_base: usize,
    mac_address: [u8; 6],

    // RX Ring
    rx_descs: *mut RxDescriptor,
    rx_bufs_virt: [usize; NUM_RX_DESCRIPTORS],
    rx_cur: usize,

    // TX Ring
    tx_descs: *mut TxDescriptor,
    tx_bufs_virt: [usize; NUM_TX_DESCRIPTORS],
    tx_cur: usize,

    // Metrics
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub tx_packets: u64,
    pub tx_bytes: u64,
}

unsafe impl Send for E1000Device {}
unsafe impl Sync for E1000Device {}

impl E1000Device {
    /// Reads a 32-bit MMIO register.
    #[inline(always)]
    unsafe fn read_reg(&self, offset: usize) -> u32 {
        core::ptr::read_volatile((self.mmio_base + offset) as *const u32)
    }

    /// Writes a 32-bit MMIO register.
    #[inline(always)]
    unsafe fn write_reg(&self, offset: usize, value: u32) {
        core::ptr::write_volatile((self.mmio_base + offset) as *mut u32, value);
    }

    /// Reads 16 bits from the EEPROM at the given word address.
    fn read_eeprom(&self, addr: u8) -> u16 {
        unsafe {
            // Write to EERD: start read (bit 0 = 1), address in bits 15:8
            self.write_reg(REG_EERD, 1 | ((addr as u32) << 8));
            let mut val = self.read_reg(REG_EERD);
            let mut timeout = 100_000;
            while (val & (1 << 4)) == 0 && timeout > 0 {
                core::hint::spin_loop();
                val = self.read_reg(REG_EERD);
                timeout -= 1;
            }
            if timeout == 0 {
                return 0;
            }
            ((val >> 16) & 0xFFFF) as u16
        }
    }

    /// Initializes the e1000 controller from a detected PCI device.
    pub fn new(pci_dev: PciDevice) -> Result<Self, &'static str> {
        if (pci_dev.bar0 & 1) != 0 {
            return Err("e1000 BAR0 is an I/O space BAR, memory-mapped MMIO required");
        }
        let bar0_phys = (pci_dev.bar0 & !0xF) as u64;
        if bar0_phys == 0 {
            return Err("e1000 BAR0 physical address is zero");
        }

        pci_dev.enable_bus_mastering();

        let mmio_virt = crate::memory::map_mmio_range(bar0_phys, 128 * 1024)
            .map_err(|_| "Failed to map e1000 BAR0 MMIO space")?;
        let mmio_base = mmio_virt.as_u64() as usize;

        let mut dev = Self {
            mmio_base,
            mac_address: [0; 6],
            rx_descs: core::ptr::null_mut(),
            rx_bufs_virt: [0; NUM_RX_DESCRIPTORS],
            rx_cur: 0,
            tx_descs: core::ptr::null_mut(),
            tx_bufs_virt: [0; NUM_TX_DESCRIPTORS],
            tx_cur: 0,
            rx_packets: 0,
            rx_bytes: 0,
            tx_packets: 0,
            tx_bytes: 0,
        };

        unsafe {
            // 1. Disable all interrupts
            dev.write_reg(REG_IMC, 0xFFFF_FFFF);
            let _ = dev.read_reg(REG_ICR); // Read to clear pending causes

            // 2. Set Link Up (SLU bit 6) and Auto-Speed Detection (ASDE bit 5)
            let ctrl = dev.read_reg(REG_CTRL);
            dev.write_reg(REG_CTRL, ctrl | (1 << 6) | (1 << 5));

            // 3. Detect MAC Address
            let ral = dev.read_reg(REG_RAL);
            let rah = dev.read_reg(REG_RAH);

            if ral != 0 && (rah & (1 << 31)) != 0 {
                // Read from Receive Address Low / High
                dev.mac_address[0] = (ral & 0xFF) as u8;
                dev.mac_address[1] = ((ral >> 8) & 0xFF) as u8;
                dev.mac_address[2] = ((ral >> 16) & 0xFF) as u8;
                dev.mac_address[3] = ((ral >> 24) & 0xFF) as u8;
                dev.mac_address[4] = (rah & 0xFF) as u8;
                dev.mac_address[5] = ((rah >> 8) & 0xFF) as u8;
            } else {
                // Read from EEPROM words 0, 1, 2
                let w0 = dev.read_eeprom(0);
                let w1 = dev.read_eeprom(1);
                let w2 = dev.read_eeprom(2);

                dev.mac_address[0] = (w0 & 0xFF) as u8;
                dev.mac_address[1] = ((w0 >> 8) & 0xFF) as u8;
                dev.mac_address[2] = (w1 & 0xFF) as u8;
                dev.mac_address[3] = ((w1 >> 8) & 0xFF) as u8;
                dev.mac_address[4] = (w2 & 0xFF) as u8;
                dev.mac_address[5] = ((w2 >> 8) & 0xFF) as u8;

                // Write MAC into RAL/RAH registers and set Address Valid (AV = bit 31)
                let new_ral = (dev.mac_address[0] as u32)
                    | ((dev.mac_address[1] as u32) << 8)
                    | ((dev.mac_address[2] as u32) << 16)
                    | ((dev.mac_address[3] as u32) << 24);
                let new_rah = (dev.mac_address[4] as u32)
                    | ((dev.mac_address[5] as u32) << 8)
                    | (1u32 << 31);
                dev.write_reg(REG_RAL, new_ral);
                dev.write_reg(REG_RAH, new_rah);
            }

            // 4. Initialize Receive DMA Ring
            let rx_ring_frame = crate::memory::allocate_frame()
                .ok_or("Failed to allocate physical frame for e1000 RX ring")?;
            let rx_ring_phys = rx_ring_frame.start_address().as_u64();
            let rx_ring_virt = crate::memory::phys_to_virt(rx_ring_phys) as *mut RxDescriptor;

            core::ptr::write_bytes(rx_ring_virt as *mut u8, 0, NUM_RX_DESCRIPTORS * core::mem::size_of::<RxDescriptor>());
            dev.rx_descs = rx_ring_virt;

            // Allocate 16 frames = 32 x 2048-byte buffers
            for i in 0..(NUM_RX_DESCRIPTORS / 2) {
                let frame = crate::memory::allocate_frame()
                    .ok_or("Failed to allocate physical frame for RX buffers")?;
                let frame_phys = frame.start_address().as_u64();
                let frame_virt = crate::memory::phys_to_virt(frame_phys) as usize;

                // Buffer 0 in frame
                let idx0 = i * 2;
                dev.rx_bufs_virt[idx0] = frame_virt;
                let desc0 = &mut *dev.rx_descs.add(idx0);
                desc0.buffer_addr = frame_phys;
                desc0.status = 0;

                // Buffer 1 in frame
                let idx1 = i * 2 + 1;
                dev.rx_bufs_virt[idx1] = frame_virt + BUFFER_SIZE;
                let desc1 = &mut *dev.rx_descs.add(idx1);
                desc1.buffer_addr = frame_phys + (BUFFER_SIZE as u64);
                desc1.status = 0;
            }

            dev.write_reg(REG_RDBAL, rx_ring_phys as u32);
            dev.write_reg(REG_RDBAH, (rx_ring_phys >> 32) as u32);
            dev.write_reg(REG_RDLEN, (NUM_RX_DESCRIPTORS * core::mem::size_of::<RxDescriptor>()) as u32);
            dev.write_reg(REG_RDH, 0);
            dev.write_reg(REG_RDT, (NUM_RX_DESCRIPTORS - 1) as u32);
            dev.rx_cur = 0;

            // Zero Multicast Table Array
            for i in 0..128 {
                dev.write_reg(REG_MTA + (i * 4), 0);
            }

            // Enable Receiver: EN(1), SBP(2), UPE(3), MPE(4), SECRC(26), BSIZE=2048(0)
            let rctl = (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 15) | (1 << 26);
            dev.write_reg(REG_RCTL, rctl);

            // 5. Initialize Transmit DMA Ring
            let tx_ring_frame = crate::memory::allocate_frame()
                .ok_or("Failed to allocate physical frame for e1000 TX ring")?;
            let tx_ring_phys = tx_ring_frame.start_address().as_u64();
            let tx_ring_virt = crate::memory::phys_to_virt(tx_ring_phys) as *mut TxDescriptor;

            core::ptr::write_bytes(tx_ring_virt as *mut u8, 0, NUM_TX_DESCRIPTORS * core::mem::size_of::<TxDescriptor>());
            dev.tx_descs = tx_ring_virt;

            // Allocate 8 frames = 16 x 2048-byte TX buffers
            for i in 0..(NUM_TX_DESCRIPTORS / 2) {
                let frame = crate::memory::allocate_frame()
                    .ok_or("Failed to allocate physical frame for TX buffers")?;
                let frame_phys = frame.start_address().as_u64();
                let frame_virt = crate::memory::phys_to_virt(frame_phys) as usize;

                let idx0 = i * 2;
                dev.tx_bufs_virt[idx0] = frame_virt;
                let desc0 = &mut *dev.tx_descs.add(idx0);
                desc0.buffer_addr = frame_phys;
                desc0.status = 1; // Mark DD (ready for transmission)

                let idx1 = i * 2 + 1;
                dev.tx_bufs_virt[idx1] = frame_virt + BUFFER_SIZE;
                let desc1 = &mut *dev.tx_descs.add(idx1);
                desc1.buffer_addr = frame_phys + (BUFFER_SIZE as u64);
                desc1.status = 1; // Mark DD
            }

            dev.write_reg(REG_TDBAL, tx_ring_phys as u32);
            dev.write_reg(REG_TDBAH, (tx_ring_phys >> 32) as u32);
            dev.write_reg(REG_TDLEN, (NUM_TX_DESCRIPTORS * core::mem::size_of::<TxDescriptor>()) as u32);
            dev.write_reg(REG_TDH, 0);
            dev.write_reg(REG_TDT, 0);
            dev.tx_cur = 0;

            // Configure Transmit IPG (TIPG standard: IPGT=10, IPGR1=8, IPGR2=6)
            dev.write_reg(REG_TIPG, 0x0060200A);

            // Configure Transmit Control: EN(1), PSP(3), CT=15(4..11), COLD=64(12..21)
            let tctl = (1 << 1) | (1 << 3) | (0x0F << 4) | (0x40 << 12);
            dev.write_reg(REG_TCTL, tctl);
        }

        Ok(dev)
    }

    /// Returns the hardware 6-byte Ethernet MAC address.
    pub fn mac_address(&self) -> [u8; 6] {
        self.mac_address
    }

    /// Checks if the physical Ethernet link is up (STATUS register bit 1).
    pub fn is_link_up(&self) -> bool {
        unsafe { (self.read_reg(REG_STATUS) & (1 << 1)) != 0 }
    }

    /// Sends a raw Ethernet packet via DMA.
    pub fn send_packet(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        if packet.len() > BUFFER_SIZE {
            return Err("Packet exceeds maximum transmission unit (2048 bytes)");
        }

        let cur = self.tx_cur;
        let desc = unsafe { &mut *self.tx_descs.add(cur) };

        // Verify that the descriptor has been processed by hardware (Descriptor Done: bit 0)
        if (desc.status & 0x01) == 0 {
            return Err("e1000 TX descriptor ring full");
        }

        // Copy packet data to DMA buffer
        let buf_virt = self.tx_bufs_virt[cur];
        unsafe {
            core::ptr::copy_nonoverlapping(packet.as_ptr(), buf_virt as *mut u8, packet.len());
        }

        desc.length = packet.len() as u16;
        // CMD: EOP(bit 0), IFCS(bit 1), RS(bit 3) -> 0x0B
        desc.cmd = (1 << 0) | (1 << 1) | (1 << 3);
        desc.status = 0;

        self.tx_cur = (cur + 1) % NUM_TX_DESCRIPTORS;
        unsafe {
            self.write_reg(REG_TDT, self.tx_cur as u32);
        }

        self.tx_packets = self.tx_packets.saturating_add(1);
        self.tx_bytes = self.tx_bytes.saturating_add(packet.len() as u64);

        Ok(())
    }

    /// Receives a raw Ethernet packet from the DMA ring if available.
    pub fn receive_packet(&mut self) -> Option<Vec<u8>> {
        let cur = self.rx_cur;
        let desc = unsafe { &mut *self.rx_descs.add(cur) };

        // Check if Descriptor Done (DD: bit 0) is set by hardware
        if (desc.status & 0x01) == 0 {
            return None;
        }

        let length = desc.length as usize;
        let buf_virt = self.rx_bufs_virt[cur];
        let mut packet = alloc::vec![0u8; length];

        unsafe {
            core::ptr::copy_nonoverlapping(buf_virt as *const u8, packet.as_mut_ptr(), length);
        }

        // Clear descriptor status and advance tail to give buffer back to hardware
        desc.status = 0;
        self.rx_cur = (cur + 1) % NUM_RX_DESCRIPTORS;
        unsafe {
            self.write_reg(REG_RDT, cur as u32);
        }

        self.rx_packets = self.rx_packets.saturating_add(1);
        self.rx_bytes = self.rx_bytes.saturating_add(length as u64);

        Some(packet)
    }
}

/// Global synchronized Intel e1000 driver instance.
pub static E1000: Spinlock<Option<E1000Device>> = Spinlock::new(None);

/// Initializes the Intel e1000 driver if an adapter is detected on the PCI bus.
pub fn init() -> bool {
    if let Some(pci_dev) = crate::pci::find_nic() {
        crate::log_info!(
            "Intel e1000 Network Card detected at {:02x}:{:02x}.{} (Device ID: {:#06x}, BAR0: {:#010x})",
            pci_dev.bus, pci_dev.slot, pci_dev.func, pci_dev.device_id, pci_dev.bar0
        );

        match E1000Device::new(pci_dev) {
            Ok(dev) => {
                let mac = dev.mac_address();
                crate::log_info!(
                    "Intel e1000 initialized successfully! MAC Address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                );
                let link_up = dev.is_link_up();
                crate::log_info!("Link status: {}", if link_up { "UP (Online)" } else { "DOWN" });
                *E1000.lock() = Some(dev);
                true
            }
            Err(e) => {
                crate::log_error!("Failed to initialize Intel e1000 driver: {}", e);
                false
            }
        }
    } else {
        crate::log_warn!("No supported PCI Ethernet Network Adapter found.");
        false
    }
}

/// Sends a packet via the global e1000 driver.
pub fn send_packet(packet: &[u8]) -> Result<(), &'static str> {
    let mut dev = E1000.lock();
    match dev.as_mut() {
        Some(d) => d.send_packet(packet),
        None => Err("e1000 driver not initialized"),
    }
}

/// Receives a packet from the global e1000 driver.
pub fn receive_packet() -> Option<Vec<u8>> {
    let mut dev = E1000.lock();
    dev.as_mut()?.receive_packet()
}

/// Returns the detected MAC address, if initialized.
pub fn mac_address() -> Option<[u8; 6]> {
    let dev = E1000.lock();
    Some(dev.as_ref()?.mac_address())
}
