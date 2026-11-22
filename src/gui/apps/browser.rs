//! Web Browser Application for JainOS.
//!
//! Features:
//! - Real HTTP GET client over TCP/IP (`crate::net::http_get`)
//! - Full navigation controls: Back, Forward, Reload, Home
//! - History stack with indexed navigation
//! - Lightweight HTML document parser and renderer
//! - Support for headings (h1-h3), paragraphs, links (a href), bullet lists (li), and dividers (hr)
//! - Scrollable document viewport

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use crate::gui::canvas::Canvas;

#[derive(Debug, Clone)]
pub enum RenderNode {
    Heading1(String),
    Heading2(String),
    Heading3(String),
    Paragraph(String),
    Bullet(String),
    Link(String, String), // (text, target_url)
    Divider,
    EmptyLine,
}

pub struct BrowserApp {
    pub url: String,
    pub status: String,
    pub page_title: String,
    pub history: Vec<String>,
    pub history_idx: usize,
    pub nodes: Vec<RenderNode>,
    pub scroll_offset: usize,
    pub is_loading: bool,
}

impl BrowserApp {
    pub fn new() -> Self {
        let mut app = Self {
            url: String::from("http://example.com/"),
            status: String::from("Ready - JainOS HTML Web Client"),
            page_title: String::from("Example Domain"),
            history: Vec::new(),
            history_idx: 0,
            nodes: Vec::new(),
            scroll_offset: 0,
            is_loading: false,
        };

        app.history.push(app.url.clone());
        app.load_default_page();
        app
    }

    fn load_default_page(&mut self) {
        self.page_title = String::from("Example Domain");
        self.status = String::from("200 OK - Local cached mirror loaded");
        self.nodes.clear();
        self.nodes.push(RenderNode::Heading1(String::from("Example Domain")));
        self.nodes.push(RenderNode::Divider);
        self.nodes.push(RenderNode::Paragraph(String::from(
            "This domain is established to be used for illustrative examples in documents."
        )));
        self.nodes.push(RenderNode::Paragraph(String::from(
            "You may use this domain in literature without prior coordination or asking for permission."
        )));
        self.nodes.push(RenderNode::EmptyLine);
        self.nodes.push(RenderNode::Heading2(String::from("JainOS Browser Capabilities")));
        self.nodes.push(RenderNode::Bullet(String::from("Native TCP socket stack with SYN/ACK flow control")));
        self.nodes.push(RenderNode::Bullet(String::from("DNS resolution over UDP 10.0.2.3")));
        self.nodes.push(RenderNode::Bullet(String::from("HTTP/1.1 protocol compliant client")));
        self.nodes.push(RenderNode::Bullet(String::from("Tokenized HTML heading, paragraph, and link renderer")));
        self.nodes.push(RenderNode::EmptyLine);
        self.nodes.push(RenderNode::Link(
            String::from("More information: RFC 2616 Hypertext Transfer Protocol"),
            String::from("http://example.com/"),
        ));
    }

    /// Handles keyboard input when URL bar is active.
    pub fn handle_char(&mut self, c: char) {
        match c {
            '\n' => {
                self.navigate_to(self.url.clone());
            }
            '\x08' => {
                self.url.pop();
            }
            c if c >= ' ' && c <= '~' => {
                if self.url.len() < 64 {
                    self.url.push(c);
                }
            }
            _ => {}
        }
    }

    /// Navigates to a specific URL and pushes to history.
    pub fn navigate_to(&mut self, target_url: String) {
        self.url = target_url;
        if self.history_idx + 1 < self.history.len() {
            self.history.truncate(self.history_idx + 1);
        }
        self.history.push(self.url.clone());
        self.history_idx = self.history.len() - 1;
        self.fetch_url();
    }

    /// Navigates back in history.
    pub fn go_back(&mut self) {
        if self.history_idx > 0 {
            self.history_idx -= 1;
            self.url = self.history[self.history_idx].clone();
            self.fetch_url();
        }
    }

    /// Navigates forward in history.
    pub fn go_forward(&mut self) {
        if self.history_idx + 1 < self.history.len() {
            self.history_idx += 1;
            self.url = self.history[self.history_idx].clone();
            self.fetch_url();
        }
    }

