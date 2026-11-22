//! Settings & Hardware Configuration Application for JainOS.
//!
//! Provides display resolution switching (1024x768 / 800x600),
//! hardware architecture inspection, and CMOS time synchronization controls.

use alloc::string::String;
use crate::gui::canvas::Canvas;

pub struct SettingsApp {
    pub status_msg: String,
}

impl SettingsApp {
    pub fn new() -> Self {
        Self {
            status_msg: String::from("Display mode: 1024x768x32 True Color (Active)"),
        }
    }

    /// Handles mouse clicks in Settings window.
    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32) -> Option<(usize, usize)> {
        // [ 1024 x 768 ] button at (24, 60, 150, 30)
        if rel_x >= 24 && rel_x < 174 && rel_y >= 60 && rel_y < 90 {
            self.status_msg = String::from("Switching to 1024x768x32...");
            return Some((1024, 768));
        }

        // [ 800 x 600 ] button at (186, 60, 150, 30)
        if rel_x >= 186 && rel_x < 336 && rel_y >= 60 && rel_y < 90 {
            self.status_msg = String::from("Switching to 800x600x32...");
            return Some((800, 600));
        }

        None
    }

    /// Renders Settings UI.
    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        // Background
        canvas.fill_rect(cx, cy, cw, ch, 0x000F172A);

        // Section 1: Display Resolution Settings Box
        let box1_y = cy + 12;
        let box1_h = 100;
        canvas.fill_rect(cx + 12, box1_y, cw - 24, box1_h, 0x001E293B);
        canvas.draw_rect(cx + 12, box1_y, cw - 24, box1_h, 0x00334155);

        canvas.draw_string(cx + 24, box1_y + 10, "Display Resolution (Bochs / VirtualBox BGA)", 0x0038BDF8);

        // Resolution buttons
        // Button 1: 1024 x 768
        canvas.draw_gradient_v(cx + 24, box1_y + 36, 140, 28, 0x002563EB, 0x001D4ED8);
        canvas.draw_rect(cx + 24, box1_y + 36, 140, 28, 0x0060A5FA);
        canvas.draw_string(cx + 36, box1_y + 42, "1024 x 768 (HD)", 0x00FFFFFF);

        // Button 2: 800 x 600
        canvas.draw_gradient_v(cx + 176, box1_y + 36, 140, 28, 0x00334155, 0x001E293B);
        canvas.draw_rect(cx + 176, box1_y + 36, 140, 28, 0x00475569);
        canvas.draw_string(cx + 188, box1_y + 42, "800 x 600 (SVGA)", 0x00E2E8F0);

        canvas.draw_string(cx + 24, box1_y + 74, &self.status_msg, 0x004ADE80);

        // Section 2: Hardware Architecture Details Box
        let box2_y = box1_y + box1_h + 12;
        let box2_h = cy + ch - box2_y - 12;
        canvas.fill_rect(cx + 12, box2_y, cw - 24, box2_h, 0x001E293B);
        canvas.draw_rect(cx + 12, box2_y, cw - 24, box2_h, 0x00334155);

        canvas.draw_string(cx + 24, box2_y + 10, "Hardware Architecture & Driver Specifications", 0x0038BDF8);

        let lines = [
            "Processor:        x86_64 Long Mode (64-bit Freestanding)",
            "Kernel Type:      Monolithic Educational Micro-Kernel",
            "Language:         Pure Rust (#![no_std], #![no_main])",
            "Graphics Adapter: Bochs Graphics Adapter (BGA 32 bpp LFB)",
            "Network Adapter:  Intel 82540EM Gigabit Ethernet (PCI 00:03.0)",
            "Input Devices:    PS/2 Keyboard (IRQ 1) & PS/2 Mouse (IRQ 12)",
            "Timers:           8254 PIT (100 Hz) & CMOS RTC (I/O 0x70/0x71)",
            "Interrupts:       Dual 8259 PIC (Remapped Vectors 32..47)",
        ];

        let mut ly = box2_y + 34;
        for line in lines.iter() {
            if ly + 16 > cy + ch {
                break;
            }
            canvas.draw_string(cx + 24, ly, line, 0x00CBD5E1);
            ly += 20;
        }
    }
}
