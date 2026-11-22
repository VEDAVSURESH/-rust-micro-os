//! Virtual File System Manager Application for JainOS.
//!
//! Provides an interactive graphical file browser with directory tree navigation,
//! file metadata inspection (size, permissions), and real-time text file preview.

use alloc::format;
use alloc::vec::Vec;
use crate::gui::canvas::Canvas;

#[derive(Debug, Clone, Copy)]
pub struct VfsEntry {
    pub name: &'static str,
    pub dir: &'static str,
    pub size: usize,
    pub is_dir: bool,
    pub content: &'static str,
}

pub const VFS_FILES: [VfsEntry; 14] = [
    // /system
    VfsEntry { name: "kernel.bin", dir: "/system", size: 1424896, is_dir: false, content: "[ELF64 Executable: JainOS x86_64 Bare-Metal Microkernel]" },
    VfsEntry { name: "boot.sys",   dir: "/system", size: 512,     is_dir: false, content: "JainOS Legacy BIOS Boot Sector\nSignature: 0x55AA\nTarget: x86_64 Long Mode" },
    VfsEntry { name: "gdt.sys",    dir: "/system", size: 64,      is_dir: false, content: "Global Descriptor Table & Task State Segment\nDouble Fault Stack: Active" },
    VfsEntry { name: "idt.sys",    dir: "/system", size: 4096,    is_dir: false, content: "Interrupt Descriptor Table (256 vectors)\nIRQ0: PIT Timer (100 Hz)\nIRQ1: PS/2 Keyboard\nIRQ12: PS/2 Mouse" },

    // /network
    VfsEntry { name: "e1000.sys",   dir: "/network", size: 16384, is_dir: false, content: "Intel 82540EM Gigabit Ethernet NIC Driver\nPCI 00:03.0 | Status: 1000 Mbps Full-Duplex" },
    VfsEntry { name: "riftnet.sys", dir: "/network", size: 8192,  is_dir: false, content: "RiftNet Dual-Stack Bridge Controller\nHost Gateway: 10.0.2.2:19820 (TCP Wi-Fi Telemetry)" },
    VfsEntry { name: "dhcp.conf",   dir: "/network", size: 128,   is_dir: false, content: "interface=eth0\nbootp=dynamic\nrequest=ip,subnet,router,dns" },
    VfsEntry { name: "resolv.conf", dir: "/network", size: 64,    is_dir: false, content: "nameserver 10.0.2.3\nsearch jainos.internal\ntimeout 3" },
    VfsEntry { name: "hosts",       dir: "/network", size: 96,    is_dir: false, content: "127.0.0.1   localhost\n10.0.2.15   jainos\n1.1.1.1     one.one.one.one\n142.250.190.46 google.com" },

    // /user/vedav
    VfsEntry { name: "profile.txt", dir: "/user/vedav", size: 256, is_dir: false, content: "User: vedav\nFull Name: Vedav Suresh\nRole: Lead Operating System Developer\nUSN: 25BTRCL280\nSystem: JainOS v0.3.0 (Rust Micro-OS)" },
    VfsEntry { name: "welcome.txt", dir: "/user/vedav", size: 384, is_dir: false, content: "Welcome to JainOS Graphical Desktop v0.3.0!\nBuilt entirely from scratch using pure Rust (#![no_std]).\nFeatures native window management, RiftNet Wi-Fi, games, browser, and shell." },
    VfsEntry { name: "arcade.rs",   dir: "/user/vedav", size: 512, is_dir: false, content: "// JainOS Native Games: Snake, Pong, Tic-Tac-Toe, 2048\npub fn launch_arcade() {\n    serial_println!(\"Enjoy retro games on bare-metal!\");\n}" },

    // /devices
    VfsEntry { name: "eth0",    dir: "/devices", size: 0, is_dir: false, content: "Device Node: Intel PRO/1000 Gigabit Network Adapter (eth0)" },
    VfsEntry { name: "bga0",    dir: "/devices", size: 0, is_dir: false, content: "Device Node: Bochs Graphics Adapter Linear Framebuffer (1024x768x32)" },
];

pub const DIRS: [&str; 4] = ["/system", "/network", "/user/vedav", "/devices"];

pub struct FileManagerApp {
    pub selected_dir_idx: usize,
    pub selected_file_idx: Option<usize>,
}

impl FileManagerApp {
    pub fn new() -> Self {
        Self {
            selected_dir_idx: 2, // Default to /user/vedav
            selected_file_idx: Some(1), // Default to welcome.txt
        }
    }

