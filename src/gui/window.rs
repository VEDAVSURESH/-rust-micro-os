//! Window Frame, Titlebar Controls, Dragging & Layering for JainOS GUI.
//!
//! Provides window creation, dragging, focus management, close/minimize buttons,
//! drop-shadows, and client area bounds calculation.

use crate::gui::canvas::Canvas;
use alloc::string::String;

pub const TITLEBAR_HEIGHT: i32 = 26;
pub const BTN_SIZE: i32 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowHitResult {
    None,
    Titlebar,
    CloseButton,
    MinimizeButton,
    ClientArea,
}

#[derive(Debug, Clone)]
pub struct Window {
    pub id: usize,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub is_minimized: bool,
    pub is_focused: bool,
    pub is_dragging: bool,
    pub drag_offset_x: i32,
    pub drag_offset_y: i32,
}

impl Window {
    pub fn new(id: usize, title: String, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            id,
            title,
            x,
            y,
            w,
            h,
            is_minimized: false,
            is_focused: true,
            is_dragging: false,
            drag_offset_x: 0,
            drag_offset_y: 0,
        }
    }

    /// Returns the client rendering area (x, y, w, h) excluding titlebar and borders.
    pub fn client_rect(&self) -> (i32, i32, i32, i32) {
        (
            self.x + 2,
            self.y + TITLEBAR_HEIGHT,
            self.w - 4,
            self.h - TITLEBAR_HEIGHT - 2,
        )
    }

    /// Performs hit-testing on the window given mouse cursor (mx, my).
    pub fn hit_test(&self, mx: i32, my: i32) -> WindowHitResult {
        if self.is_minimized {
            return WindowHitResult::None;
        }

        if mx < self.x || mx >= self.x + self.w || my < self.y || my >= self.y + self.h {
            return WindowHitResult::None;
        }

        // Check if inside title bar
        if my < self.y + TITLEBAR_HEIGHT {
            let close_x = self.x + self.w - 22;
            let min_x = self.x + self.w - 44;

            if mx >= close_x && mx < close_x + BTN_SIZE {
                return WindowHitResult::CloseButton;
            }
            if mx >= min_x && mx < min_x + BTN_SIZE {
                return WindowHitResult::MinimizeButton;
            }
            return WindowHitResult::Titlebar;
        }

        WindowHitResult::ClientArea
    }

    /// Draws the outer window frame, titlebar, controls, and client area background.
    pub fn draw_frame(&self, canvas: &mut Canvas) {
        if self.is_minimized {
            return;
        }

        // 1. Drop shadow behind window
        canvas.fill_rect(self.x + 4, self.y + 4, self.w, self.h, 0x00040711);

        // 2. Window outer border
        let border_color = if self.is_focused { 0x0038BDF8 } else { 0x00475569 };
        canvas.draw_rect(self.x, self.y, self.w, self.h, border_color);

        // 3. Titlebar
        let (tb_c1, tb_c2, text_color) = if self.is_focused {
            (0x001E40AF, 0x002563EB, 0x00FFFFFF)
        } else {
            (0x001E293B, 0x00334155, 0x0094A3B8)
        };

        canvas.draw_gradient_v(self.x + 1, self.y + 1, self.w - 2, TITLEBAR_HEIGHT - 1, tb_c1, tb_c2);

        // Title text
        canvas.draw_string(self.x + 8, self.y + 5, &self.title, text_color);

        // Titlebar action buttons
        let close_x = self.x + self.w - 22;
        let min_x = self.x + self.w - 44;
        let btn_y = self.y + 4;

        // Minimize Button [-]
        canvas.fill_rect(min_x, btn_y, BTN_SIZE, BTN_SIZE, 0x001E293B);
        canvas.draw_rect(min_x, btn_y, BTN_SIZE, BTN_SIZE, 0x00475569);
        canvas.fill_rect(min_x + 4, btn_y + 8, 10, 2, 0x0094A3B8);

        // Close Button [X]
        canvas.fill_rect(close_x, btn_y, BTN_SIZE, BTN_SIZE, 0x00DC2626);
        canvas.draw_rect(close_x, btn_y, BTN_SIZE, BTN_SIZE, 0x00EF4444);
        canvas.draw_string(close_x + 5, btn_y + 1, "x", 0x00FFFFFF);

        // 4. Client area background fill
        let (cx, cy, cw, ch) = self.client_rect();
        canvas.fill_rect(cx, cy, cw, ch, 0x000F172A);
    }
}
