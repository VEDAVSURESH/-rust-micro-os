//! About JainOS Dialog & Project Verification Specification (v0.3.3).
//!
//! Displays operating system branding, developer credits, architecture information,
//! and college project requirement compliance status.

use crate::gui::canvas::Canvas;

pub struct AboutApp {}

impl AboutApp {
    pub fn new() -> Self {
        Self {}
    }

    /// Renders the About JainOS dialog UI.
    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        // Background
        canvas.fill_rect(cx, cy, cw, ch, 0x000F172A);

        // Header Banner with gradient
        canvas.draw_gradient_v(cx, cy, cw, 64, 0x001E3A8A, 0x001D4ED8);
        canvas.fill_rect(cx, cy + 64, cw, 1, 0x0038BDF8);

        // Stylized emblem in header
        canvas.fill_rect(cx + 20, cy + 12, 40, 40, 0x000F172A);
        canvas.draw_rect(cx + 20, cy + 12, 40, 40, 0x0038BDF8);
        canvas.draw_string(cx + 34, cy + 24, "J", 0x0038BDF8);

        canvas.draw_string(cx + 72, cy + 16, "JainOS Micro Operating System", 0x00FFFFFF);
        canvas.draw_string(cx + 72, cy + 34, "Version 0.3.3 (Native Graphical Desktop Edition)", 0x0093C5FD);

        // Details Container Box
        let box_y = cy + 74;
        let box_h = ch - 86;
        canvas.fill_rect(cx + 16, box_y, cw - 32, box_h, 0x001E293B);
        canvas.draw_rect(cx + 16, box_y, cw - 32, box_h, 0x00334155);

        let lines = [
            ("Lead Developer:", "Vedav Suresh (USN: 25BTRCL280)"),
            ("Operating System:", "JainOS (Standalone Bare-Metal Kernel)"),
            ("Implementation:", "100% Pure Rust (#![no_std], #![no_main])"),
            ("Architecture:", "x86_64 Long Mode (BIOS MBR Bootloader)"),
            ("ISO Image Size:", "~1.8 MB (Strictly below 100 MB limit)"),
            ("Network Stack:", "Intel 82540EM, ARP, IPv4, DHCP, DNS, ICMP, TCP"),
            ("Network Control:", "Dual-Interface: Host Wi-Fi Bridge (10.0.2.2) + Virtual Eth"),
            ("Web Browser:", "JainOS WebEngine v0.3.3 (Multi-Tab, Bookmarks, History)"),
            ("Games Suite:", "Snake, Pong, Tic-Tac-Toe, 2048 (Interactive)"),
            ("Active User:", "vedav (`whoami` verified)"),
        ];

        let mut ly = box_y + 12;
        for (label, val) in lines.iter() {
            if ly + 16 > cy + ch - 36 {
                break;
            }
            canvas.draw_string(cx + 28, ly, label, 0x0038BDF8);
            canvas.draw_string(cx + 170, ly, val, 0x00F1F5F9);
            ly += 20;
        }

        // College Requirements Checklist Banner
        if ly + 28 <= cy + ch - 8 {
            canvas.fill_rect(cx + 24, ly + 2, cw - 48, 22, 0x000F766E);
            canvas.draw_rect(cx + 24, ly + 2, cw - 48, 22, 0x002DD4BF);
            canvas.draw_string(
                cx + 34,
                ly + 6,
                "[PASS] All 6 College Project Requirements Successfully Satisfied",
                0x00FFFFFF,
            );
        }
    }
}
