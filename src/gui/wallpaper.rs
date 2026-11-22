//! Custom Wallpaper, Branding Emblem & Desktop Icons for JainOS (v0.3.3).
//!
//! Renders the deep navy gradient background, central JainOS geometric emblem,
//! university/developer credits, and desktop application shortcuts.

use crate::gui::canvas::Canvas;

/// Desktop application shortcut icon specification.
#[derive(Debug, Clone, Copy)]
pub struct DesktopIcon {
    pub id: usize,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub label: &'static str,
    pub icon_type: AppIconType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppIconType {
    Terminal,
    Network,
    WebBrowser,
    FileManager,
    Games,
    SystemMonitor,
    Settings,
    About,
}

pub const DESKTOP_ICONS: [DesktopIcon; 8] = [
    DesktopIcon { id: 1, x: 28, y: 24,  w: 72, h: 58, label: "Terminal", icon_type: AppIconType::Terminal },
    DesktopIcon { id: 2, x: 28, y: 92,  w: 72, h: 58, label: "Network",  icon_type: AppIconType::Network },
    DesktopIcon { id: 3, x: 28, y: 160, w: 72, h: 58, label: "Browser",  icon_type: AppIconType::WebBrowser },
    DesktopIcon { id: 4, x: 28, y: 228, w: 72, h: 58, label: "Files",    icon_type: AppIconType::FileManager },
    DesktopIcon { id: 5, x: 28, y: 296, w: 72, h: 58, label: "Games",    icon_type: AppIconType::Games },
    DesktopIcon { id: 6, x: 28, y: 364, w: 72, h: 58, label: "SysMon",   icon_type: AppIconType::SystemMonitor },
    DesktopIcon { id: 7, x: 28, y: 432, w: 72, h: 58, label: "Settings", icon_type: AppIconType::Settings },
    DesktopIcon { id: 8, x: 28, y: 500, w: 72, h: 58, label: "About",    icon_type: AppIconType::About },
];

/// Checks if mouse coordinates (mx, my) fall within a desktop icon.
pub fn hit_test_icon(mx: i32, my: i32) -> Option<AppIconType> {
    for icon in DESKTOP_ICONS.iter() {
        if mx >= icon.x && mx < icon.x + icon.w && my >= icon.y && my < icon.y + icon.h {
            return Some(icon.icon_type);
        }
    }
    None
}

/// Draws the complete desktop wallpaper, branding emblem, and desktop icons.
pub fn draw_wallpaper(canvas: &mut Canvas, width: usize, height: usize) {
    // 1. Deep navy to midnight blue vertical linear gradient
    canvas.draw_gradient_v(0, 0, width as i32, height as i32 - 36, 0x000B132B, 0x001E293B);

    // 2. Central JainOS Geometric Emblem & Branding
    let cx = width as i32 / 2;
    let cy = height as i32 / 2 - 30;

    // Outer decorative glow diamond
    for r in 0..6 {
        let col = if r % 2 == 0 { 0x001E3A8A } else { 0x002563EB };
        canvas.draw_rect(cx - 70 - r, cy - 70 - r, 140 + r * 2, 140 + r * 2, col);
    }

    // Inner diamond fill
    canvas.fill_rect(cx - 64, cy - 64, 128, 128, 0x000F172A);
    canvas.draw_rect(cx - 64, cy - 64, 128, 128, 0x0038BDF8);
    canvas.draw_rect(cx - 56, cy - 56, 112, 112, 0x001D4ED8);

    // Modern "J" & "OS" geometric logo inside emblem
    canvas.fill_rect(cx - 28, cy - 36, 12, 54, 0x0038BDF8);
    canvas.fill_rect(cx - 42, cy + 6, 26, 12, 0x0038BDF8);
    canvas.fill_rect(cx - 42, cy - 6, 12, 24, 0x0038BDF8);

    canvas.fill_rect(cx + 4, cy - 36, 30, 12, 0x0060A5FA);
    canvas.fill_rect(cx + 4, cy - 14, 30, 10, 0x003B82F6);
    canvas.fill_rect(cx + 4, cy + 8, 30, 12, 0x002563EB);

    // System Branding Typography
    let title = "J A I N O S";
    let title_x = cx - (title.len() as i32 * 8);
    canvas.draw_string(title_x + 1, cy + 91, title, 0x00000000); // Drop shadow
    canvas.draw_string(title_x, cy + 90, title, 0x0038BDF8);

    let subtitle = "Next-Generation Rust Bare-Metal Desktop";
    let sub_x = cx - (subtitle.len() as i32 * 4);
    canvas.draw_string(sub_x, cy + 114, subtitle, 0x0094A3B8);

    let info = "JainOS v0.3.3 • x86_64 Long Mode • Developed by Vedav Suresh";
    let info_x = cx - (info.len() as i32 * 4);
    canvas.draw_string(info_x, cy + 134, info, 0x0064748B);

    // 3. Desktop Application Icons
    for icon in DESKTOP_ICONS.iter() {
        draw_desktop_icon(canvas, icon);
    }
}

/// Renders a single desktop application icon with graphics and label.
fn draw_desktop_icon(canvas: &mut Canvas, icon: &DesktopIcon) {
    // Icon tile backing with subtle border
    canvas.fill_rect(icon.x, icon.y, 48, 38, 0x001E293B);
    canvas.draw_rect(icon.x, icon.y, 48, 38, 0x00334155);

    // Icon internal artwork based on type
    match icon.icon_type {
        AppIconType::Terminal => {
            canvas.fill_rect(icon.x + 4, icon.y + 4, 40, 30, 0x000F172A);
            canvas.draw_rect(icon.x + 4, icon.y + 4, 40, 30, 0x00475569);
            canvas.draw_string(icon.x + 10, icon.y + 12, ">_", 0x004ADE80);
        }
        AppIconType::Network => {
            // Wi-Fi wave arcs & Ethernet connector emblem
            canvas.fill_rect(icon.x + 6, icon.y + 6, 36, 26, 0x000284C7);
            canvas.draw_rect(icon.x + 6, icon.y + 6, 36, 26, 0x0038BDF8);
            // Wi-Fi signal broadcast dot and arcs
            canvas.fill_rect(icon.x + 22, icon.y + 22, 4, 4, 0x0022C55E);
            canvas.draw_rect(icon.x + 17, icon.y + 17, 14, 10, 0x00F8FAFC);
            canvas.draw_rect(icon.x + 12, icon.y + 11, 24, 16, 0x0038BDF8);
        }
        AppIconType::WebBrowser => {
            canvas.fill_rect(icon.x + 6, icon.y + 6, 36, 26, 0x001D4ED8);
            canvas.draw_rect(icon.x + 6, icon.y + 6, 36, 26, 0x0060A5FA);
            canvas.draw_string(icon.x + 10, icon.y + 12, "WWW", 0x00FFFFFF);
        }
        AppIconType::FileManager => {
            canvas.fill_rect(icon.x + 6, icon.y + 10, 36, 22, 0x00D97706);
            canvas.fill_rect(icon.x + 6, icon.y + 6, 14, 6, 0x00F59E0B);
            canvas.draw_rect(icon.x + 6, icon.y + 10, 36, 22, 0x00FCD34D);
            canvas.draw_string(icon.x + 12, icon.y + 14, "VFS", 0x00000000);
        }
        AppIconType::Games => {
            canvas.fill_rect(icon.x + 6, icon.y + 6, 36, 26, 0x007C3AED);
            canvas.draw_rect(icon.x + 6, icon.y + 6, 36, 26, 0x00A78BFA);
            canvas.draw_string(icon.x + 8, icon.y + 12, "+ O", 0x00FACC15);
        }
        AppIconType::SystemMonitor => {
            canvas.fill_rect(icon.x + 6, icon.y + 6, 36, 26, 0x000F172A);
            canvas.draw_rect(icon.x + 6, icon.y + 6, 36, 26, 0x0010B981);
            canvas.fill_rect(icon.x + 10, icon.y + 18, 6, 2, 0x0034D399);
            canvas.fill_rect(icon.x + 16, icon.y + 12, 4, 10, 0x0034D399);
            canvas.fill_rect(icon.x + 20, icon.y + 15, 4, 7, 0x0034D399);
            canvas.fill_rect(icon.x + 24, icon.y + 18, 12, 2, 0x0034D399);
        }
        AppIconType::Settings => {
            canvas.fill_rect(icon.x + 8, icon.y + 6, 32, 26, 0x00475569);
            canvas.draw_rect(icon.x + 8, icon.y + 6, 32, 26, 0x0094A3B8);
            canvas.draw_string(icon.x + 16, icon.y + 12, "*", 0x00E2E8F0);
        }
        AppIconType::About => {
            canvas.fill_rect(icon.x + 8, icon.y + 6, 32, 26, 0x004338CA);
            canvas.draw_rect(icon.x + 8, icon.y + 6, 32, 26, 0x00818CF8);
            canvas.draw_string(icon.x + 20, icon.y + 12, "i", 0x00FFFFFF);
        }
    }

    // Centered text label underneath icon
    let label_w = icon.label.len() as i32 * 8;
    let lx = icon.x + (48 - label_w) / 2;
    canvas.draw_string(lx + 1, icon.y + 43, icon.label, 0x00000000); // Shadow
    canvas.draw_string(lx, icon.y + 42, icon.label, 0x00E2E8F0);
}
