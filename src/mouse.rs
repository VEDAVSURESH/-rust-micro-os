//! PS/2 Mouse Driver for JainOS.
//!
//! Handles 8042 Auxiliary PS/2 Port, IRQ 12 (Vector 44), 3-byte standard mouse packets,
//! coordinate tracking, left/right/middle clicks, and screen clamp limits.

use crate::serial::{inb, outb};
use crate::sync::Spinlock;

/// Current state of the graphical pointing device.
#[derive(Debug, Clone, Copy)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub left_button: bool,
    pub right_button: bool,
    pub middle_button: bool,
    pub max_x: i32,
    pub max_y: i32,
}

impl MouseState {
    pub const fn new(max_x: i32, max_y: i32) -> Self {
        Self {
            x: max_x / 2,
            y: max_y / 2,
            left_button: false,
            right_button: false,
            middle_button: false,
            max_x,
            max_y,
        }
    }
}

pub static MOUSE: Spinlock<MouseState> = Spinlock::new(MouseState::new(1024, 768));

/// Internal packet parser state machine.
struct MouseDriverState {
    cycle: u8,
    packet: [u8; 3],
}

static DRIVER_STATE: Spinlock<MouseDriverState> = Spinlock::new(MouseDriverState {
    cycle: 0,
    packet: [0; 3],
});

unsafe fn mouse_wait_input() {
    let mut timeout = 100_000;
    while (inb(0x64) & 0x02) != 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
}

unsafe fn mouse_wait_output() -> bool {
    let mut timeout = 100_000;
    while (inb(0x64) & 0x01) == 0 && timeout > 0 {
        core::hint::spin_loop();
        timeout -= 1;
    }
    timeout > 0
}

unsafe fn mouse_write(byte: u8) {
    mouse_wait_input();
    outb(0x64, 0xD4); // Tell 8042 controller to route next byte to auxiliary device (mouse)
    mouse_wait_input();
    outb(0x60, byte);
}

unsafe fn mouse_read() -> u8 {
    if mouse_wait_output() {
        inb(0x60)
    } else {
        0
    }
}

/// Initializes the PS/2 mouse interface on the 8042 controller.
pub fn init() {
    unsafe {
        // 1. Enable auxiliary device (second PS/2 port)
        mouse_wait_input();
        outb(0x64, 0xA8);

        // 2. Read Compaq status byte from 8042
        mouse_wait_input();
        outb(0x64, 0x20);
        let mut status = mouse_read();

        // 3. Set bit 1 (enable IRQ 12) and clear bit 5 (enable mouse clock)
        status |= 0x02;
        status &= !0x20;

        // 4. Write Compaq status byte back
        mouse_wait_input();
        outb(0x64, 0x60);
        mouse_wait_input();
        outb(0x60, status);

        // 5. Set mouse defaults (command 0xF6)
        mouse_write(0xF6);
        let _ack1 = mouse_read();

        // 6. Enable mouse streaming / data reporting (command 0xF4)
        mouse_write(0xF4);
        let _ack2 = mouse_read();
    }
    crate::serial_println!("[MOUSE] 8042 PS/2 auxiliary mouse driver initialized (IRQ 12 active)");
}

/// Sets screen boundary limits for mouse coordinate clamping.
pub fn set_screen_bounds(width: i32, height: i32) {
    let mut m = MOUSE.lock();
    m.max_x = width;
    m.max_y = height;
    m.x = m.x.clamp(0, width - 1);
    m.y = m.y.clamp(0, height - 1);
}

/// Interrupt handler called directly from IDT Vector 44 (IRQ 12).
pub fn handle_interrupt() {
    let status = unsafe { inb(0x64) };
    if (status & 0x01) == 0 {
        return;
    }
    // Check if data came from auxiliary device (bit 5)
    if (status & 0x20) == 0 {
        return;
    }

    let byte = unsafe { inb(0x60) };
    let mut state = DRIVER_STATE.lock();

    match state.cycle {
        0 => {
            // In standard PS/2 mouse protocol, bit 3 of byte 0 is always 1
            if (byte & 0x08) != 0 {
                state.packet[0] = byte;
                state.cycle = 1;
            }
        }
        1 => {
            state.packet[1] = byte;
            state.cycle = 2;
        }
        2 => {
            state.packet[2] = byte;
            state.cycle = 0;

            // Packet complete - decode coordinates
            let flags = state.packet[0];
            let mut dx = state.packet[1] as i32;
            let mut dy = state.packet[2] as i32;

            // Sign extend 8-bit to 32-bit if negative
            if (flags & 0x10) != 0 {
                dx |= !0xFF;
            }
            if (flags & 0x20) != 0 {
                dy |= !0xFF;
            }

            // In PS/2 mouse protocol, positive dy moves upward, but screen y increases downward
            let screen_dy = -dy;

            let mut mouse = MOUSE.lock();
            mouse.left_button = (flags & 0x01) != 0;
            mouse.right_button = (flags & 0x02) != 0;
            mouse.middle_button = (flags & 0x04) != 0;

            mouse.x = (mouse.x + dx).clamp(0, mouse.max_x.saturating_sub(1));
            mouse.y = (mouse.y + screen_dy).clamp(0, mouse.max_y.saturating_sub(1));
        }
        _ => {
            state.cycle = 0;
        }
    }
}

/// Returns the current mouse position and button state.
pub fn get_state() -> MouseState {
    *MOUSE.lock()
}

/// Moves mouse pointer by relative delta (used for keyboard navigation fallback).
pub fn move_relative(dx: i32, dy: i32) {
    let mut mouse = MOUSE.lock();
    mouse.x = (mouse.x + dx).clamp(0, mouse.max_x.saturating_sub(1));
    mouse.y = (mouse.y + dy).clamp(0, mouse.max_y.saturating_sub(1));
}

/// Simulates a mouse button press or release (keyboard navigation fallback).
pub fn simulate_click(left: bool, right: bool) {
    let mut mouse = MOUSE.lock();
    mouse.left_button = left;
    mouse.right_button = right;
}
