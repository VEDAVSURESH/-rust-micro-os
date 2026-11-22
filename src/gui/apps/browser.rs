//! Modern Native Web Browser Application for JainOS (v0.3.1).
//!
//! Features:
//! - Real HTTP/1.1 TCP client over bare-metal socket (`crate::net::http_get`)
//! - Modern browser UI: tab bar, toolbar, omnibox, Back/Forward/Reload/Home
//! - Clickable hyperlinks with mouse hit-testing & auto-navigation
//! - Multi-level HTML renderer with word-wrapping (H1-H3, paragraphs, lists, hr, tables)
//! - Script (<script>) and Style (<style>) filter so inline code is stripped cleanly
//! - HTML entity decoding (&amp;, &lt;, &gt;, &quot;, &nbsp;, etc.)
//! - Proportional vertical scrollbar with interactive up/down controls
//! - History stack with Back and Forward navigation

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
    Paragraph(String),
    Bullet(String),
    Link { text: String, url: String },
    Divider,
    TableRow(Vec<String>),
    EmptyLine,
}

pub struct BrowserApp {
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
    pub hovered_link: Option<String>,
}

impl BrowserApp {
    pub fn new() -> Self {
        let mut app = Self {
            url: String::from("http://example.com/"),
            status: String::from("HTTP 200 OK • Document loaded"),
            page_title: String::from("Example Domain"),
            history: Vec::new(),
            history_idx: 0,
            blocks: Vec::new(),
            link_hitboxes: Vec::new(),
            scroll_y: 0,
            total_doc_height: 400,
            is_loading: false,
            hovered_link: None,
        };

        app.history.push(app.url.clone());
        app.load_default_homepage();
        app
    }

    /// Loads the default welcome homepage.
    fn load_default_homepage(&mut self) {
        self.page_title = String::from("Example Domain");
        self.status = String::from("HTTP 200 OK • Example Domain loaded");
        self.blocks.clear();
        self.blocks.push(HtmlBlock::Heading1(String::from("Example Domain")));
        self.blocks.push(HtmlBlock::Divider);
        self.blocks.push(HtmlBlock::Paragraph(String::from(
            "This domain is for use in documentation examples without needing permission. \
             This is not a service; avoid relying on it for testing and monitoring purposes."
        )));
        self.blocks.push(HtmlBlock::EmptyLine);
        self.blocks.push(HtmlBlock::Heading2(String::from("JainOS Browser Capabilities")));
        self.blocks.push(HtmlBlock::Bullet(String::from("Bare-metal TCP socket stack with SYN/ACK flow control")));
        self.blocks.push(HtmlBlock::Bullet(String::from("Domain Name System resolution over UDP port 53")));
        self.blocks.push(HtmlBlock::Bullet(String::from("HTTP/1.1 protocol compliant client with keep-alive handling")));
        self.blocks.push(HtmlBlock::Bullet(String::from("HTML tokenizer with word-wrapping, headings, and clickable links")));
        self.blocks.push(HtmlBlock::EmptyLine);
        self.blocks.push(HtmlBlock::Heading3(String::from("Tested Real Websites")));
        self.blocks.push(HtmlBlock::Link {
            text: String::from("http://example.com/ (IANA Example Domain)"),
            url: String::from("http://example.com/"),
        });
        self.blocks.push(HtmlBlock::Link {
            text: String::from("http://neverssl.com/ (Unencrypted Portal & Connectivity Test)"),
            url: String::from("http://neverssl.com/"),
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
                if self.url.len() < 70 {
                    self.url.push(c);
                }
            }
            _ => {}
        }
    }

