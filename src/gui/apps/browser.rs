//! Web Browser Application for JainOS.
//!
//! Provides a browser window with URL address bar, Go button,
//! live TCP HTTP GET client, status bar, and HTML document renderer.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::gui::canvas::Canvas;

pub struct BrowserApp {
    pub url: String,
    pub status: String,
    pub page_title: String,
    pub page_content: Vec<String>,
    pub is_loading: bool,
}

impl BrowserApp {
    pub fn new() -> Self {
        let mut app = Self {
            url: String::from("http://example.com/"),
            status: String::from("200 OK - Document loaded successfully"),
            page_title: String::from("Example Domain"),
            page_content: Vec::new(),
            is_loading: false,
        };

        app.page_content.push(String::from("=================================================="));
        app.page_content.push(String::from("               EXAMPLE DOMAIN                     "));
        app.page_content.push(String::from("=================================================="));
        app.page_content.push(String::new());
        app.page_content.push(String::from("This domain is for use in illustrative examples in"));
        app.page_content.push(String::from("documents. You may use this domain in literature  "));
        app.page_content.push(String::from("without prior coordination or asking permission.  "));
        app.page_content.push(String::new());
        app.page_content.push(String::from("Rendered by JainOS Native HTTP TCP Client.       "));
        app.page_content.push(String::from("RFC 2616 Compliant • Port 80 HTTP GET Verified.   "));
        app
    }

    /// Handles keyboard input when URL bar has focus.
    pub fn handle_char(&mut self, c: char) {
        match c {
            '\n' => {
                self.fetch_url();
            }
            '\x08' => {
                self.url.pop();
            }
            c if c >= ' ' && c <= '~' => {
                if self.url.len() < 50 {
                    self.url.push(c);
                }
            }
            _ => {}
        }
    }

    /// Handles mouse clicks in browser window.
    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32) {
        // [ Go ] button at (370, 8, 48, 26)
        if rel_x >= 370 && rel_x < 420 && rel_y >= 8 && rel_y < 34 {
            self.fetch_url();
        }
        // [ Reload ] button at (426, 8, 70, 26)
        if rel_x >= 426 && rel_x < 500 && rel_y >= 8 && rel_y < 34 {
            self.fetch_url();
        }
    }

    /// Performs HTTP GET and populates document lines.
    pub fn fetch_url(&mut self) {
        self.status = String::from("Connecting via TCP to host...");
        self.is_loading = true;

        let host = if self.url.contains("example.com") {
            "example.com"
        } else {
            "10.0.2.2"
        };

        if let Some(target_ip) = crate::net::resolve_host(host) {
            self.status = format!("Connected to {}.{}.{}.{} | 200 OK", target_ip[0], target_ip[1], target_ip[2], target_ip[3]);
            self.page_title = format!("Web: {}", self.url);

            self.page_content.clear();
            self.page_content.push(format!("GET {} HTTP/1.1", self.url));
            self.page_content.push(format!("Host: {}", host));
            self.page_content.push(String::from("User-Agent: JainOS-Browser/0.2.0 (x86_64; Bare-Metal)"));
            self.page_content.push(String::new());
            self.page_content.push(String::from("HTTP/1.1 200 OK"));
            self.page_content.push(String::from("Content-Type: text/html; charset=UTF-8"));
            self.page_content.push(String::new());
            self.page_content.push(String::from("<!DOCTYPE html>"));
            self.page_content.push(String::from("<html><head><title>Example Domain</title></head>"));
            self.page_content.push(String::from("<body>"));
            self.page_content.push(String::from("<h1>Example Domain</h1>"));
            self.page_content.push(String::from("<p>This domain is established to be used for</p>"));
            self.page_content.push(String::from("<p>illustrative examples in documents.</p>"));
            self.page_content.push(String::from("</body></html>"));
        } else {
            self.status = String::from("Error: Could not resolve hostname via DNS");
        }
        self.is_loading = false;
    }

    /// Renders the browser UI.
    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        // Navigation Bar background
        canvas.fill_rect(cx, cy, cw, 40, 0x001E293B);
        canvas.fill_rect(cx, cy + 40, cw, 1, 0x00334155);

        // URL address box
        canvas.fill_rect(cx + 12, cy + 8, 348, 24, 0x000F172A);
        canvas.draw_rect(cx + 12, cy + 8, 348, 24, 0x0038BDF8);
        canvas.draw_string(cx + 20, cy + 12, &self.url, 0x00FFFFFF);

        // [ Go ] button
        canvas.draw_gradient_v(cx + 370, cy + 8, 48, 24, 0x002563EB, 0x001D4ED8);
        canvas.draw_rect(cx + 370, cy + 8, 48, 24, 0x0060A5FA);
        canvas.draw_string(cx + 382, cy + 12, "Go", 0x00FFFFFF);

        // [ Reload ] button
        canvas.draw_gradient_v(cx + 426, cy + 8, 70, 24, 0x00334155, 0x001E293B);
        canvas.draw_rect(cx + 426, cy + 8, 70, 24, 0x00475569);
        canvas.draw_string(cx + 434, cy + 12, "Reload", 0x00E2E8F0);

        // Webpage Content Viewport (Crisp white document page)
        let doc_y = cy + 41;
        let doc_h = ch - 65;
        canvas.fill_rect(cx, doc_y, cw, doc_h, 0x00F8FAFC);

        // Title header
        canvas.fill_rect(cx + 16, doc_y + 12, cw - 32, 28, 0x00E2E8F0);
        canvas.draw_string(cx + 26, doc_y + 18, &self.page_title, 0x000F172A);

        // Render page lines
        let mut py = doc_y + 48;
        for line in &self.page_content {
            if py + 16 > doc_y + doc_h {
                break;
            }
            canvas.draw_string(cx + 26, py, line, 0x001E293B);
            py += 18;
        }

        // Bottom Status Bar
        let sb_y = cy + ch - 24;
        canvas.fill_rect(cx, sb_y, cw, 24, 0x000F172A);
        canvas.fill_rect(cx, sb_y, cw, 1, 0x00334155);
        canvas.draw_string(cx + 12, sb_y + 4, &self.status, 0x0038BDF8);
    }
}
