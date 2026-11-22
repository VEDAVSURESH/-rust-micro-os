//! Modern Native Web Browser Application for JainOS (v0.3.4).
//!
//! Features:
//! - Multi-tab browsing architecture with tab switching, new tab (+), and close (x)
//! - Professional JainOS Start Page (about:start) with quick-access tiles and search
//! - Local technical information website (about:jainos) with architecture diagram
//! - Bookmarks management: add bookmark, open bookmark, list bookmarks
//! - Browsing history: visit history, reopen previous pages, clear history
//! - Real HTTP/1.1 TCP client over bare-metal socket (`crate::net::http_get`)
//! - Multi-level HTML renderer with word-wrapping (H1-H4, paragraphs, lists, hr, tables)
//! - Clickable hyperlinks with mouse hit-testing & auto-navigation
//! - Script (<script>) and Style (<style>) filter, comment stripping (<!-- ... -->)
//! - Informative error pages (DNS failure, connection timeout, unsupported HTTPS)
//! - Proportional vertical scrollbar with interactive up/down controls

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::gui::canvas::Canvas;

#[derive(Debug, Clone)]
pub struct LinkHitBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub target_url: String,
}

#[derive(Debug, Clone)]
pub enum HtmlBlock {
    Heading1(String),
    Heading2(String),
    Heading3(String),
    Heading4(String),
    Paragraph(String),
    Bullet(String),
    OrderedItem(usize, String),
    Link { text: String, url: String },
    Divider,
    TableHeader(Vec<String>),
    TableRow(Vec<String>),
    EmptyLine,
}

#[derive(Debug, Clone)]
pub struct BrowserTab {
    pub url: String,
    pub page_title: String,
    pub status: String,
    pub history: Vec<String>,
    pub history_idx: usize,
    pub blocks: Vec<HtmlBlock>,
    pub scroll_y: i32,
    pub total_doc_height: i32,
}

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserDrawer {
    None,
    Bookmarks,
    History,
}

pub struct BrowserApp {
    pub tabs: Vec<BrowserTab>,
    pub active_tab: usize,
    pub url: String,
    pub status: String,
    pub page_title: String,
    pub history: Vec<String>,
    pub history_idx: usize,
    pub blocks: Vec<HtmlBlock>,
    pub link_hitboxes: Vec<LinkHitBox>,
    pub scroll_y: i32,
    pub total_doc_height: i32,
    pub is_loading: bool,
    pub bookmarks: Vec<Bookmark>,
    pub global_history: Vec<HistoryEntry>,
    pub drawer: BrowserDrawer,
}

impl BrowserApp {
    pub fn new() -> Self {
        let mut app = Self {
            tabs: Vec::new(),
            active_tab: 0,
            url: String::from("about:start"),
            status: String::from("Ready • JainOS WebEngine v0.3.4"),
            page_title: String::from("Start Page"),
            history: Vec::new(),
            history_idx: 0,
            blocks: Vec::new(),
            link_hitboxes: Vec::new(),
            scroll_y: 0,
            total_doc_height: 500,
            is_loading: false,
            bookmarks: Vec::new(),
            global_history: Vec::new(),
            drawer: BrowserDrawer::None,
        };

        // Initialize default bookmarks
        app.bookmarks.push(Bookmark {
            title: String::from("JainOS Project Info"),
            url: String::from("about:jainos"),
        });
        app.bookmarks.push(Bookmark {
            title: String::from("Example Domain (IANA)"),
            url: String::from("http://example.com/"),
        });
        app.bookmarks.push(Bookmark {
            title: String::from("NeverSSL (HTTP Portal)"),
            url: String::from("http://neverssl.com/"),
        });

        // Initialize Tab 0: Start Page
        app.history.push(String::from("about:start"));
        app.load_start_page();
        let tab0 = BrowserTab {
            url: app.url.clone(),
            page_title: app.page_title.clone(),
            status: app.status.clone(),
            history: app.history.clone(),
            history_idx: app.history_idx,
            blocks: app.blocks.clone(),
            scroll_y: app.scroll_y,
            total_doc_height: app.total_doc_height,
        };
        app.tabs.push(tab0);

        // Record initial history entry
        app.global_history.push(HistoryEntry {
            url: String::from("about:start"),
            title: String::from("JainOS Start Page"),
        });

        app
    }

    /// Saves the active view state into the active tab record.
    pub fn save_active_tab_state(&mut self) {
        if self.active_tab < self.tabs.len() {
            self.tabs[self.active_tab] = BrowserTab {
                url: self.url.clone(),
                page_title: self.page_title.clone(),
                status: self.status.clone(),
                history: self.history.clone(),
                history_idx: self.history_idx,
                blocks: self.blocks.clone(),
                scroll_y: self.scroll_y,
                total_doc_height: self.total_doc_height,
            };
        }
    }

    /// Switches the active view to another tab.
    pub fn switch_tab(&mut self, new_idx: usize) {
        if new_idx < self.tabs.len() && new_idx != self.active_tab {
            self.save_active_tab_state();
            self.active_tab = new_idx;
            let tab = &self.tabs[new_idx];
            self.url = tab.url.clone();
            self.page_title = tab.page_title.clone();
            self.status = tab.status.clone();
            self.history = tab.history.clone();
            self.history_idx = tab.history_idx;
            self.blocks = tab.blocks.clone();
            self.scroll_y = tab.scroll_y;
            self.total_doc_height = tab.total_doc_height;
            self.link_hitboxes.clear();
            self.drawer = BrowserDrawer::None;
        }
    }

    /// Creates and switches to a new tab.
    pub fn new_tab(&mut self, target_url: &str) {
        self.save_active_tab_state();
        let tab = BrowserTab {
            url: String::from(target_url),
            page_title: String::from("New Tab"),
            status: String::from("Ready"),
            history: alloc::vec![String::from(target_url)],
            history_idx: 0,
            blocks: Vec::new(),
            scroll_y: 0,
            total_doc_height: 500,
        };
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.url = String::from(target_url);
        self.page_title = String::from("New Tab");
        self.status = String::from("Ready");
        self.history = alloc::vec![String::from(target_url)];
        self.history_idx = 0;
        self.blocks.clear();
        self.scroll_y = 0;
        self.total_doc_height = 500;
        self.link_hitboxes.clear();
        self.drawer = BrowserDrawer::None;

        if target_url == "about:start" {
            self.load_start_page();
        } else if target_url == "about:jainos" {
            self.load_jainos_doc_page();
        } else {
            self.fetch_url();
        }
    }