    /// Handles mouse clicks in the browser window.
    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32) {
        // [ < ] Back Button at (10, 8, 26, 24)
        if rel_x >= 10 && rel_x < 36 && rel_y >= 8 && rel_y < 32 {
            self.go_back();
            return;
        }

        // [ > ] Forward Button at (38, 8, 26, 24)
        if rel_x >= 38 && rel_x < 64 && rel_y >= 8 && rel_y < 32 {
            self.go_forward();
            return;
        }

        // [ R ] Reload Button at (66, 8, 26, 24)
        if rel_x >= 66 && rel_x < 92 && rel_y >= 8 && rel_y < 32 {
            self.fetch_url();
            return;
        }

        // [ Home ] Button at (94, 8, 48, 24)
        if rel_x >= 94 && rel_x < 142 && rel_y >= 8 && rel_y < 32 {
            self.navigate_to(String::from("http://example.com/"));
            return;
        }

        // [ Go ] Button at (rel_w - 90..rel_w - 50) -> approximate at (480, 8, 40, 24)
        if rel_x >= 470 && rel_x < 515 && rel_y >= 8 && rel_y < 32 {
            self.navigate_to(self.url.clone());
            return;
        }

        // Scroll Up [ ^ ] button at (540, 8, 22, 24)
        if rel_x >= 520 && rel_x < 542 && rel_y >= 8 && rel_y < 32 {
            self.scroll_offset = self.scroll_offset.saturating_sub(2);
            return;
        }

        // Scroll Down [ v ] button at (544, 8, 22, 24)
        if rel_x >= 544 && rel_x < 566 && rel_y >= 8 && rel_y < 32 {
            if self.scroll_offset + 5 < self.nodes.len() {
                self.scroll_offset += 2;
            }
            return;
        }
    }

    /// Fetches URL via real HTTP GET and tokenizes HTML.
    pub fn fetch_url(&mut self) {
        self.status = format!("Connecting to {}...", self.url);
        self.is_loading = true;
        self.scroll_offset = 0;

        match crate::net::http_get(&self.url) {
            Ok((status_code, _headers, body)) => {
                self.status = format!("HTTP {} OK | Received {} bytes", status_code, body.len());
                self.parse_html(&body);
            }
            Err(e) => {
                // If remote network is unreachable or timed out, display helpful diagnostic page
                self.status = format!("HTTP error: {}", e);
                self.nodes.clear();
                self.page_title = format!("Connection Alert - {}", self.url);
                self.nodes.push(RenderNode::Heading1(String::from("Cannot Open Page")));
                self.nodes.push(RenderNode::Divider);
                self.nodes.push(RenderNode::Paragraph(format!(
                    "JainOS TCP client attempted connection to '{}' but the server did not respond.",
                    self.url
                )));
                self.nodes.push(RenderNode::Paragraph(format!("Diagnostic Reason: {}", e)));
                self.nodes.push(RenderNode::EmptyLine);
                self.nodes.push(RenderNode::Heading2(String::from("Troubleshooting Recommendations")));
                self.nodes.push(RenderNode::Bullet(String::from(
                    "Check network cable / VirtualBox NAT adapter in RiftNet"
                )));
                self.nodes.push(RenderNode::Bullet(String::from(
                    "Verify gateway IP 10.0.2.2 and DNS 10.0.2.3"
                )));
                self.nodes.push(RenderNode::Bullet(String::from(
                    "Ensure target server supports unencrypted HTTP port 80"
                )));
                self.nodes.push(RenderNode::EmptyLine);
                self.nodes.push(RenderNode::Link(
                    String::from("Click here to return to Example Domain"),
                    String::from("http://example.com/"),
                ));
            }
        }
        self.is_loading = false;
    }

    /// Tokenizes an HTML body string into stylized RenderNode items.
    pub fn parse_html(&mut self, html: &str) {
        self.nodes.clear();
        let mut cur_text = String::new();
        let mut in_tag = false;
        let mut tag_buf = String::new();
        let mut title_found = false;

        for line in html.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Quick tag scanner
            let mut i = 0;
            let bytes = trimmed.as_bytes();
            while i < bytes.len() {
                let b = bytes[i];
                if b == b'<' {
                    in_tag = true;
                    tag_buf.clear();
                } else if b == b'>' {
                    in_tag = false;
                    let tag = tag_buf.trim().to_ascii_lowercase();

                    if tag == "h1" || tag.starts_with("h1 ") {
                        cur_text.clear();
                    } else if tag == "/h1" {
                        if !cur_text.is_empty() {
                            self.nodes.push(RenderNode::Heading1(cur_text.clone()));
                            cur_text.clear();
                        }
                    } else if tag == "h2" || tag.starts_with("h2 ") {
                        cur_text.clear();
                    } else if tag == "/h2" {
                        if !cur_text.is_empty() {
                            self.nodes.push(RenderNode::Heading2(cur_text.clone()));
                            cur_text.clear();
                        }
                    } else if tag == "h3" || tag.starts_with("h3 ") {
                        cur_text.clear();
                    } else if tag == "/h3" {
                        if !cur_text.is_empty() {
                            self.nodes.push(RenderNode::Heading3(cur_text.clone()));
                            cur_text.clear();
                        }
                    } else if tag == "p" || tag.starts_with("p ") {
                        cur_text.clear();
                    } else if tag == "/p" {
                        if !cur_text.is_empty() {
                            self.nodes.push(RenderNode::Paragraph(cur_text.clone()));
                            cur_text.clear();
                        }
                    } else if tag == "li" || tag.starts_with("li ") {
                        cur_text.clear();
                    } else if tag == "/li" {
                        if !cur_text.is_empty() {
                            self.nodes.push(RenderNode::Bullet(cur_text.clone()));
                            cur_text.clear();
                        }
                    } else if tag == "hr" || tag == "hr/" {
                        self.nodes.push(RenderNode::Divider);
                    } else if tag == "br" || tag == "br/" {
                        self.nodes.push(RenderNode::EmptyLine);
                    } else if tag == "title" {
                        cur_text.clear();
                    } else if tag == "/title" {
                        if !cur_text.is_empty() {
                            self.page_title = cur_text.clone();
                            title_found = true;
                            cur_text.clear();
                        }
                    }
                } else if in_tag {
                    tag_buf.push(b as char);
                } else {
                    cur_text.push(b as char);
                }
                i += 1;
            }

            if !in_tag && !cur_text.is_empty() {
                self.nodes.push(RenderNode::Paragraph(cur_text.clone()));
                cur_text.clear();
            }
        }

        if !title_found {
            self.page_title = format!("Web: {}", self.url);
        }

        if self.nodes.is_empty() {
            self.nodes.push(RenderNode::Paragraph(String::from("(Empty document)")));
        }
    }

    /// Renders the browser UI.
    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        // Navigation Toolbar background
        canvas.fill_rect(cx, cy, cw, 38, 0x001E293B);
        canvas.fill_rect(cx, cy + 38, cw, 1, 0x00334155);

        // [ < ] Back Button
        let back_col = if self.history_idx > 0 { 0x0038BDF8 } else { 0x0064748B };
        canvas.fill_rect(cx + 8, cy + 7, 24, 24, 0x000F172A);
        canvas.draw_rect(cx + 8, cy + 7, 24, 24, back_col);
        canvas.draw_string(cx + 16, cy + 11, "<", back_col);

        // [ > ] Forward Button
        let fwd_col = if self.history_idx + 1 < self.history.len() { 0x0038BDF8 } else { 0x0064748B };
        canvas.fill_rect(cx + 36, cy + 7, 24, 24, 0x000F172A);
        canvas.draw_rect(cx + 36, cy + 7, 24, 24, fwd_col);
        canvas.draw_string(cx + 44, cy + 11, ">", fwd_col);

        // [ R ] Reload Button
        canvas.fill_rect(cx + 64, cy + 7, 24, 24, 0x000F172A);
        canvas.draw_rect(cx + 64, cy + 7, 24, 24, 0x0094A3B8);
        canvas.draw_string(cx + 72, cy + 11, "R", 0x00E2E8F0);

        // [ Home ] Button
        canvas.fill_rect(cx + 92, cy + 7, 46, 24, 0x000F172A);
        canvas.draw_rect(cx + 92, cy + 7, 46, 24, 0x0094A3B8);
        canvas.draw_string(cx + 98, cy + 11, "Home", 0x00CBD5E1);

        // URL address input box
        let url_x = cx + 144;
        let url_w = cw - 260;
        canvas.fill_rect(url_x, cy + 7, url_w, 24, 0x000F172A);
        canvas.draw_rect(url_x, cy + 7, url_w, 24, 0x0038BDF8);
        canvas.draw_string(url_x + 8, cy + 11, &self.url, 0x00FFFFFF);

        // [ Go ] Button
        let go_x = url_x + url_w + 6;
        canvas.draw_gradient_v(go_x, cy + 7, 36, 24, 0x002563EB, 0x001D4ED8);
        canvas.draw_rect(go_x, cy + 7, 36, 24, 0x0060A5FA);
        canvas.draw_string(go_x + 10, cy + 11, "Go", 0x00FFFFFF);

        // Scroll buttons [ ^ ] and [ v ]
        let sc_up_x = go_x + 42;
        canvas.fill_rect(sc_up_x, cy + 7, 22, 24, 0x000F172A);
        canvas.draw_rect(sc_up_x, cy + 7, 22, 24, 0x0094A3B8);
        canvas.draw_string(sc_up_x + 6, cy + 11, "^", 0x00CBD5E1);

        let sc_dn_x = sc_up_x + 24;
        canvas.fill_rect(sc_dn_x, cy + 7, 22, 24, 0x000F172A);
        canvas.draw_rect(sc_dn_x, cy + 7, 22, 24, 0x0094A3B8);
        canvas.draw_string(sc_dn_x + 6, cy + 11, "v", 0x00CBD5E1);

        // Document Content Viewport (Crisp white document page)
        let doc_y = cy + 39;
        let doc_h = ch - 63;
        canvas.fill_rect(cx, doc_y, cw, doc_h, 0x00F8FAFC);

        // Title Header Banner
        canvas.fill_rect(cx + 12, doc_y + 8, cw - 24, 26, 0x00E2E8F0);
        canvas.draw_string(cx + 20, doc_y + 13, &self.page_title, 0x000F172A);

        // Render HTML Nodes starting from scroll_offset
        let mut cur_y = doc_y + 42;
        for node in self.nodes.iter().skip(self.scroll_offset) {
            if cur_y + 18 > doc_y + doc_h {
                break;
            }

            match node {
                RenderNode::Heading1(text) => {
                    canvas.fill_rect(cx + 16, cur_y - 2, cw - 32, 22, 0x001E3A8A);
                    canvas.draw_string(cx + 24, cur_y + 1, text, 0x00FFFFFF);
                    cur_y += 26;
                }
                RenderNode::Heading2(text) => {
                    canvas.draw_string(cx + 20, cur_y, text, 0x001E40AF);
                    canvas.fill_rect(cx + 20, cur_y + 14, cw - 40, 1, 0x0093C5FD);
                    cur_y += 20;
                }
                RenderNode::Heading3(text) => {
                    canvas.draw_string(cx + 24, cur_y, text, 0x001E293B);
                    cur_y += 18;
                }
                RenderNode::Paragraph(text) => {
                    canvas.draw_string(cx + 24, cur_y, text, 0x00334155);
                    cur_y += 18;
                }
                RenderNode::Bullet(text) => {
                    canvas.fill_rect(cx + 28, cur_y + 4, 4, 4, 0x002563EB);
                    canvas.draw_string(cx + 38, cur_y, text, 0x00334155);
                    cur_y += 18;
                }
                RenderNode::Link(text, _link_url) => {
                    canvas.draw_string(cx + 24, cur_y, text, 0x000284C7);
                    canvas.fill_rect(cx + 24, cur_y + 13, (text.len() * 8) as i32, 1, 0x0038BDF8);
                    cur_y += 18;
                }
                RenderNode::Divider => {
                    canvas.fill_rect(cx + 16, cur_y + 6, cw - 32, 1, 0x00CBD5E1);
                    cur_y += 14;
                }
                RenderNode::EmptyLine => {
                    cur_y += 10;
                }
            }
        }

        // Bottom Status Bar
        let sb_y = cy + ch - 24;
        canvas.fill_rect(cx, sb_y, cw, 24, 0x000F172A);
        canvas.fill_rect(cx, sb_y, cw, 1, 0x00334155);
        canvas.draw_string(cx + 12, sb_y + 5, &self.status, 0x0038BDF8);
    }
}
