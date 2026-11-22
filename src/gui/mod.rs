//! JainOS Native Graphical Desktop Environment & Window Manager (v0.3.4).
//!
//! Features:
//! - Full double-buffered linear framebuffer composition (Bochs BGA True Color)
//! - Multi-window manager with active dragging, minimization, and z-ordering
//! - Sleek dark navy / electric blue branding and custom wallpaper
//! - High-precision custom mouse pointer with IRQ 12 hardware tracking & keyboard fallback
//! - Bottom taskbar with Start Menu, active window dock, live RTC clock, and network telemetry
//! - Complete suite of 8 native applications (Terminal, Network, Browser, Files, Games, SysMon, Settings, About)

pub mod apps;
pub mod canvas;
pub mod font;
pub mod taskbar;
pub mod wallpaper;
pub mod window;

use alloc::string::String;
use alloc::vec::Vec;
use crate::bga::{self, DISPLAY};
use crate::gui::apps::about::AboutApp;
use crate::gui::apps::browser::BrowserApp;
use crate::gui::apps::file_manager::FileManagerApp;
use crate::gui::apps::games::GamesApp;
use crate::gui::apps::network::NetworkApp;
use crate::gui::apps::settings::SettingsApp;
use crate::gui::apps::system_monitor::SystemMonitorApp;
use crate::gui::apps::terminal::TerminalApp;
use crate::gui::canvas::Canvas;
use crate::gui::taskbar::{handle_taskbar_click, TaskbarAction, TaskbarTab};
use crate::gui::wallpaper::{hit_test_icon, AppIconType};
use crate::gui::window::{Window, WindowHitResult};
use crate::sync::Spinlock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKind {
    Terminal,
    Network,
    WebBrowser,
    FileManager,
    Games,
    SystemMonitor,
    Settings,
    About,
}

pub struct DesktopWindow {
    pub win: Window,
    pub kind: AppKind,
}

pub struct Desktop {
    pub canvas: Canvas,
    pub windows: Vec<DesktopWindow>,
    pub next_win_id: usize,
    pub start_menu_open: bool,
    pub prev_left_button: bool,
    pub dragging_win_idx: Option<usize>,

    // App instances
    pub terminal: TerminalApp,
    pub network: NetworkApp,
    pub browser: BrowserApp,
    pub file_manager: FileManagerApp,
    pub games: GamesApp,
    pub system_monitor: SystemMonitorApp,
    pub settings: SettingsApp,
    pub about: AboutApp,
}

pub static DESKTOP: Spinlock<Option<Desktop>> = Spinlock::new(None);

impl Desktop {
    pub fn new(width: usize, height: usize) -> Self {
        let mut d = Self {
            canvas: Canvas::new(width, height),
            windows: Vec::new(),
            next_win_id: 1,
            start_menu_open: false,
            prev_left_button: false,
            dragging_win_idx: None,
            terminal: TerminalApp::new(),
            network: NetworkApp::new(),
            browser: BrowserApp::new(),
            file_manager: FileManagerApp::new(),
            games: GamesApp::new(),
            system_monitor: SystemMonitorApp::new(),
            settings: SettingsApp::new(),
            about: AboutApp::new(),
        };

        // Open default startup windows: Terminal and Network Center
        d.open_app(AppKind::Network);
        d.open_app(AppKind::Terminal);

        d
    }

    /// Brings an application window to the front or creates it if closed.
    pub fn open_app(&mut self, kind: AppKind) {
        // Check if window already exists
        for idx in 0..self.windows.len() {
            if self.windows[idx].kind == kind {
                self.windows[idx].win.is_minimized = false;
                self.focus_window_idx(idx);
                return;
            }
        }

        // Create new window
        let id = self.next_win_id;
        self.next_win_id += 1;

        let (title, x, y, w, h) = match kind {
            AppKind::Terminal => ("Terminal - JainOS Shell (vedav)", 120, 60, 560, 360),
            AppKind::Network => ("Network - Connection & Wi-Fi Control Center", 110, 45, 740, 500),
            AppKind::WebBrowser => ("Web Browser - JainOS HTML Web Client", 130, 45, 740, 500),
            AppKind::FileManager => ("File Manager - Virtual File System", 160, 90, 540, 370),
            AppKind::Games => ("JainOS Arcade - Retro 4-Game Suite", 170, 75, 560, 380),
            AppKind::SystemMonitor => ("System Monitor - Kernel Telemetry", 210, 100, 520, 380),
            AppKind::Settings => ("Settings - System Configuration", 230, 120, 480, 320),
            AppKind::About => ("About JainOS - Micro Operating System", 200, 100, 540, 360),
        };

        let mut win = Window::new(id, String::from(title), x, y, w, h);
        win.is_focused = true;

        // Unfocus all other windows
        for w in self.windows.iter_mut() {
            w.win.is_focused = false;
        }

        self.windows.push(DesktopWindow { win, kind });
    }