    /// Closes the tab at `idx` if there are multiple tabs.
    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() > 1 && idx < self.tabs.len() {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            let tab = &self.tabs[self.active_tab];
            self.url = tab.url.clone();
            self.page_title = tab.page_title.clone();
            self.status = tab.status.clone();
            self.history = tab.history.clone();
            self.history_idx = tab.history_idx;
            self.blocks = tab.blocks.clone();
            self.scroll_y = tab.scroll_y;
            self.total_doc_height = tab.total_doc_height;
            self.link_hitboxes.clear();
            self.drawer = BrowserDrawer::None;
        }
    }

    /// Adds current page to bookmarks.
    pub fn add_current_bookmark(&mut self) {
        if !self.bookmarks.iter().any(|b| b.url == self.url) {
            self.bookmarks.push(Bookmark {
                title: self.page_title.clone(),
                url: self.url.clone(),
            });
            self.status = format!("Bookmarked: {}", self.page_title);
        } else {
            self.status = String::from("Page is already bookmarked");
        }
    }

    /// Clears the global browsing history.
    pub fn clear_history(&mut self) {
        self.global_history.clear();
        self.status = String::from("Browsing history cleared");
    }

    /// Loads the official JainOS Start Page (about:start).
    pub fn load_start_page(&mut self) {
        self.page_title = String::from("Start Page");
        self.status = String::from("Ready • JainOS WebEngine v0.3.4");
        self.blocks.clear();

        self.blocks.push(HtmlBlock::Heading1(String::from("JainOS Web Browser")));
        self.blocks.push(HtmlBlock::Heading3(String::from("Fast • Lightweight • 100% Native Bare-Metal")));
        self.blocks.push(HtmlBlock::Divider);

        self.blocks.push(HtmlBlock::Heading2(String::from("Quick Access Web Portals")));
        self.blocks.push(HtmlBlock::Link {
            text: String::from("➜ Example Domain (http://example.com/) - IANA Standard Test Site"),
            url: String::from("http://example.com/"),
        });
        self.blocks.push(HtmlBlock::Link {
            text: String::from("➜ NeverSSL (http://neverssl.com/) - Unencrypted HTTP Portal Test"),
            url: String::from("http://neverssl.com/"),
        });
        self.blocks.push(HtmlBlock::Link {
            text: String::from("➜ JainOS Architecture & Project Documentation (about:jainos)"),
            url: String::from("about:jainos"),
        });

        self.blocks.push(HtmlBlock::EmptyLine);
        self.blocks.push(HtmlBlock::Heading2(String::from("Saved Bookmarks")));
        if self.bookmarks.is_empty() {
            self.blocks.push(HtmlBlock::Paragraph(String::from("(No bookmarks saved yet)")));
        } else {
            for b in self.bookmarks.clone() {
                self.blocks.push(HtmlBlock::Link {
                    text: format!("★ {} ({})", b.title, b.url),
                    url: b.url,
                });
            }
        }

        self.blocks.push(HtmlBlock::EmptyLine);
        self.blocks.push(HtmlBlock::Heading2(String::from("Technical Specifications")));
        self.blocks.push(HtmlBlock::Bullet(String::from("Platform: x86_64 Long Mode Bare-Metal Microkernel (#![no_std])")));
        self.blocks.push(HtmlBlock::Bullet(String::from("Network Driver: Intel 82540EM Gigabit NIC with Ring Buffers")));
        self.blocks.push(HtmlBlock::Bullet(String::from("Transport: Native RFC 793 TCP Engine with SYN/ACK Flow Control")));
        self.blocks.push(HtmlBlock::Bullet(String::from("Renderer: Lexical HTML Tokenizer with Word-Wrap & Clickable Hyperlinks")));
        self.blocks.push(HtmlBlock::Bullet(String::from("Security Note: Plain HTTP/1.1 is supported. HTTPS/TLS requires TLS accelerator.")));
    }

    /// Loads the local technical information website (about:jainos).
    pub fn load_jainos_doc_page(&mut self) {
        self.page_title = String::from("Welcome to JainOS");
        self.status = String::from("Local Document • 100% Offline");
        self.blocks.clear();

        self.blocks.push(HtmlBlock::Heading1(String::from("Welcome to JainOS")));
        self.blocks.push(HtmlBlock::Heading3(String::from("A Lightweight Standalone Micro-Operating System Written in Rust")));
        self.blocks.push(HtmlBlock::Divider);

        // Architecture Flow
        self.blocks.push(HtmlBlock::Heading2(String::from("Operating System Architecture")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "Hardware  -->  Bootloader (Legacy BIOS)  -->  Rust Kernel (#![no_std])"
        )));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "  -->  Memory & Task Scheduler  -->  Device Drivers (PCI, e1000, BGA, PS/2)"
        )));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "  -->  Networking Stack (ARP, IPv4, DHCP, DNS, TCP)  -->  Graphical Desktop & Native Apps"
        )));

        self.blocks.push(HtmlBlock::EmptyLine);
        self.blocks.push(HtmlBlock::Heading2(String::from("1. About JainOS")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "JainOS is an educational micro-operating system created by Vedav Suresh (USN: 25BTRCL280). \
             It is designed from scratch in pure bare-metal Rust to demonstrate modern systems programming, \
             memory safety without a garbage collector, and lightweight graphical desktop capabilities."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("2. Rust Kernel Core")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "The kernel executes in 64-bit Long Mode with a dedicated Global Descriptor Table (GDT), \
             Task State Segment (TSS) for double-fault handling, and Interrupt Descriptor Table (IDT). \
             Hardware interrupts from the 8259 PIC and 8254 PIT are serviced with zero runtime overhead."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("3. Memory Management & Heap")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "A physical memory bitmap allocator tracks usable 4 KiB frames. A thread-safe locked heap \
             allocator provides dynamic memory allocation for standard collections (Vec, String, Box) \
             under the alloc crate."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("4. Process Scheduling")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "A cooperative task scheduler manages concurrent kernel execution contexts, including the \
             desktop compositor, network polling, and interactive shells, switching contexts seamlessly."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("5. Networking Architecture")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "The networking subsystem includes an Intel 82540EM PCI driver, Ethernet II parsing, \
             ARP table resolution, IPv4 routing, UDP transport, DHCP lease auto-configuration, \
             DNS name resolution over port 53, ICMP echo ping, and an RFC 793 TCP client."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("6. Network Application")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "The Network control center features a dual-interface model: it communicates with the \
             host Windows WLAN helper (10.0.2.2:19820) to scan real Wi-Fi networks while independently \
             monitoring the guest Intel e1000 Gigabit interface (eth0)."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("7. Native Web Browser")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "The JainOS Web Browser features multi-tab navigation, URL omnibox, HTML tokenizer, \
             word wrapping, clickable links, bookmark management, and real HTTP web content retrieval."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("8. Built-in Arcade Games")));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "Includes four interactive retro games: Snake, Pong (with AI paddle), Tic-Tac-Toe \
             (with minimax decision AI), and the sliding tile puzzle 2048."
        )));

        self.blocks.push(HtmlBlock::Heading2(String::from("9. College Requirements Matrix")));
        self.blocks.push(HtmlBlock::TableHeader(alloc::vec![
            String::from("Requirement"),
            String::from("Status"),
            String::from("Specification"),
        ]));
        self.blocks.push(HtmlBlock::TableRow(alloc::vec![
            String::from("Rust Language"),
            String::from("PASSED"),
            String::from("100% Pure Bare-Metal Rust (#![no_std])"),
        ]));
        self.blocks.push(HtmlBlock::TableRow(alloc::vec![
            String::from("ISO Size Limit"),
            String::from("PASSED"),
            String::from("1.85 MB (Strictly below 100 MB limit)"),
        ]));
        self.blocks.push(HtmlBlock::TableRow(alloc::vec![
            String::from("Real Internet"),
            String::from("PASSED"),
            String::from("DHCP, ping 1.1.1.1, DNS, HTTP/1.1 client"),
        ]));
        self.blocks.push(HtmlBlock::TableRow(alloc::vec![
            String::from("whoami Command"),
            String::from("PASSED"),
            String::from("Displays 'vedav'"),
        ]));
        self.blocks.push(HtmlBlock::TableRow(alloc::vec![
            String::from("Dual Compatibility"),
            String::from("PASSED"),
            String::from("QEMU and Oracle VirtualBox 7.x Legacy BIOS"),
        ]));

        self.blocks.push(HtmlBlock::EmptyLine);
        self.blocks.push(HtmlBlock::Link {
            text: String::from("➜ Click here to return to the Start Page"),
            url: String::from("about:start"),
        });
    }

    /// Handles keyboard input when URL omnibox has focus.
    pub fn handle_char(&mut self, c: char) {
        match c {
            '\n' => {
                let target = self.url.clone();
                self.navigate_to(target);
            }
            '\x08' => {
                self.url.pop();
            }
            c if c >= ' ' && c <= '~' => {
                if self.url.len() < 75 {
                    self.url.push(c);
                }
            }
            _ => {}
        }
    }

    /// Navigates to a target URL, updating history stack.
    pub fn navigate_to(&mut self, mut target_url: String) {
        target_url = target_url.trim().into();

        if target_url == "about:start" || target_url == "home" {
            self.url = String::from("about:start");
            self.update_history_and_record();
            self.load_start_page();
            self.save_active_tab_state();
            return;
        }

        if target_url == "about:jainos" || target_url == "local://about" {
            self.url = String::from("about:jainos");
            self.update_history_and_record();
            self.load_jainos_doc_page();
            self.save_active_tab_state();
            return;
        }

        // HTTPS warning check
        if target_url.starts_with("https://") {
            self.url = target_url.clone();
            self.update_history_and_record();
            self.show_https_unsupported_error();
            self.save_active_tab_state();
            return;
        }

        if !target_url.starts_with("http://") {
            target_url = format!("http://{}", target_url);
        }
        self.url = target_url;

        self.update_history_and_record();
        self.fetch_url();
    }

    fn update_history_and_record(&mut self) {
        if self.history_idx + 1 < self.history.len() {
            self.history.truncate(self.history_idx + 1);
        }
        self.history.push(self.url.clone());
        self.history_idx = self.history.len() - 1;

        // Record in global history
        if !self.global_history.iter().any(|h| h.url == self.url) {
            self.global_history.push(HistoryEntry {
                url: self.url.clone(),
                title: self.url.clone(),
            });
        }
    }

    /// Navigates back in history.
    pub fn go_back(&mut self) {
        if self.history_idx > 0 {
            self.history_idx -= 1;
            let target = self.history[self.history_idx].clone();
            self.navigate_to(target);
        }
    }

    /// Navigates forward in history.
    pub fn go_forward(&mut self) {
        if self.history_idx + 1 < self.history.len() {
            self.history_idx += 1;
            let target = self.history[self.history_idx].clone();
            self.navigate_to(target);
        }
    }

    /// Shows error page when user enters an HTTPS URL.
    fn show_https_unsupported_error(&mut self) {
        self.status = String::from("Protocol Error • HTTPS/TLS Unsupported");
        self.page_title = String::from("Unsupported Protocol");
        self.blocks.clear();

        self.blocks.push(HtmlBlock::Heading1(String::from("HTTPS / TLS Unsupported")));
        self.blocks.push(HtmlBlock::Divider);
        self.blocks.push(HtmlBlock::Paragraph(format!(
            "The requested address '{}' uses TLS encryption (HTTPS).",
            self.url
        )));
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "JainOS is a lightweight bare-metal operating system without a TLS 1.3 cryptographic accelerator. \
             Web browsing is currently supported over plain unencrypted HTTP on port 80."
        )));
        self.blocks.push(HtmlBlock::EmptyLine);
        self.blocks.push(HtmlBlock::Heading2(String::from("Recommended Actions")));
        let http_fallback = self.url.replace("https://", "http://");
        self.blocks.push(HtmlBlock::Link {
            text: format!("➜ Try connecting via unencrypted HTTP ({})", http_fallback),
            url: http_fallback,
        });
        self.blocks.push(HtmlBlock::Link {
            text: String::from("➜ Return to JainOS Start Page"),
            url: String::from("about:start"),
        });
    }

    /// Handles mouse clicks inside the Browser window.
    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32) {
        // --- Row 1: Tab Bar Clicks (y: 0..26) ---
        if rel_y >= 0 && rel_y <= 26 {
            let tab_w = 150;
            for i in 0..self.tabs.len() {
                let tx = 8 + (i as i32 * (tab_w + 4));
                if rel_x >= tx && rel_x < tx + tab_w {
                    // Check close button 'x' (last 16px of tab)
                    if rel_x >= tx + tab_w - 16 {
                        self.close_tab(i);
                    } else {
                        self.switch_tab(i);
                    }
                    return;
                }
            }

            // [ + ] New Tab Button
            let new_tab_x = 8 + (self.tabs.len() as i32 * (tab_w + 4));
            if rel_x >= new_tab_x && rel_x <= new_tab_x + 24 {
                self.new_tab("about:start");
                return;
            }
        }

        // --- Row 2: Navigation Toolbar (y: 26..62) ---
        if rel_y >= 26 && rel_y <= 62 {
            // [ ◄ ] Back Button at (8..34)
            if rel_x >= 8 && rel_x < 34 {
                self.go_back();
                return;
            }
            // [ ► ] Forward Button at (38..64)
            if rel_x >= 38 && rel_x < 64 {
                self.go_forward();
                return;
            }
            // [ ⟳ ] Reload Button at (68..94)
            if rel_x >= 68 && rel_x < 94 {
                let target = self.url.clone();
                self.navigate_to(target);
                return;
            }
            // [ ⌂ ] Home Button at (98..124)
            if rel_x >= 98 && rel_x < 124 {
                self.navigate_to(String::from("about:start"));
                return;
            }
            // [ Go ] Button at (530..574)
            if rel_x >= 530 && rel_x < 574 {
                let target = self.url.clone();
                self.navigate_to(target);
                return;
            }
            // [ ★ +Mark ] Button at (580..635)
            if rel_x >= 580 && rel_x < 635 {
                self.add_current_bookmark();
                return;
            }
            // [ ★ Marks ] Button at (640..685)
            if rel_x >= 640 && rel_x < 685 {
                self.drawer = if self.drawer == BrowserDrawer::Bookmarks {
                    BrowserDrawer::None
                } else {
                    BrowserDrawer::Bookmarks
                };
                return;
            }
            // [ ◷ Hist ] Button at (690..735)
            if rel_x >= 690 && rel_x < 735 {
                self.drawer = if self.drawer == BrowserDrawer::History {
                    BrowserDrawer::None
                } else {
                    BrowserDrawer::History
                };
                return;
            }
        }

        // --- Drawer Clicks (Bookmarks / History Drawer overlay) ---
        if self.drawer != BrowserDrawer::None && rel_y >= 64 && rel_y <= 240 && rel_x >= 450 {
            // [ Clear ] history button
            if self.drawer == BrowserDrawer::History && rel_y >= 68 && rel_y <= 88 && rel_x >= 650 {
                self.clear_history();
                return;
            }
            // Item clicks inside drawer
            let item_y = rel_y - 94;
            if item_y >= 0 {
                let idx = (item_y / 24) as usize;
                if self.drawer == BrowserDrawer::Bookmarks && idx < self.bookmarks.len() {
                    let target = self.bookmarks[idx].url.clone();
                    self.drawer = BrowserDrawer::None;
                    self.navigate_to(target);
                    return;
                } else if self.drawer == BrowserDrawer::History && idx < self.global_history.len() {
                    let target = self.global_history[idx].url.clone();
                    self.drawer = BrowserDrawer::None;
                    self.navigate_to(target);
                    return;
                }
            }
        }

        // --- Scrollbar Clicks (Right 16px of document area: rel_x >= cw - 18) ---
        if rel_x >= 720 && rel_y >= 64 {
            // Top arrow [ ▲ ]
            if rel_y <= 80 {
                self.scroll_y = (self.scroll_y - 45).max(0);
                return;
            }
            // Bottom arrow [ ▼ ]
            if rel_y >= 440 {
                self.scroll_y = (self.scroll_y + 45).min((self.total_doc_height - 200).max(0));
                return;
            }
            // Track click
            if rel_y < 250 {
                self.scroll_y = (self.scroll_y - 70).max(0);
            } else {
                self.scroll_y = (self.scroll_y + 70).min((self.total_doc_height - 200).max(0));
            }
            return;
        }

        // --- Document Area Hyperlink Hit-Testing ---
        for link in &self.link_hitboxes {
            if rel_x >= link.x && rel_x < link.x + link.w && rel_y >= link.y && rel_y < link.y + link.h {
                let target = link.target_url.clone();
                self.navigate_to(target);
                return;
            }
        }
    }

    /// Performs HTTP GET and parses HTML response.
    pub fn fetch_url(&mut self) {
        self.status = format!("Connecting to {}...", self.url);
        self.is_loading = true;
        self.scroll_y = 0;
        self.link_hitboxes.clear();

        match crate::net::http_get(&self.url) {
            Ok((status_code, _headers, body)) => {
                self.status = format!("HTTP {} OK • Received {} bytes", status_code, body.len());
                self.parse_html(&body);
            }
            Err(e) => {
                self.status = format!("HTTP error: {}", e);
                self.blocks.clear();
                self.page_title = format!("Connection Error - {}", self.url);
                self.blocks.push(HtmlBlock::Heading1(String::from("Cannot Reach Server")));
                self.blocks.push(HtmlBlock::Divider);
                self.blocks.push(HtmlBlock::Paragraph(format!(
                    "JainOS TCP client attempted connection to '{}' but the server did not respond.",
                    self.url
                )));
                self.blocks.push(HtmlBlock::Paragraph(format!("Diagnostic Details: {}", e)));
                self.blocks.push(HtmlBlock::EmptyLine);
                self.blocks.push(HtmlBlock::Heading2(String::from("Troubleshooting Recommendations")));
                self.blocks.push(HtmlBlock::Bullet(String::from(
                    "Check network adapter and gateway in the Network application"
                )));
                self.blocks.push(HtmlBlock::Bullet(String::from(
                    "Ensure target server supports unencrypted HTTP on port 80"
                )));
                self.blocks.push(HtmlBlock::Bullet(String::from(
                    "Verify DNS query to gateway 10.0.2.3 succeeds"
                )));
                self.blocks.push(HtmlBlock::EmptyLine);
                self.blocks.push(HtmlBlock::Link {
                    text: String::from("Click here to return to JainOS Start Page"),
                    url: String::from("about:start"),
                });
            }
        }
        self.is_loading = false;
        self.save_active_tab_state();
    }

    /// Tokenizes HTML document, ignoring comments, <style> and <script>, decoding entities.
    pub fn parse_html(&mut self, html: &str) {
        self.blocks.clear();
        let mut cur_text = String::new();
        let mut in_tag = false;
        let mut tag_buf = String::new();
        let mut title_found = false;
        let mut skip_content = false;
        let mut cur_row: Vec<String> = Vec::new();
        let mut is_header_row = false;

        let mut i = 0;
        let bytes = html.as_bytes();
        let len = bytes.len();

        while i < len {
            // Strip HTML comments <!-- ... -->
            if bytes[i..].starts_with(b"<!--") {
                if let Some(end) = html[i..].find("-->") {
                    i += end + 3;
                    continue;
                } else {
                    break;
                }
            }

            let b = bytes[i];

            if b == b'<' {
                in_tag = true;
                tag_buf.clear();
            } else if b == b'>' {
                in_tag = false;
                let tag_raw = tag_buf.trim().to_ascii_lowercase();

                // Check for script/style blocks to skip
                if tag_raw == "style" || tag_raw.starts_with("style ") {
                    skip_content = true;
                    cur_text.clear();
                } else if tag_raw == "/style" {
                    skip_content = false;
                    cur_text.clear();
                } else if tag_raw == "script" || tag_raw.starts_with("script ") {
                    skip_content = true;
                    cur_text.clear();
                } else if tag_raw == "/script" {
                    skip_content = false;
                    cur_text.clear();
                } else if !skip_content {
                    if tag_raw == "h1" || tag_raw.starts_with("h1 ") {
                        cur_text.clear();
                    } else if tag_raw == "/h1" {
                        let t = decode_entities(&cur_text);
                        if !t.is_empty() {
                            self.blocks.push(HtmlBlock::Heading1(t));
                            cur_text.clear();
                        }
                    } else if tag_raw == "h2" || tag_raw.starts_with("h2 ") {
                        cur_text.clear();
                    } else if tag_raw == "/h2" {
                        let t = decode_entities(&cur_text);
                        if !t.is_empty() {
                            self.blocks.push(HtmlBlock::Heading2(t));
                            cur_text.clear();
                        }
                    } else if tag_raw == "h3" || tag_raw.starts_with("h3 ") {
                        cur_text.clear();
                    } else if tag_raw == "/h3" {
                        let t = decode_entities(&cur_text);
                        if !t.is_empty() {
                            self.blocks.push(HtmlBlock::Heading3(t));
                            cur_text.clear();
                        }
                    } else if tag_raw == "h4" || tag_raw.starts_with("h4 ") || tag_raw == "h5" || tag_raw == "h6" {
                        cur_text.clear();
                    } else if tag_raw == "/h4" || tag_raw == "/h5" || tag_raw == "/h6" {
                        let t = decode_entities(&cur_text);
                        if !t.is_empty() {
                            self.blocks.push(HtmlBlock::Heading4(t));
                            cur_text.clear();
                        }
                    } else if tag_raw == "p" || tag_raw.starts_with("p ") {
                        cur_text.clear();
                    } else if tag_raw == "/p" {
                        let t = decode_entities(&cur_text);
                        if !t.is_empty() {
                            self.blocks.push(HtmlBlock::Paragraph(t));
                            cur_text.clear();
                        }
                    } else if tag_raw == "li" || tag_raw.starts_with("li ") {
                        cur_text.clear();
                    } else if tag_raw == "/li" {
                        let t = decode_entities(&cur_text);
                        if !t.is_empty() {
                            self.blocks.push(HtmlBlock::Bullet(t));
                            cur_text.clear();
                        }
                    } else if tag_raw.starts_with("a ") && tag_raw.contains("href=") {
                        let href = extract_href(&tag_buf);
                        cur_text.clear();
                        if !href.is_empty() {
                            cur_text.push_str("@@LINK@@");
                            cur_text.push_str(&href);
                            cur_text.push_str("@@TEXT@@");
                        }
                    } else if tag_raw == "/a" {
                        if let Some(pos_link) = cur_text.find("@@LINK@@") {
                            if let Some(pos_text) = cur_text.find("@@TEXT@@") {
                                let href = String::from(&cur_text[pos_link + 8..pos_text]);
                                let label = decode_entities(&cur_text[pos_text + 8..]);
                                if !label.is_empty() {
                                    self.blocks.push(HtmlBlock::Link {
                                        text: label,
                                        url: href,
                                    });
                                }
                                cur_text.clear();
                            }
                        }
                    } else if tag_raw == "hr" || tag_raw == "hr/" {
                        self.blocks.push(HtmlBlock::Divider);
                    } else if tag_raw == "br" || tag_raw == "br/" {
                        self.blocks.push(HtmlBlock::EmptyLine);
                    } else if tag_raw == "tr" || tag_raw.starts_with("tr ") {
                        cur_row.clear();
                        is_header_row = false;
                        cur_text.clear();
                    } else if tag_raw == "/tr" {
                        if !cur_row.is_empty() {
                            if is_header_row {
                                self.blocks.push(HtmlBlock::TableHeader(cur_row.clone()));
                            } else {
                                self.blocks.push(HtmlBlock::TableRow(cur_row.clone()));
                            }
                            cur_row.clear();
                        }
                        cur_text.clear();
                    } else if tag_raw == "th" || tag_raw.starts_with("th ") {
                        is_header_row = true;
                        cur_text.clear();
                    } else if tag_raw == "td" || tag_raw.starts_with("td ") {
                        cur_text.clear();
                    } else if tag_raw == "/td" || tag_raw == "/th" {
                        let cell = decode_entities(&cur_text);
                        if !cell.trim().is_empty() {
                            cur_row.push(cell.trim().into());
                        }
                        cur_text.clear();
                    } else if tag_raw == "title" {
                        cur_text.clear();
                    } else if tag_raw == "/title" {
                        let t = decode_entities(&cur_text);
                        if !t.is_empty() {
                            self.page_title = t;
                            title_found = true;
                            cur_text.clear();
                        }
                    }
                }
            } else if in_tag {
                tag_buf.push(b as char);
            } else if !skip_content {
                cur_text.push(b as char);
            }

            i += 1;
        }

        if !title_found {
            self.page_title = format!("Web: {}", self.url);
        }

        // Flush any remaining text as a paragraph
        let remaining = decode_entities(&cur_text);
        if !remaining.trim().is_empty() {
            self.blocks.push(HtmlBlock::Paragraph(remaining));
        }

        if self.blocks.is_empty() {
            self.blocks.push(HtmlBlock::Paragraph(String::from("(Document loaded successfully)")));
        }
    }

    /// Renders the complete Browser UI.
    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        self.link_hitboxes.clear();

        // --- ROW 1: MULTI-TAB BAR (y: cy, h: 26) ---
        canvas.fill_rect(cx, cy, cw, 26, 0x000F172A);
        canvas.fill_rect(cx, cy + 25, cw, 1, 0x00334155);

        let tab_w = 150;
        for (i, tab) in self.tabs.iter().enumerate() {
            let tx = cx + 8 + (i as i32 * (tab_w + 4));
            if tx + tab_w > cx + cw - 40 {
                break;
            }
            let is_active = i == self.active_tab;
            if is_active {
                canvas.draw_gradient_v(tx, cy + 3, tab_w, 23, 0x001E3A8A, 0x001E293B);
                canvas.draw_rect(tx, cy + 3, tab_w, 23, 0x0038BDF8);
            } else {
                canvas.fill_rect(tx, cy + 5, tab_w, 21, 0x001E293B);
                canvas.draw_rect(tx, cy + 5, tab_w, 21, 0x00334155);
            }

            // Tab globe icon
            let icon_col = if is_active { 0x0038BDF8 } else { 0x0064748B };
            canvas.fill_rect(tx + 6, cy + 9, 8, 8, icon_col);

            // Tab title
            let mut title = if is_active { self.page_title.clone() } else { tab.page_title.clone() };
            if title.len() > 13 {
                title.truncate(11);
                title.push_str("..");
            }
            let text_col = if is_active { 0x00FFFFFF } else { 0x0094A3B8 };
            canvas.draw_string(tx + 18, cy + 8, &title, text_col);

            // Close button 'x'
            canvas.draw_string(tx + tab_w - 14, cy + 8, "x", 0x0094A3B8);
        }

        // [ + ] New Tab Button
        let new_tab_x = cx + 8 + (self.tabs.len() as i32 * (tab_w + 4));
        if new_tab_x + 24 <= cx + cw {
            canvas.fill_rect(new_tab_x, cy + 5, 22, 20, 0x001E293B);
            canvas.draw_rect(new_tab_x, cy + 5, 22, 20, 0x0038BDF8);
            canvas.draw_string(new_tab_x + 7, cy + 7, "+", 0x0038BDF8);
        }

        // --- ROW 2: MODERN NAVIGATION TOOLBAR (y: cy + 26, h: 36) ---
        let tb_y = cy + 26;
        canvas.fill_rect(cx, tb_y, cw, 36, 0x001E293B);
        canvas.fill_rect(cx, tb_y + 35, cw, 1, 0x00334155);

        // [ ◄ ] Back Button
        let back_col = if self.history_idx > 0 { 0x0038BDF8 } else { 0x0064748B };
        canvas.fill_rect(cx + 8, tb_y + 5, 26, 26, 0x000F172A);
        canvas.draw_rect(cx + 8, tb_y + 5, 26, 26, back_col);
        canvas.draw_string(cx + 16, tb_y + 9, "<", back_col);

        // [ ► ] Forward Button
        let fwd_col = if self.history_idx + 1 < self.history.len() { 0x0038BDF8 } else { 0x0064748B };
        canvas.fill_rect(cx + 38, tb_y + 5, 26, 26, 0x000F172A);
        canvas.draw_rect(cx + 38, tb_y + 5, 26, 26, fwd_col);
        canvas.draw_string(cx + 46, tb_y + 9, ">", fwd_col);

        // [ ⟳ ] Reload Button
        canvas.fill_rect(cx + 68, tb_y + 5, 26, 26, 0x000F172A);
        canvas.draw_rect(cx + 68, tb_y + 5, 26, 26, 0x0094A3B8);
        canvas.draw_string(cx + 76, tb_y + 9, "R", 0x00E2E8F0);

        // [ ⌂ ] Home Button
        canvas.fill_rect(cx + 98, tb_y + 5, 26, 26, 0x000F172A);
        canvas.draw_rect(cx + 98, tb_y + 5, 26, 26, 0x0094A3B8);
        canvas.draw_string(cx + 104, tb_y + 9, "H", 0x00CBD5E1);

        // URL Omnibox (Address input field)
        let omni_x = cx + 130;
        let omni_w = 394;
        canvas.fill_rect(omni_x, tb_y + 5, omni_w, 26, 0x000F172A);
        canvas.draw_rect(omni_x, tb_y + 5, omni_w, 26, 0x0038BDF8);

        // Protocol badge inside Omnibox
        let (badge_txt, badge_col) = if self.url.starts_with("about:") {
            ("LOCAL", 0x00A855F7)
        } else {
            ("HTTP", 0x0038BDF8)
        };
        canvas.fill_rect(omni_x + 3, tb_y + 7, 42, 20, 0x001E293B);
        canvas.draw_string(omni_x + 6, tb_y + 9, badge_txt, badge_col);

        // URL text
        let mut display_url = self.url.clone();
        let max_url_chars = ((omni_w - 56) / 8).max(10) as usize;
        if display_url.len() > max_url_chars {
            display_url.truncate(max_url_chars - 2);
            display_url.push_str("..");
        }
        canvas.draw_string(omni_x + 50, tb_y + 9, &display_url, 0x00FFFFFF);

        // [ Go ] Button at (omni_x + omni_w + 6)
        let go_x = omni_x + omni_w + 6;
        canvas.draw_gradient_v(go_x, tb_y + 5, 42, 26, 0x002563EB, 0x001D4ED8);
        canvas.draw_rect(go_x, tb_y + 5, 42, 26, 0x0060A5FA);
        canvas.draw_string(go_x + 12, tb_y + 9, "Go", 0x00FFFFFF);

        // [ ★ +Mark ] Add Bookmark button
        let bm_add_x = go_x + 48;
        canvas.fill_rect(bm_add_x, tb_y + 5, 52, 26, 0x001E293B);
        canvas.draw_rect(bm_add_x, tb_y + 5, 52, 26, 0x00FACC15);
        canvas.draw_string(bm_add_x + 6, tb_y + 9, "+Mark", 0x00FACC15);

        // [ ★ Marks ] Bookmarks Drawer toggle button
        let bms_x = bm_add_x + 56;
        let bms_bg = if self.drawer == BrowserDrawer::Bookmarks { 0x001D4ED8 } else { 0x001E293B };
        canvas.fill_rect(bms_x, tb_y + 5, 48, 26, bms_bg);
        canvas.draw_rect(bms_x, tb_y + 5, 48, 26, 0x0038BDF8);
        canvas.draw_string(bms_x + 6, tb_y + 9, "Marks", 0x0038BDF8);

        // [ ◷ Hist ] History Drawer toggle button
        let hist_x = bms_x + 52;
        let hist_bg = if self.drawer == BrowserDrawer::History { 0x001D4ED8 } else { 0x001E293B };
        canvas.fill_rect(hist_x, tb_y + 5, 44, 26, hist_bg);
        canvas.draw_rect(hist_x, tb_y + 5, 44, 26, 0x0038BDF8);
        canvas.draw_string(hist_x + 6, tb_y + 9, "Hist", 0x0038BDF8);

        // --- ROW 3: WEBPAGE DOCUMENT VIEWPORT ---
        let doc_y = tb_y + 36;
        let doc_h = ch - 62 - 24;
        let doc_w = cw - 16;
        canvas.fill_rect(cx, doc_y, cw, doc_h, 0x00F8FAFC); // Clean paper white canvas

        let max_line_chars = ((doc_w - 48) / 8).max(20) as usize;
        let mut cur_y = doc_y + 16 - self.scroll_y;
        let content_start_y = cur_y;

        for block in &self.blocks {
            match block {
                HtmlBlock::Heading1(text) => {
                    if cur_y + 26 >= doc_y && cur_y <= doc_y + doc_h {
                        canvas.fill_rect(cx + 16, cur_y - 2, doc_w - 32, 26, 0x001E3A8A);
                        canvas.draw_string(cx + 24, cur_y + 4, text, 0x00FFFFFF);
                    }
                    cur_y += 32;
                }
                HtmlBlock::Heading2(text) => {
                    if cur_y + 22 >= doc_y && cur_y <= doc_y + doc_h {
                        canvas.draw_string(cx + 20, cur_y, text, 0x001D4ED8);
                        canvas.fill_rect(cx + 20, cur_y + 15, doc_w - 40, 2, 0x0093C5FD);
                    }
                    cur_y += 24;
                }
                HtmlBlock::Heading3(text) => {
                    if cur_y + 18 >= doc_y && cur_y <= doc_y + doc_h {
                        canvas.draw_string(cx + 20, cur_y, text, 0x000F172A);
                    }
                    cur_y += 20;
                }
                HtmlBlock::Heading4(text) => {
                    if cur_y + 16 >= doc_y && cur_y <= doc_y + doc_h {
                        canvas.draw_string(cx + 20, cur_y, text, 0x00334155);
                    }
                    cur_y += 18;
                }
                HtmlBlock::Paragraph(text) => {
                    let wrapped_lines = wrap_text(text, max_line_chars);
                    for line in wrapped_lines {
                        if cur_y + 16 >= doc_y && cur_y <= doc_y + doc_h {
                            canvas.draw_string(cx + 20, cur_y, &line, 0x00334155);
                        }
                        cur_y += 18;
                    }
                    cur_y += 4;
                }
                HtmlBlock::Bullet(text) => {
                    let wrapped = wrap_text(text, max_line_chars.saturating_sub(4));
                    for (line_idx, line) in wrapped.iter().enumerate() {
                        if cur_y + 16 >= doc_y && cur_y <= doc_y + doc_h {
                            if line_idx == 0 {
                                canvas.fill_rect(cx + 24, cur_y + 5, 5, 5, 0x002563EB);
                                canvas.draw_string(cx + 34, cur_y, line, 0x00334155);
                            } else {
                                canvas.draw_string(cx + 34, cur_y, line, 0x00334155);
                            }
                        }
                        cur_y += 18;
                    }
                }
                HtmlBlock::OrderedItem(num, text) => {
                    let prefix = format!("{}. ", num);
                    let wrapped = wrap_text(text, max_line_chars.saturating_sub(5));
                    for (line_idx, line) in wrapped.iter().enumerate() {
                        if cur_y + 16 >= doc_y && cur_y <= doc_y + doc_h {
                            if line_idx == 0 {
                                canvas.draw_string(cx + 20, cur_y, &prefix, 0x002563EB);
                                canvas.draw_string(cx + 40, cur_y, line, 0x00334155);
                            } else {
                                canvas.draw_string(cx + 40, cur_y, line, 0x00334155);
                            }
                        }
                        cur_y += 18;
                    }
                }
                HtmlBlock::Link { text, url } => {
                    let wrapped = wrap_text(text, max_line_chars.saturating_sub(2));
                    for line in wrapped {
                        let link_w = (line.len() * 8) as i32;
                        if cur_y + 16 >= doc_y && cur_y <= doc_y + doc_h {
                            canvas.draw_string(cx + 20, cur_y, &line, 0x000284C7);
                            canvas.fill_rect(cx + 20, cur_y + 13, link_w, 1, 0x0038BDF8);

                            self.link_hitboxes.push(LinkHitBox {
                                x: cx + 20,
                                y: cur_y,
                                w: link_w,
                                h: 16,
                                target_url: url.clone(),
                            });
                        }
                        cur_y += 18;
                    }
                    cur_y += 2;
                }
                HtmlBlock::Divider => {
                    if cur_y + 6 >= doc_y && cur_y <= doc_y + doc_h {
                        canvas.fill_rect(cx + 20, cur_y + 4, doc_w - 40, 1, 0x00CBD5E1);
                    }
                    cur_y += 14;
                }
                HtmlBlock::TableHeader(cells) => {
                    if cur_y + 20 >= doc_y && cur_y <= doc_y + doc_h {
                        let cell_w = (doc_w - 40) / cells.len().max(1) as i32;
                        for (c_idx, cell) in cells.iter().enumerate() {
                            let cell_x = cx + 20 + (c_idx as i32 * cell_w);
                            canvas.fill_rect(cell_x, cur_y, cell_w - 2, 20, 0x001E3A8A);
                            canvas.draw_string(cell_x + 6, cur_y + 3, cell, 0x00FFFFFF);
                        }
                    }
                    cur_y += 22;
                }
                HtmlBlock::TableRow(cells) => {
                    if cur_y + 20 >= doc_y && cur_y <= doc_y + doc_h {
                        let cell_w = (doc_w - 40) / cells.len().max(1) as i32;
                        for (c_idx, cell) in cells.iter().enumerate() {
                            let cell_x = cx + 20 + (c_idx as i32 * cell_w);
                            canvas.fill_rect(cell_x, cur_y, cell_w - 2, 18, 0x00E2E8F0);
                            canvas.draw_string(cell_x + 4, cur_y + 2, cell, 0x000F172A);
                        }
                    }
                    cur_y += 20;
                }
                HtmlBlock::EmptyLine => {
                    cur_y += 10;
                }
            }
        }

        self.total_doc_height = (cur_y - content_start_y).max(doc_h);

        // --- DRAW DRAWER OVERLAY (Bookmarks or History) ---
        if self.drawer != BrowserDrawer::None {
            let dw_x = cx + cw - 280;
            let dw_y = doc_y;
            let dw_w = 264;
            let dw_h = 240;

            canvas.fill_rect(dw_x, dw_y, dw_w, dw_h, 0x000F172A);
            canvas.draw_rect(dw_x, dw_y, dw_w, dw_h, 0x0038BDF8);

            match self.drawer {
                BrowserDrawer::Bookmarks => {
                    canvas.fill_rect(dw_x + 1, dw_y + 1, dw_w - 2, 22, 0x001E293B);
                    canvas.draw_string(dw_x + 8, dw_y + 4, "★ BOOKMARKS", 0x00FACC15);

                    let mut by = dw_y + 28;
                    for (b_idx, bm) in self.bookmarks.iter().enumerate() {
                        if by + 22 > dw_y + dw_h {
                            break;
                        }
                        canvas.fill_rect(dw_x + 4, by, dw_w - 8, 20, 0x001E293B);
                        let mut b_title = bm.title.clone();
                        if b_title.len() > 25 {
                            b_title.truncate(23);
                            b_title.push_str("..");
                        }
                        canvas.draw_string(dw_x + 8, by + 3, &format!("{}. {}", b_idx + 1, b_title), 0x0038BDF8);
                        by += 24;
                    }
                }
                BrowserDrawer::History => {
                    canvas.fill_rect(dw_x + 1, dw_y + 1, dw_w - 2, 22, 0x001E293B);
                    canvas.draw_string(dw_x + 8, dw_y + 4, "◷ BROWSING HISTORY", 0x0038BDF8);

                    // [ Clear ] button inside drawer
                    canvas.fill_rect(dw_x + dw_w - 55, dw_y + 3, 50, 16, 0x00DC2626);
                    canvas.draw_string(dw_x + dw_w - 48, dw_y + 4, "Clear", 0x00FFFFFF);

                    let mut hy = dw_y + 28;
                    for (h_idx, entry) in self.global_history.iter().rev().take(8).enumerate() {
                        if hy + 22 > dw_y + dw_h {
                            break;
                        }
                        canvas.fill_rect(dw_x + 4, hy, dw_w - 8, 20, 0x001E293B);
                        let mut h_url = entry.url.clone();
                        if h_url.len() > 26 {
                            h_url.truncate(24);
                            h_url.push_str("..");
                        }
                        canvas.draw_string(dw_x + 8, hy + 3, &format!("{}. {}", h_idx + 1, h_url), 0x00CBD5E1);
                        hy += 24;
                    }
                }
                BrowserDrawer::None => {}
            }
        }

        // --- MODERN VERTICAL SCROLLBAR (Right 16px) ---
        let sb_x = cx + cw - 16;
        canvas.fill_rect(sb_x, doc_y, 16, doc_h, 0x00E2E8F0);
        canvas.draw_rect(sb_x, doc_y, 16, doc_h, 0x00CBD5E1);

        // Scroll Up button [ ▲ ]
        canvas.fill_rect(sb_x + 1, doc_y + 1, 14, 14, 0x00CBD5E1);
        canvas.draw_string(sb_x + 4, doc_y + 2, "^", 0x000F172A);

        // Scroll Down button [ ▼ ]
        canvas.fill_rect(sb_x + 1, doc_y + doc_h - 15, 14, 14, 0x00CBD5E1);
        canvas.draw_string(sb_x + 4, doc_y + doc_h - 14, "v", 0x000F172A);

        // Scroll Thumb
        let track_h = doc_h - 30;
        let thumb_h = ((doc_h as i64 * track_h as i64) / self.total_doc_height.max(1) as i64).clamp(20, track_h as i64) as i32;
        let max_scroll = (self.total_doc_height - doc_h).max(1);
        let thumb_y = doc_y + 15 + ((self.scroll_y as i64 * (track_h - thumb_h) as i64) / max_scroll as i64) as i32;

        canvas.fill_rect(sb_x + 2, thumb_y, 12, thumb_h, 0x0094A3B8);
        canvas.draw_rect(sb_x + 2, thumb_y, 12, thumb_h, 0x0064748B);

        // --- ROW 4: BOTTOM STATUS BAR (y: cy + ch - 24, h: 24) ---
        let sb_bar_y = cy + ch - 24;
        canvas.fill_rect(cx, sb_bar_y, cw, 24, 0x000F172A);
        canvas.fill_rect(cx, sb_bar_y, cw, 1, 0x00334155);

        // Loading indicator dot or green ready dot
        let dot_col = if self.is_loading { 0x00FACC15 } else { 0x0022C55E };
        canvas.fill_rect(cx + 10, sb_bar_y + 8, 8, 8, dot_col);

        // Status text
        canvas.draw_string(cx + 24, sb_bar_y + 5, &self.status, 0x0038BDF8);

        // Engine badge on right
        let engine_str = "JainOS WebEngine v0.3.4 (No-STD)";
        let engine_w = (engine_str.len() * 8) as i32;
        canvas.draw_string(cx + cw - engine_w - 10, sb_bar_y + 5, engine_str, 0x0064748B);
    }
}

