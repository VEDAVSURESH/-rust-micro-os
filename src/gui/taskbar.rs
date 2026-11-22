//! Taskbar, System Tray, Live Clock & Start Menu for JainOS.
//!
//! Provides the bottom dock with Start menu toggle, active window tabs,
//! live network link telemetry, heap memory gauge, and real-time CMOS clock.

use crate::gui::canvas::Canvas;
use crate::gui::wallpaper::AppIconType;
use alloc::format;
use alloc::string::String;

pub const TASKBAR_HEIGHT: usize = 36;
pub const START_MENU_WIDTH: i32 = 220;
pub const START_MENU_HEIGHT: i32 = 280;

#[derive(Debug, Clone)]
pub struct TaskbarTab {
    pub window_id: usize,
    pub title: String,
    pub is_focused: bool,
    pub is_minimized: bool,
    pub x: i32,
    pub w: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskbarAction {
    ToggleStartMenu,
    SelectWindow(usize),
    LaunchApp(AppIconType),
    Reboot,
    None,
}

pub struct StartMenuItem {
    pub label: &'static str,
    pub icon_type: Option<AppIconType>,
    pub is_reboot: bool,
}

pub const START_ITEMS: [StartMenuItem; 8] = [
    StartMenuItem { label: "Terminal", icon_type: Some(AppIconType::Terminal), is_reboot: false },
    StartMenuItem { label: "Network Center", icon_type: Some(AppIconType::NetworkCenter), is_reboot: false },
    StartMenuItem { label: "Web Browser", icon_type: Some(AppIconType::WebBrowser), is_reboot: false },
    StartMenuItem { label: "File Manager", icon_type: Some(AppIconType::FileManager), is_reboot: false },
    StartMenuItem { label: "System Monitor", icon_type: Some(AppIconType::SystemMonitor), is_reboot: false },
    StartMenuItem { label: "Settings", icon_type: Some(AppIconType::Settings), is_reboot: false },
    StartMenuItem { label: "About JainOS", icon_type: Some(AppIconType::About), is_reboot: false },
    StartMenuItem { label: "Reboot System", icon_type: None, is_reboot: true },
];

/// Draws the bottom taskbar, open application buttons, and system tray.
pub fn draw_taskbar(
    canvas: &mut Canvas,
    width: usize,
    height: usize,
    tabs: &[TaskbarTab],
    start_menu_open: bool,
) {
    let bar_y = (height - TASKBAR_HEIGHT) as i32;

    // 1. Taskbar background gradient with crisp top highlight line
    canvas.draw_gradient_v(0, bar_y, width as i32, TASKBAR_HEIGHT as i32, 0x000F172A, 0x00090D16);
    canvas.fill_rect(0, bar_y, width as i32, 1, 0x00334155);

    // 2. Start Button
    let start_x = 6;
    let start_y = bar_y + 4;
    let start_w = 94;
    let start_h = 28;

    let (btn_c1, btn_c2, border_col) = if start_menu_open {
        (0x001E40AF, 0x001D4ED8, 0x0060A5FA)
    } else {
        (0x002563EB, 0x001D4ED8, 0x003B82F6)
    };

    canvas.draw_gradient_v(start_x, start_y, start_w, start_h, btn_c1, btn_c2);
    canvas.draw_rect(start_x, start_y, start_w, start_h, border_col);

    // Small JainOS blue diamond emblem on Start button
    canvas.fill_rect(start_x + 8, start_y + 8, 12, 12, 0x0038BDF8);
    canvas.draw_string(start_x + 26, start_y + 6, "JainOS", 0x00FFFFFF);

    // 3. Open Window Tabs
    let mut tab_x = start_x + start_w + 12;
    for tab in tabs {
        let tab_w = 140;
        let tab_h = 28;
        let tab_y = bar_y + 4;

        let (tc1, tc2, t_border, text_color) = if tab.is_focused && !tab.is_minimized {
            (0x001E3A8A, 0x002563EB, 0x0060A5FA, 0x00FFFFFF)
        } else if tab.is_minimized {
            (0x000F172A, 0x001E293B, 0x00334155, 0x0094A3B8)
        } else {
            (0x001E293B, 0x00334155, 0x00475569, 0x00E2E8F0)
        };

        canvas.draw_gradient_v(tab_x, tab_y, tab_w, tab_h, tc1, tc2);
        canvas.draw_rect(tab_x, tab_y, tab_w, tab_h, t_border);

        // Status indicator dot
        let dot_color = if tab.is_focused { 0x0038BDF8 } else { 0x0064748B };
        canvas.fill_rect(tab_x + 6, tab_y + 11, 6, 6, dot_color);

        // Title clipped to fit button
        let mut display_title = tab.title.clone();
        if display_title.len() > 13 {
            display_title.truncate(11);
            display_title.push_str("..");
        }
        canvas.draw_string(tab_x + 16, tab_y + 6, &display_title, text_color);

        tab_x += tab_w + 4;
    }

    // 4. System Tray (Right side)
    draw_system_tray(canvas, width, bar_y);

    // 5. Start Menu (if open)
    if start_menu_open {
        draw_start_menu(canvas, height);
    }
}

/// Renders the system tray on the right side of the taskbar.
fn draw_system_tray(canvas: &mut Canvas, width: usize, bar_y: i32) {
    let tray_y = bar_y + 4;

    // A. Read live Real-Time Clock (CMOS RTC)
    let dt = crate::cmos::read_datetime();
    let time_str = format!("{:02}:{:02}:{:02} | {:04}-{:02}-{:02}", dt.hours, dt.minutes, dt.seconds, dt.year, dt.month, dt.day);
    let time_w = (time_str.len() * 8) as i32 + 16;
    let time_x = (width as i32) - time_w - 8;

    canvas.fill_rect(time_x, tray_y, time_w, 28, 0x001E293B);
    canvas.draw_rect(time_x, tray_y, time_w, 28, 0x00334155);
    canvas.draw_string(time_x + 8, tray_y + 6, &time_str, 0x00F8FAFC);

    // B. Live Kernel Heap Memory Gauge
    let stats = crate::allocator::ALLOCATOR.stats();
    let used_kb = stats.total_allocated_bytes / 1024;
    let total_mb = stats.heap_size / (1024 * 1024);
    let mem_str = format!("RAM: {}K/{}M", used_kb, total_mb);
    let mem_w = (mem_str.len() * 8) as i32 + 16;
    let mem_x = time_x - mem_w - 6;

    canvas.fill_rect(mem_x, tray_y, mem_w, 28, 0x001E293B);
    canvas.draw_rect(mem_x, tray_y, mem_w, 28, 0x00334155);
    canvas.draw_string(mem_x + 8, tray_y + 6, &mem_str, 0x0038BDF8);

    // C. Live Network Status Indicator
    let net_cfg = *crate::net::NET_CONFIG.lock();
    let (net_str, dot_color) = if net_cfg.configured {
        (
            format!("ETH: {}.{}.{}.{}", net_cfg.ip[0], net_cfg.ip[1], net_cfg.ip[2], net_cfg.ip[3]),
            0x0022C55E, // Green active link
        )
    } else {
        (
            String::from("ETH: Offline"),
            0x00EF4444, // Red offline
        )
    };

    let net_w = (net_str.len() * 8) as i32 + 24;
    let net_x = mem_x - net_w - 6;

    canvas.fill_rect(net_x, tray_y, net_w, 28, 0x001E293B);
    canvas.draw_rect(net_x, tray_y, net_w, 28, 0x00334155);
    // Connection dot
    canvas.fill_rect(net_x + 8, tray_y + 10, 8, 8, dot_color);
    canvas.draw_string(net_x + 20, tray_y + 6, &net_str, 0x00E2E8F0);
}

/// Renders the popup Start Menu with user header and application launch items.
fn draw_start_menu(canvas: &mut Canvas, screen_height: usize) {
    let menu_x = 6;
    let menu_y = (screen_height as i32) - (TASKBAR_HEIGHT as i32) - START_MENU_HEIGHT - 4;

    // Drop shadow
    canvas.fill_rect(menu_x + 4, menu_y + 4, START_MENU_WIDTH, START_MENU_HEIGHT, 0x00050810);

    // Main frame
    canvas.fill_rect(menu_x, menu_y, START_MENU_WIDTH, START_MENU_HEIGHT, 0x000F172A);
    canvas.draw_rect(menu_x, menu_y, START_MENU_WIDTH, START_MENU_HEIGHT, 0x0038BDF8);

    // User Profile Header Banner
    canvas.draw_gradient_v(menu_x + 1, menu_y + 1, START_MENU_WIDTH - 2, 48, 0x001E40AF, 0x001D4ED8);
    // User avatar icon
    canvas.fill_rect(menu_x + 10, menu_y + 8, 32, 32, 0x0038BDF8);
    canvas.draw_string(menu_x + 20, menu_y + 16, "V", 0x000F172A);

    canvas.draw_string(menu_x + 48, menu_y + 10, "vedav@jainos", 0x00FFFFFF);
    canvas.draw_string(menu_x + 48, menu_y + 26, "Lead Developer", 0x0093C5FD);

    canvas.fill_rect(menu_x + 1, menu_y + 49, START_MENU_WIDTH - 2, 1, 0x00334155);

    // Start Menu Items
    let item_y_start = menu_y + 54;
    let item_h = 26;

    for (idx, item) in START_ITEMS.iter().enumerate() {
        let iy = item_y_start + (idx as i32) * item_h;

        if item.is_reboot {
            // Divider before reboot
            canvas.fill_rect(menu_x + 8, iy - 2, START_MENU_WIDTH - 16, 1, 0x00334155);
            canvas.draw_string(menu_x + 16, iy + 4, "x", 0x00EF4444);
            canvas.draw_string(menu_x + 32, iy + 4, item.label, 0x00F87171);
        } else {
            canvas.draw_string(menu_x + 16, iy + 4, ">", 0x0038BDF8);
            canvas.draw_string(menu_x + 32, iy + 4, item.label, 0x00F1F5F9);
        }
    }
}

/// Evaluates mouse clicks on the taskbar or open start menu.
pub fn handle_taskbar_click(
    mx: i32,
    my: i32,
    _screen_width: usize,
    screen_height: usize,
    tabs: &[TaskbarTab],
    start_menu_open: bool,
) -> TaskbarAction {
    let bar_y = (screen_height - TASKBAR_HEIGHT) as i32;

    // Check Start Menu clicks if open
    if start_menu_open {
        let menu_x = 6;
        let menu_y = (screen_height as i32) - (TASKBAR_HEIGHT as i32) - START_MENU_HEIGHT - 4;

        if mx >= menu_x && mx < menu_x + START_MENU_WIDTH && my >= menu_y && my < menu_y + START_MENU_HEIGHT {
            let item_y_start = menu_y + 54;
            let item_h = 26;

            for (idx, item) in START_ITEMS.iter().enumerate() {
                let iy = item_y_start + (idx as i32) * item_h;
                if my >= iy && my < iy + item_h {
                    if item.is_reboot {
                        return TaskbarAction::Reboot;
                    } else if let Some(app) = item.icon_type {
                        return TaskbarAction::LaunchApp(app);
                    }
                }
            }
            return TaskbarAction::None;
        }
    }

    // Check Taskbar clicks
    if my >= bar_y && my < (screen_height as i32) {
        // Start Button
        if mx >= 6 && mx < 100 {
            return TaskbarAction::ToggleStartMenu;
        }

        // Window Tabs
        let mut tab_x = 6 + 94 + 12;
        for tab in tabs {
            let tab_w = 140;
            if mx >= tab_x && mx < tab_x + tab_w {
                return TaskbarAction::SelectWindow(tab.window_id);
            }
            tab_x += tab_w + 4;
        }
    }

    TaskbarAction::None
}