    /// Handles mouse clicks in directory tree, back button, or file list.
    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32) {
        // [ < Back ] button at (8, 4, 60, 24)
        if rel_x >= 8 && rel_x <= 68 && rel_y >= 4 && rel_y <= 28 {
            if self.selected_dir_idx > 0 {
                self.selected_dir_idx -= 1;
            } else {
                self.selected_dir_idx = DIRS.len() - 1;
            }
            self.selected_file_idx = None;
            return;
        }

        // Directory sidebar: rel_x < 140
        if rel_x < 140 && rel_y >= 36 && rel_y < 160 {
            let clicked_idx = ((rel_y - 36) / 26) as usize;
            if clicked_idx < DIRS.len() {
                self.selected_dir_idx = clicked_idx;
                self.selected_file_idx = None;
            }
        }

        // File list: rel_x >= 140 && rel_y < 190
        if rel_x >= 140 && rel_y >= 36 && rel_y < 190 {
            let clicked_file = ((rel_y - 36) / 24) as usize;
            let current_dir = DIRS[self.selected_dir_idx];
            let matching_files: Vec<usize> = VFS_FILES
                .iter()
                .enumerate()
                .filter(|(_, f)| f.dir == current_dir)
                .map(|(idx, _)| idx)
                .collect();

            if clicked_file < matching_files.len() {
                self.selected_file_idx = Some(matching_files[clicked_file]);
            }
        }
    }

    /// Renders File Manager UI.
    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        // Top breadcrumb / path bar
        canvas.fill_rect(cx, cy, cw, 32, 0x001E293B);
        canvas.fill_rect(cx, cy + 32, cw, 1, 0x00334155);

        // [ < Back ] button
        canvas.draw_gradient_v(cx + 8, cy + 4, 56, 24, 0x002563EB, 0x001D4ED8);
        canvas.draw_rect(cx + 8, cy + 4, 56, 24, 0x0060A5FA);
        canvas.draw_string(cx + 14, cy + 8, "< Back", 0x00FFFFFF);

        let cur_path = format!("Path: {}", DIRS[self.selected_dir_idx]);
        canvas.draw_string(cx + 74, cy + 8, &cur_path, 0x0038BDF8);

        // Split pane 1: Directory Tree (Left 140px)
        let sidebar_w = 140;
        let main_h = 160;
        canvas.fill_rect(cx, cy + 33, sidebar_w, main_h, 0x000F172A);
        canvas.fill_rect(cx + sidebar_w, cy + 33, 1, main_h, 0x00334155);

        let mut dy = cy + 40;
        for (idx, dir) in DIRS.iter().enumerate() {
            let (bg, fg) = if idx == self.selected_dir_idx {
                (0x002563EB, 0x00FFFFFF)
            } else {
                (0x000F172A, 0x0094A3B8)
            };
            canvas.fill_rect(cx + 4, dy - 2, sidebar_w - 8, 22, bg);
            canvas.draw_string(cx + 8, dy, dir, fg);
            dy += 26;
        }

        // Split pane 2: File List (Right of sidebar)
        let file_x = cx + sidebar_w + 1;
        let file_w = cw - sidebar_w - 1;
        canvas.fill_rect(file_x, cy + 33, file_w, main_h, 0x00131B2E);

        // Header
        canvas.fill_rect(file_x, cy + 33, file_w, 20, 0x001E293B);
        canvas.draw_string(file_x + 10, cy + 36, "Name", 0x0094A3B8);
        canvas.draw_string(file_x + 150, cy + 36, "Size", 0x0094A3B8);
        canvas.draw_string(file_x + 230, cy + 36, "Type", 0x0094A3B8);

        let cur_dir = DIRS[self.selected_dir_idx];
        let mut fy = cy + 58;
        for (idx, file) in VFS_FILES.iter().enumerate().filter(|(_, f)| f.dir == cur_dir) {
            let is_sel = self.selected_file_idx == Some(idx);
            let bg = if is_sel { 0x001E3A8A } else { 0x00131B2E };
            let fg = if is_sel { 0x0038BDF8 } else { 0x00F1F5F9 };

            canvas.fill_rect(file_x + 4, fy - 2, file_w - 8, 20, bg);
            canvas.draw_string(file_x + 10, fy, file.name, fg);
            let size_str = format!("{} B", file.size);
            canvas.draw_string(file_x + 150, fy, &size_str, 0x00CBD5E1);
            let type_str = if file.name.ends_with(".bin") || file.name.ends_with(".sys") { "System" } else { "Text" };
            canvas.draw_string(file_x + 230, fy, type_str, 0x0094A3B8);

            fy += 24;
        }

        // Split pane 3: Bottom Text Preview Pane
        let preview_y = cy + 33 + main_h;
        let preview_h = ch - 33 - main_h;
        canvas.fill_rect(cx, preview_y, cw, 1, 0x00334155);
        canvas.fill_rect(cx, preview_y + 1, cw, 22, 0x001E293B);
        canvas.draw_string(cx + 12, preview_y + 4, "File Content Preview:", 0x0094A3B8);

        canvas.fill_rect(cx, preview_y + 23, cw, preview_h - 23, 0x000A0E17);

        if let Some(f_idx) = self.selected_file_idx {
            let file = &VFS_FILES[f_idx];
            let mut py = preview_y + 30;
            for line in file.content.lines() {
                if py + 16 > cy + ch {
                    break;
                }
                canvas.draw_string(cx + 16, py, line, 0x00E2E8F0);
                py += 18;
            }
        } else {
            canvas.draw_string(cx + 16, preview_y + 32, "(Select a file to preview contents)", 0x0064748B);
        }
    }
}