    /// Moves a window to the top of the z-order and focuses it.
    pub fn focus_window_idx(&mut self, idx: usize) {
        if idx >= self.windows.len() {
            return;
        }
        for (i, w) in self.windows.iter_mut().enumerate() {
            w.win.is_focused = i == idx;
        }
        let w = self.windows.remove(idx);
        self.windows.push(w);
    }

    /// Changes screen resolution dynamically via BGA.
    pub fn switch_resolution(&mut self, width: usize, height: usize) {
        if bga::set_video_mode(width, height, 32).is_ok() {
            self.canvas.resize(width, height);
            crate::mouse::set_screen_bounds(width as i32, height as i32);
        }
    }

    /// Processes keyboard, mouse, and redraws the desktop frame.
    pub fn update_and_render(&mut self) {
        let mouse = crate::mouse::get_state();
        let mouse_clicked = mouse.left_button && !self.prev_left_button;
        let mouse_released = !mouse.left_button && self.prev_left_button;
        self.prev_left_button = mouse.left_button;

        // 1. Process Keyboard Input & Hotkeys
        while let Some(c) = crate::keyboard::read_char() {
            match c {
                '\u{F1}' => self.open_app(AppKind::Terminal),
                '\u{F2}' => self.open_app(AppKind::Network),
                '\u{F3}' => self.open_app(AppKind::WebBrowser),
                '\u{F4}' => self.open_app(AppKind::FileManager),
                '\u{F5}' => self.open_app(AppKind::Games),
                '\u{F6}' => self.open_app(AppKind::SystemMonitor),
                '\u{F7}' => self.open_app(AppKind::Settings),
                '\u{F8}' => self.open_app(AppKind::About),
                '\u{F9}' => self.start_menu_open = !self.start_menu_open,
                '\t' => {
                    // If the active front window is Network, route Tab to toggle password field focus
                    if let Some(top_win) = self.windows.last_mut() {
                        if !top_win.win.is_minimized && top_win.kind == AppKind::Network {
                            self.network.handle_char('\t');
                            continue;
                        }
                    }
                    // Cycle window focus
                    if self.windows.len() > 1 {
                        let first = self.windows.remove(0);
                        self.windows.push(first);
                        let top_idx = self.windows.len() - 1;
                        self.focus_window_idx(top_idx);
                    }
                }
                c => {
                    // Send keystroke to focused window
                    if let Some(top_win) = self.windows.last_mut() {
                        if !top_win.win.is_minimized {
                            match top_win.kind {
                                AppKind::Terminal => self.terminal.handle_char(c),
                                AppKind::Network => self.network.handle_char(c),
                                AppKind::WebBrowser => self.browser.handle_char(c),
                                AppKind::Games => self.games.handle_char(c),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // Process any application launch requested from terminal commands
        if let Some(kind) = self.terminal.pending_launch.take() {
            self.open_app(kind);
        }

        // 2. Handle Mouse Dragging
        if mouse.left_button {
            if let Some(idx) = self.dragging_win_idx {
                if idx < self.windows.len() {
                    let w = &mut self.windows[idx].win;
                    w.x = (mouse.x - w.drag_offset_x).clamp(0, self.canvas.width as i32 - 100);
                    w.y = (mouse.y - w.drag_offset_y).clamp(0, self.canvas.height as i32 - 40);
                }
            }
        }

        if mouse_released {
            if let Some(idx) = self.dragging_win_idx {
                if idx < self.windows.len() {
                    self.windows[idx].win.is_dragging = false;
                }
            }
            self.dragging_win_idx = None;
        }

        // 3. Handle Mouse Clicks
        if mouse_clicked {
            let mut click_handled = false;

            // A. Check Taskbar and Start Menu Clicks
            let tabs: Vec<TaskbarTab> = self
                .windows
                .iter()
                .map(|w| TaskbarTab {
                    window_id: w.win.id,
                    title: w.win.title.clone(),
                    is_focused: w.win.is_focused,
                    is_minimized: w.win.is_minimized,
                    x: 0,
                    w: 0,
                })
                .collect();

            let task_action = handle_taskbar_click(
                mouse.x,
                mouse.y,
                self.canvas.width,
                self.canvas.height,
                &tabs,
                self.start_menu_open,
            );

            match task_action {
                TaskbarAction::ToggleStartMenu => {
                    self.start_menu_open = !self.start_menu_open;
                    click_handled = true;
                }
                TaskbarAction::SelectWindow(win_id) => {
                    for (idx, w) in self.windows.iter_mut().enumerate() {
                        if w.win.id == win_id {
                            if w.win.is_minimized {
                                w.win.is_minimized = false;
                            }
                            self.focus_window_idx(idx);
                            break;
                        }
                    }
                    self.start_menu_open = false;
                    click_handled = true;
                }
                TaskbarAction::LaunchApp(app_icon) => {
                    let kind = match app_icon {
                        AppIconType::Terminal => AppKind::Terminal,
                        AppIconType::Network => AppKind::Network,
                        AppIconType::WebBrowser => AppKind::WebBrowser,
                        AppIconType::FileManager => AppKind::FileManager,
                        AppIconType::Games => AppKind::Games,
                        AppIconType::SystemMonitor => AppKind::SystemMonitor,
                        AppIconType::Settings => AppKind::Settings,
                        AppIconType::About => AppKind::About,
                    };
                    self.open_app(kind);
                    self.start_menu_open = false;
                    click_handled = true;
                }
                TaskbarAction::Reboot => {
                    unsafe {
                        crate::serial::outb(0x64, 0xFE);
                    }
                }
                TaskbarAction::None => {}
            }

            // Close Start Menu if clicked outside
            if !click_handled && self.start_menu_open {
                self.start_menu_open = false;
                click_handled = true;
            }

            // B. Check Window Clicks (From frontmost window to backmost)
            if !click_handled {
                let win_count = self.windows.len();
                let mut clicked_win_idx = None;

                for rev_idx in 0..win_count {
                    let idx = win_count - 1 - rev_idx;
                    let hit = self.windows[idx].win.hit_test(mouse.x, mouse.y);

                    match hit {
                        WindowHitResult::CloseButton => {
                            self.windows.remove(idx);
                            click_handled = true;
                            break;
                        }
                        WindowHitResult::MinimizeButton => {
                            self.windows[idx].win.is_minimized = true;
                            self.windows[idx].win.is_focused = false;
                            click_handled = true;
                            break;
                        }
                        WindowHitResult::Titlebar => {
                            self.windows[idx].win.is_dragging = true;
                            self.windows[idx].win.drag_offset_x = mouse.x - self.windows[idx].win.x;
                            self.windows[idx].win.drag_offset_y = mouse.y - self.windows[idx].win.y;
                            clicked_win_idx = Some(idx);
                            click_handled = true;
                            break;
                        }
                        WindowHitResult::ClientArea => {
                            let (cx, cy, _, _) = self.windows[idx].win.client_rect();
                            let rel_x = mouse.x - cx;
                            let rel_y = mouse.y - cy;

                            match self.windows[idx].kind {
                                AppKind::Terminal => {}
                                AppKind::Network => self.network.handle_click(rel_x, rel_y),
                                AppKind::WebBrowser => self.browser.handle_click(rel_x, rel_y),
                                AppKind::FileManager => self.file_manager.handle_click(rel_x, rel_y),
                                AppKind::Games => self.games.handle_click(rel_x, rel_y),
                                AppKind::Settings => {
                                    if let Some((nw, nh)) = self.settings.handle_click(rel_x, rel_y) {
                                        self.switch_resolution(nw, nh);
                                    }
                                }
                                AppKind::SystemMonitor | AppKind::About => {}
                            }
                            clicked_win_idx = Some(idx);
                            click_handled = true;
                            break;
                        }
                        WindowHitResult::None => {}
                    }
                }

                if let Some(idx) = clicked_win_idx {
                    self.focus_window_idx(idx);
                    if self.windows.last().map(|w| w.win.is_dragging).unwrap_or(false) {
                        self.dragging_win_idx = Some(self.windows.len() - 1);
                    }
                }
            }

            // C. Check Desktop Icon Clicks
            if !click_handled {
                if let Some(app_type) = hit_test_icon(mouse.x, mouse.y) {
                    let kind = match app_type {
                        AppIconType::Terminal => AppKind::Terminal,
                        AppIconType::Network => AppKind::Network,
                        AppIconType::WebBrowser => AppKind::WebBrowser,
                        AppIconType::FileManager => AppKind::FileManager,
                        AppIconType::Games => AppKind::Games,
                        AppIconType::SystemMonitor => AppKind::SystemMonitor,
                        AppIconType::Settings => AppKind::Settings,
                        AppIconType::About => AppKind::About,
                    };
                    self.open_app(kind);
                }
            }
        }

        // 4. Render Desktop Scene
        let (w, h) = (self.canvas.width, self.canvas.height);

        // A. Background Wallpaper & Icons
        wallpaper::draw_wallpaper(&mut self.canvas, w, h);

        // B. Windows (Rendered in z-order: bottom to top)
        for dwin in self.windows.iter_mut() {
            if !dwin.win.is_minimized {
                dwin.win.draw_frame(&mut self.canvas);
                let (cx, cy, cw, ch) = dwin.win.client_rect();

                match dwin.kind {
                    AppKind::Terminal => self.terminal.draw(&mut self.canvas, cx, cy, cw, ch),
                    AppKind::Network => self.network.draw(&mut self.canvas, cx, cy, cw, ch),
                    AppKind::WebBrowser => self.browser.draw(&mut self.canvas, cx, cy, cw, ch),
                    AppKind::FileManager => self.file_manager.draw(&mut self.canvas, cx, cy, cw, ch),
                    AppKind::Games => self.games.draw(&mut self.canvas, cx, cy, cw, ch),
                    AppKind::SystemMonitor => self.system_monitor.draw(&mut self.canvas, cx, cy, cw, ch),
                    AppKind::Settings => self.settings.draw(&mut self.canvas, cx, cy, cw, ch),
                    AppKind::About => self.about.draw(&mut self.canvas, cx, cy, cw, ch),
                }
            }
        }

        // C. Taskbar & System Tray & Start Menu
        let tabs: Vec<TaskbarTab> = self
            .windows
            .iter()
            .map(|w| TaskbarTab {
                window_id: w.win.id,
                title: w.win.title.clone(),
                is_focused: w.win.is_focused,
                is_minimized: w.win.is_minimized,
                x: 0,
                w: 0,
            })
            .collect();

        taskbar::draw_taskbar(&mut self.canvas, w, h, &tabs, self.start_menu_open);

        // D. High-Visibility Hardware Mouse Pointer
        self.canvas.draw_cursor(mouse.x, mouse.y);

        // E. Blit Back-Buffer to Linear Framebuffer
        if let Some(disp) = *DISPLAY.lock() {
            self.canvas.present(disp.lfb_virt_addr as *mut u32);
        }
    }
}

/// Initializes the native graphical desktop environment.
pub fn init_desktop() -> Result<(), &'static str> {
    if !bga::is_available() {
        return Err("BGA graphics adapter unavailable");
    }

    // Set 1024x768x32 True Color Mode
    bga::set_video_mode(1024, 768, 32)?;

    let desktop = Desktop::new(1024, 768);
    *DESKTOP.lock() = Some(desktop);

    crate::serial_println!("[GUI] JainOS Native Graphical Desktop Initialized (1024x768x32 True Color)");
    Ok(())
}

/// Cooperative task entry point for the desktop GUI compositor.
pub extern "C" fn desktop_task() {
    crate::serial_println!("[TASK] Desktop GUI Compositor task started");

    loop {
        if let Some(desktop) = DESKTOP.lock().as_mut() {
            desktop.update_and_render();
        }

        // Cooperatively yield CPU cycles to background network and kernel tasks
        crate::task::yield_now();
    }
}