    /// Navigates to a target URL, updating history stack.
    pub fn navigate_to(&mut self, mut target_url: String) {
        if !target_url.starts_with("http://") && !target_url.starts_with("https://") {
            target_url = format!("http://{}", target_url);
        }
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

    /// Handles mouse clicks inside the Browser window.
    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32) {
        // --- Toolbar Navigation Controls (y: 26..60) ---
        if rel_y >= 26 && rel_y <= 58 {
            // [ ◄ ] Back Button at (8, 28, 26, 26)
            if rel_x >= 8 && rel_x < 34 {
                self.go_back();
                return;
            }
            // [ ► ] Forward Button at (38, 28, 26, 26)
            if rel_x >= 38 && rel_x < 64 {
                self.go_forward();
                return;
            }
            // [ ⟳ ] Reload Button at (68, 28, 26, 26)
            if rel_x >= 68 && rel_x < 94 {
                self.fetch_url();
                return;
            }
            // [ ⌂ ] Home Button at (98, 28, 26, 26)
            if rel_x >= 98 && rel_x < 124 {
                self.navigate_to(String::from("http://example.com/"));
                return;
            }
            // [ Go ] Button at (rel_x >= 520)
            if rel_x >= 520 && rel_x < 576 {
                let target = self.url.clone();
                self.navigate_to(target);
                return;
            }
        }

        // --- Scrollbar Clicks (Right 16px of document area: rel_x: 560..578) ---
        if rel_x >= 560 && rel_x <= 580 && rel_y >= 64 {
            // Top arrow [ ▲ ]
            if rel_y <= 80 {
                self.scroll_y = (self.scroll_y - 40).max(0);
                return;
            }
            // Bottom arrow [ ▼ ]
            if rel_y >= 360 {
                self.scroll_y = (self.scroll_y + 40).min((self.total_doc_height - 200).max(0));
                return;
            }
            // Track click: scroll down or up
            if rel_y < 200 {
                self.scroll_y = (self.scroll_y - 60).max(0);
            } else {
                self.scroll_y = (self.scroll_y + 60).min((self.total_doc_height - 200).max(0));
            }
            return;
        }

        // --- Document Area Hyperlink Hit-Testing (y: 64..ch - 24) ---
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
                self.page_title = format!("Connection Alert - {}", self.url);
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
                    "Check network adapter and gateway 10.0.2.2 in the Network application"
                )));
                self.blocks.push(HtmlBlock::Bullet(String::from(
                    "Ensure target server supports unencrypted HTTP on port 80"
                )));
                self.blocks.push(HtmlBlock::Bullet(String::from(
                    "Verify DNS query to gateway 10.0.2.3 succeeds"
                )));
                self.blocks.push(HtmlBlock::EmptyLine);
                self.blocks.push(HtmlBlock::Link {
                    text: String::from("Click here to return to Example Domain"),
                    url: String::from("http://example.com/"),
                });
            }
        }
        self.is_loading = false;
    }

    /// Tokenizes HTML document, ignoring <style> and <script>, decoding entities.
    pub fn parse_html(&mut self, html: &str) {
        self.blocks.clear();
        let mut cur_text = String::new();
        let mut in_tag = false;
        let mut tag_buf = String::new();
        let mut title_found = false;
        let mut skip_content = false; // true while inside <style> or <script>

        let mut i = 0;
        let bytes = html.as_bytes();
        let len = bytes.len();

        while i < len {
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
                        // Extract href
                        let href = extract_href(&tag_buf);
                        cur_text.clear();
                        // Next text will be link text until </a>
                        // Store href temporarily by appending marker
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

        // --- ROW 1: TAB BAR (y: cy, h: 26) ---
        canvas.fill_rect(cx, cy, cw, 26, 0x000F172A);
        canvas.fill_rect(cx, cy + 25, cw, 1, 0x00334155);

        // Active Tab Pill
        let tab_w = 190.min(cw - 60);
        canvas.draw_gradient_v(cx + 8, cy + 3, tab_w, 23, 0x001E3A8A, 0x001E293B);
        canvas.draw_rect(cx + 8, cy + 3, tab_w, 23, 0x0038BDF8);

        // Tab globe icon & title
        canvas.fill_rect(cx + 14, cy + 9, 8, 8, 0x0038BDF8);
        let mut tab_title = self.page_title.clone();
        if tab_title.len() > 16 {
            tab_title.truncate(14);
            tab_title.push_str("..");
        }
        canvas.draw_string(cx + 26, cy + 7, &tab_title, 0x00FFFFFF);
        canvas.draw_string(cx + tab_w - 10, cy + 7, "x", 0x0094A3B8);

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
        let omni_w = cw - 188;
        canvas.fill_rect(omni_x, tb_y + 5, omni_w, 26, 0x000F172A);
        canvas.draw_rect(omni_x, tb_y + 5, omni_w, 26, 0x0038BDF8);

        // Security / Protocol badge inside Omnibox
        canvas.fill_rect(omni_x + 3, tb_y + 7, 36, 20, 0x001E293B);
        canvas.draw_string(omni_x + 6, tb_y + 9, "HTTP", 0x0038BDF8);

        // URL text
        let mut display_url = self.url.clone();
        let max_url_chars = ((omni_w - 50) / 8).max(10) as usize;
        if display_url.len() > max_url_chars {
            display_url.truncate(max_url_chars - 2);
            display_url.push_str("..");
        }
        canvas.draw_string(omni_x + 44, tb_y + 9, &display_url, 0x00FFFFFF);

        // [ Go ] Button
        let go_x = omni_x + omni_w + 6;
        canvas.draw_gradient_v(go_x, tb_y + 5, 42, 26, 0x002563EB, 0x001D4ED8);
        canvas.draw_rect(go_x, tb_y + 5, 42, 26, 0x0060A5FA);
        canvas.draw_string(go_x + 12, tb_y + 9, "Go", 0x00FFFFFF);

        // --- ROW 3: WEBPAGE DOCUMENT VIEWPORT ---
        let doc_y = tb_y + 36;
        let doc_h = ch - 62 - 24;
        let doc_w = cw - 16; // 16px reserved for scrollbar
        canvas.fill_rect(cx, doc_y, cw, doc_h, 0x00F8FAFC); // Clean paper white canvas

        // Document word-wrap width: (doc_w - 40) / 8 chars per line
        let max_line_chars = ((doc_w - 48) / 8).max(20) as usize;

        // Render HTML Blocks with word wrapping and bounding box tracking
        let mut cur_y = doc_y + 16 - self.scroll_y;
        let content_start_y = cur_y;

        for block in &self.blocks {
            match block {
                HtmlBlock::Heading1(text) => {
                    if cur_y + 26 >= doc_y && cur_y <= doc_y + doc_h {
                        // Navy blue banner header
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
                                canvas.fill_rect(cx + 24, cur_y + 5, 5, 5, 0x002563EB); // Electric blue bullet
                                canvas.draw_string(cx + 34, cur_y, line, 0x00334155);
                            } else {
                                canvas.draw_string(cx + 34, cur_y, line, 0x00334155);
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
                            canvas.fill_rect(cx + 20, cur_y + 13, link_w, 1, 0x0038BDF8); // Underline

                            // Record clickable link bounding box
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
        let engine_str = "JainOS WebEngine v0.3.1 (No-STD)";
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