/// Helper function to wrap text cleanly without splitting words.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = String::from(word);
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Helper function to decode common HTML character entities.
fn decode_entities(input: &str) -> String {
    let mut s = String::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '&' {
            let mut ent = String::new();
            let mut j = i + 1;
            while j < len && j < i + 10 && chars[j] != ';' && chars[j] != ' ' {
                ent.push(chars[j]);
                j += 1;
            }
            if j < len && chars[j] == ';' {
                match ent.as_str() {
                    "amp" => s.push('&'),
                    "lt" => s.push('<'),
                    "gt" => s.push('>'),
                    "quot" => s.push('"'),
                    "#39" | "apos" => s.push('\''),
                    "nbsp" | "#160" => s.push(' '),
                    _ => {
                        s.push('&');
                        s.push_str(&ent);
                        s.push(';');
                    }
                }
                i = j + 1;
                continue;
            }
        }
        s.push(chars[i]);
        i += 1;
    }
    s
}

/// Extracts the target href from an <a ...> tag buffer.
fn extract_href(tag: &str) -> String {
    let lower = tag.to_ascii_lowercase();
    if let Some(pos) = lower.find("href=") {
        let after = &tag[pos + 5..].trim_start();
        let quote = after.chars().next().unwrap_or(' ');
        if quote == '"' || quote == '\'' {
            let inner = &after[1..];
            if let Some(end_pos) = inner.find(quote) {
                return String::from(&inner[..end_pos]);
            }
        } else {
            let end_pos = after.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(after.len());
            return String::from(&after[..end_pos]);
        }
    }
    String::new()
}
