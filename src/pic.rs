//! 8259 Programmable Interrupt Controller (PIC) Driver.
//!
//! The PC architecture contains two chained 8259 PIC chips:
//! - Master PIC: I/O ports 0x20 (command) and 0x21 (data)
//! - Slave PIC:  I/O ports 0xA0 (command) and 0xA1 (data)
//!
//! By default, the IBM PC hardware configured the Master PIC to use interrupt vectors 8..15.
//! In 32-bit and 64-bit protected mode, this directly collides with CPU exceptions (e.g. Double Fault is vector 8!).
//! We remap the PICs to vectors 32..47:
//! - Master PIC: vectors 32..39 (0x20..0x27) -> IRQ0 (Timer) is Vector 32.
//! - Slave PIC:  vectors 40..47 (0x28..0x2F).

use crate::serial::outb;
use crate::sync::Spinlock;

/// Interrupt vector offset for the Master PIC (vectors 32..39).
pub const PIC_1_OFFSET: u8 = 32;

/// Interrupt vector offset for the Slave PIC (vectors 40..47).
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8; // 40

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const PIC_EOI: u8 = 0x20;

// Initialization Command Words (ICW)
const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

/// Driver for the chained 8259 PIC controllers.
pub struct ChainedPics {
    offset1: u8,
    offset2: u8,
}

impl ChainedPics {
    pub const fn new(offset1: u8, offset2: u8) -> Self {
        Self { offset1, offset2 }
    }

    /// Remaps the 8259 Master and Slave PICs to the specified vector offsets.
    ///
    /// # Safety
    /// Writing to hardware PIC ports reconfigures CPU interrupt routing.
    pub unsafe fn initialize(&mut self) {
        // Small I/O delay by writing to an unused port (0x80)
        let io_wait = || outb(0x80, 0);

        // ICW1: Start initialization sequence in cascade mode (expect ICW4)
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();

        // ICW2: Vector offsets
        outb(PIC1_DATA, self.offset1); // Master vector offset (32)
        io_wait();
        outb(PIC2_DATA, self.offset2); // Slave vector offset (40)
        io_wait();

        // ICW3: Tell Master PIC that there is a Slave PIC at IRQ2 (0000 0100 = 0x04)
        outb(PIC1_DATA, 0x04);
        io_wait();
        // Tell Slave PIC its cascade identity (0000 0010 = 0x02)
        outb(PIC2_DATA, 0x02);
        io_wait();

        // ICW4: Set 8086/88 mode
        outb(PIC1_DATA, ICW4_8086);
        io_wait();
        outb(PIC2_DATA, ICW4_8086);
        io_wait();

        // Configure Interrupt Mask Registers:
        // Mask 0xF8 = 0b1111_1000 -> Unmask IRQ0 (Timer), IRQ1 (Keyboard), and IRQ2 (Slave PIC cascade)
        outb(PIC1_DATA, 0xF8);
        // Mask 0xEF = 0b1110_1111 -> Unmask IRQ12 (PS/2 Mouse), keep other slave IRQs masked.
        outb(PIC2_DATA, 0xEF);
    }

    /// Unmasks a specific IRQ line (0..15).
    pub unsafe fn unmask_irq(&mut self, irq: u8) {
        if irq < 8 {
            let port = PIC1_DATA;
            let value = crate::serial::inb(port) & !(1 << irq);
            outb(port, value);
        } else if irq < 16 {
            // Must also unmask cascade line IRQ 2 on master
            let master_val = crate::serial::inb(PIC1_DATA) & !(1 << 2);
            outb(PIC1_DATA, master_val);

            let port = PIC2_DATA;
            let value = crate::serial::inb(port) & !(1 << (irq - 8));
            outb(port, value);
        }
    }

    /// Masks a specific IRQ line (0..15).
    pub unsafe fn mask_irq(&mut self, irq: u8) {
        if irq < 8 {
            let port = PIC1_DATA;
            let value = crate::serial::inb(port) | (1 << irq);
            outb(port, value);
        } else if irq < 16 {
            let port = PIC2_DATA;
            let value = crate::serial::inb(port) | (1 << (irq - 8));
            outb(port, value);
        }
    }

    /// Acknowledges an interrupt by sending an End Of Interrupt (EOI) command to the PICs.
    ///
    /// If the interrupt came from the Slave PIC (IRQ 8..15), both PICs must be notified.
    /// If from the Master PIC (IRQ 0..7), only the Master PIC needs the EOI.
    pub unsafe fn notify_end_of_interrupt(&mut self, interrupt_id: u8) {
        if interrupt_id >= self.offset2 {
            outb(PIC2_COMMAND, PIC_EOI);
        }
        outb(PIC1_COMMAND, PIC_EOI);
    }
}

/// Global synchronized 8259 PIC controller instance.
pub static PICS: Spinlock<ChainedPics> = Spinlock::new(ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET));
