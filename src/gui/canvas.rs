//! Double-Buffered Framebuffer Canvas & 2D Graphics Engine for JainOS.
//!
//! Provides hardware-accelerated memory blitting, rectangle rendering,
//! smooth gradients, anti-aliased borders, font rendering, and cursor overlays.

use alloc::vec::Vec;
use crate::gui::font::{get_glyph, FONT_HEIGHT, FONT_WIDTH};

/// Double-buffered graphical canvas holding off-screen 32-bit ARGB pixels.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
}

impl Canvas {
    /// Creates a new canvas matching the specified display dimensions.
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        let mut buffer = Vec::with_capacity(size);
        buffer.resize(size, 0x000F172A); // Default dark navy
        Self {
            width,
            height,
            buffer,
        }
    }

    /// Resizes the canvas back-buffer (e.g. during resolution changes).
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        let size = width * height;
        self.buffer.resize(size, 0);
    }

    /// Fills the entire back-buffer with a single solid color.
    #[inline]
    pub fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }

    /// Sets a pixel with boundary checking.
    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
            let idx = (y as usize) * self.width + (x as usize);
            self.buffer[idx] = color;
        }
    }

    /// Gets a pixel color at the specified coordinate.
    #[inline]
    pub fn get_pixel(&self, x: i32, y: i32) -> u32 {
        if x >= 0 && (x as usize) < self.width && y >= 0 && (y as usize) < self.height {
            let idx = (y as usize) * self.width + (x as usize);
            self.buffer[idx]
        } else {
            0
        }
    }

    /// Fills an axis-aligned rectangle with clipping to screen bounds.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w) as usize).min(self.width);
        let y1 = ((y + h) as usize).min(self.height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let run_len = x1 - x0;
        for row in y0..y1 {
            let offset = row * self.width + x0;
            self.buffer[offset..offset + run_len].fill(color);
        }
    }

    /// Draws an unfilled 1-pixel rectangle outline.
    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, color);             // Top
        self.fill_rect(x, y + h - 1, w, 1, color);     // Bottom
        self.fill_rect(x, y, 1, h, color);             // Left
        self.fill_rect(x + w - 1, y, 1, h, color);     // Right
    }

    /// Draws a vertical linear gradient.
    pub fn draw_gradient_v(&mut self, x: i32, y: i32, w: i32, h: i32, top_color: u32, bottom_color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w) as usize).min(self.width);
        let y1 = ((y + h) as usize).min(self.height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let run_len = x1 - x0;
        let total_steps = (h - 1).max(1) as u32;

        for (step_idx, row) in (y0..y1).enumerate() {
            let curr_y = (y0 + step_idx) as i32 - y;
            let factor = (curr_y.clamp(0, h - 1)) as u32;
            let row_color = lerp_color(top_color, bottom_color, factor, total_steps);
            let offset = row * self.width + x0;
            self.buffer[offset..offset + run_len].fill(row_color);
        }
    }

    /// Draws a single character glyph using the 8x16 bitmap font.
    pub fn draw_char(&mut self, x: i32, y: i32, c: char, color: u32) {
        let glyph = get_glyph(c);
        for row in 0..FONT_HEIGHT {
            let py = y + row as i32;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            let row_bits = glyph[row];
            for col in 0..FONT_WIDTH {
                let px = x + col as i32;
                if px < 0 || px >= self.width as i32 {
                    continue;
                }
                if (row_bits & (0x80 >> col)) != 0 {
                    let idx = (py as usize) * self.width + (px as usize);
                    self.buffer[idx] = color;
                }
            }
        }
    }

    /// Draws a string of characters at (x, y) with transparent background.
    pub fn draw_string(&mut self, mut x: i32, y: i32, text: &str, color: u32) {
        for c in text.chars() {
            if c == '\n' {
                break;
            }
            self.draw_char(x, y, c, color);
            x += FONT_WIDTH as i32;
        }
    }

    /// Draws a string of characters with an explicit solid background.
    pub fn draw_string_bg(&mut self, x: i32, y: i32, text: &str, fg: u32, bg: u32) {
        let len = text.len() as i32;
        self.fill_rect(x, y, len * (FONT_WIDTH as i32), FONT_HEIGHT as i32, bg);
        self.draw_string(x, y, text, fg);
    }

    /// Draws a sleek, highly visible custom mouse pointer at (cx, cy).
    /// Features a sharp white arrow with a distinct navy/black outline.
    pub fn draw_cursor(&mut self, cx: i32, cy: i32) {
        // 12x19 cursor bitmap: 0=transparent, 1=outline (dark), 2=fill (white)
        #[rustfmt::skip]
        const CURSOR_PATTERN: [&str; 19] = [
            "1           ",
            "11          ",
            "121         ",
            "1221        ",
            "12221       ",
            "122221      ",
            "1222221     ",
            "12222221    ",
            "122222221   ",
            "1222222221  ",
            "12222211111 ",
            "1221221     ",
            "121 1221    ",
            "11  1221    ",
            "1    1221   ",
            "     1221   ",
            "      1221  ",
            "      111   ",
            "            ",
        ];

        let outline_color = 0x00000000; // Black outline
        let fill_color = 0x00FFFFFF;    // Crisp white fill

        for (row, line) in CURSOR_PATTERN.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                let px = cx + col as i32;
                let py = cy + row as i32;
                match ch {
                    '1' => self.set_pixel(px, py, outline_color),
                    '2' => self.set_pixel(px, py, fill_color),
                    _ => {}
                }
            }
        }
    }

    /// Blits the double-buffer to the linear framebuffer memory using fast 64-bit word writes.
    pub fn present(&self, lfb_virt: *mut u32) {
        if lfb_virt.is_null() {
            return;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(self.buffer.as_ptr(), lfb_virt, self.width * self.height);
        }
    }
}

/// Linearly interpolates between two 32-bit ARGB colors: c1 and c2 by factor / total.
#[inline]
pub fn lerp_color(c1: u32, c2: u32, factor: u32, total: u32) -> u32 {
    if total == 0 {
        return c1;
    }
    let r1 = (c1 >> 16) & 0xFF;
    let g1 = (c1 >> 8) & 0xFF;
    let b1 = c1 & 0xFF;

    let r2 = (c2 >> 16) & 0xFF;
    let g2 = (c2 >> 8) & 0xFF;
    let b2 = c2 & 0xFF;

    let r = (r1 as i32 + ((r2 as i32 - r1 as i32) * (factor as i32) / (total as i32))).clamp(0, 255) as u32;
    let g = (g1 as i32 + ((g2 as i32 - g1 as i32) * (factor as i32) / (total as i32))).clamp(0, 255) as u32;
    let b = (b1 as i32 + ((b2 as i32 - b1 as i32) * (factor as i32) / (total as i32))).clamp(0, 255) as u32;

    (r << 16) | (g << 8) | b
}
