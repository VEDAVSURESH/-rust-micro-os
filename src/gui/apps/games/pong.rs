//! Pong Arcade Game for JainOS.

use alloc::format;
use crate::gui::canvas::Canvas;

pub struct PongGame {
    pub player_y: i32,
    pub ai_y: i32,
    pub paddle_h: i32,
    pub paddle_w: i32,
    pub ball_x: i32,
    pub ball_y: i32,
    pub ball_dx: i32,
    pub ball_dy: i32,
    pub ball_size: i32,
    pub score_player: u32,
    pub score_ai: u32,
    pub game_over: bool,
    pub last_tick: u64,
}

impl PongGame {
    pub fn new() -> Self {
        let mut game = Self {
            player_y: 100,
            ai_y: 100,
            paddle_h: 60,
            paddle_w: 12,
            ball_x: 200,
            ball_y: 120,
            ball_dx: 4,
            ball_dy: 3,
            ball_size: 10,
            score_player: 0,
            score_ai: 0,
            game_over: false,
            last_tick: 0,
        };
        game.reset_ball();
        game
    }

    pub fn reset_ball(&mut self) {
        self.ball_x = 240;
        self.ball_y = 130;
        self.ball_dx = if (self.score_player + self.score_ai) % 2 == 0 { 4 } else { -4 };
        self.ball_dy = 2;
    }

    pub fn reset_game(&mut self) {
        self.score_player = 0;
        self.score_ai = 0;
        self.player_y = 100;
        self.ai_y = 100;
        self.game_over = false;
        self.reset_ball();
    }

    pub fn handle_key(&mut self, c: char) {
        match c {
            'w' | 'W' => {
                self.player_y = (self.player_y - 18).max(10);
            }
            's' | 'S' => {
                self.player_y = (self.player_y + 18).min(200);
            }
            'r' | 'R' => {
                self.reset_game();
            }
            _ => {}
        }
    }

    pub fn update(&mut self, court_w: i32, court_h: i32) {
        if self.game_over {
            return;
        }

        let now = crate::timer::current_ticks();
        if now.saturating_sub(self.last_tick) < 3 {
            return; // ~30ms frame step
        }
        self.last_tick = now;

        // Move ball
        self.ball_x += self.ball_dx;
        self.ball_y += self.ball_dy;

        // Top/Bottom wall bounce
        if self.ball_y <= 4 || self.ball_y + self.ball_size >= court_h - 4 {
            self.ball_dy = -self.ball_dy;
        }

        // Left paddle (Player) collision
        let p_left = 20;
        let p_right = p_left + self.paddle_w;
        if self.ball_x <= p_right && self.ball_x >= p_left {
            if self.ball_y + self.ball_size >= self.player_y && self.ball_y <= self.player_y + self.paddle_h {
                self.ball_dx = self.ball_dx.abs().max(4);
            }
        }

        // Right paddle (AI) collision
        let ai_left = court_w - 20 - self.paddle_w;
        let ai_right = ai_left + self.paddle_w;
        if self.ball_x + self.ball_size >= ai_left && self.ball_x <= ai_right {
            if self.ball_y + self.ball_size >= self.ai_y && self.ball_y <= self.ai_y + self.paddle_h {
                self.ball_dx = -(self.ball_dx.abs().max(4));
            }
        }

        // AI tracking
        if self.ball_x > court_w / 3 {
            let ai_center = self.ai_y + self.paddle_h / 2;
            if self.ball_y < ai_center - 8 {
                self.ai_y = (self.ai_y - 3).max(10);
            } else if self.ball_y > ai_center + 8 {
                self.ai_y = (self.ai_y + 3).min(court_h - self.paddle_h - 10);
            }
        }

        // Score check
        if self.ball_x < 0 {
            self.score_ai += 1;
            if self.score_ai >= 7 {
                self.game_over = true;
            } else {
                self.reset_ball();
            }
        } else if self.ball_x > court_w {
            self.score_player += 1;
            if self.score_player >= 7 {
                self.game_over = true;
            } else {
                self.reset_ball();
            }
        }
    }

    pub fn draw(&mut self, canvas: &mut Canvas, bx: i32, by: i32, bw: i32, bh: i32) {
        let court_w = bw - 20;
        let court_h = bh - 40;
        self.update(court_w, court_h);

        // Court background
        canvas.fill_rect(bx, by, bw, bh, 0x000F172A);
        canvas.draw_rect(bx, by, bw, bh, 0x00334155);

        // Score Header
        canvas.fill_rect(bx, by, bw, 28, 0x001E293B);
        let score_str = format!("PLAYER: {:<2}  |  AI: {:<2}", self.score_player, self.score_ai);
        canvas.draw_string(bx + 16, by + 8, &score_str, 0x0038BDF8);
        canvas.draw_string(bx + bw - 210, by + 8, "W/S: Paddle | R: Reset", 0x0094A3B8);

        let court_x = bx + 10;
        let court_y = by + 34;
        canvas.fill_rect(court_x, court_y, court_w, court_h, 0x00020617);
        canvas.draw_rect(court_x, court_y, court_w, court_h, 0x001E293B);

        // Center dashed net
        let net_x = court_x + court_w / 2;
        let mut ny = court_y + 4;
        while ny < court_y + court_h - 8 {
            canvas.fill_rect(net_x, ny, 2, 8, 0x00334155);
            ny += 16;
        }

        // Player Paddle (Left, Blue)
        canvas.fill_rect(court_x + 20, court_y + self.player_y, self.paddle_w, self.paddle_h, 0x0038BDF8);

        // AI Paddle (Right, Red)
        canvas.fill_rect(court_x + court_w - 20 - self.paddle_w, court_y + self.ai_y, self.paddle_w, self.paddle_h, 0x00F87171);

        // Ball (Yellow)
        canvas.fill_rect(court_x + self.ball_x, court_y + self.ball_y, self.ball_size, self.ball_size, 0x00FACC15);

        // Game Over Banner
        if self.game_over {
            let banner_w = 260;
            let banner_h = 70;
            let gbx = court_x + (court_w - banner_w) / 2;
            let gby = court_y + (court_h - banner_h) / 2;
            canvas.fill_rect(gbx, gby, banner_w, banner_h, 0x001E1B4B);
            canvas.draw_rect(gbx, gby, banner_w, banner_h, 0x0038BDF8);

            let msg = if self.score_player >= 7 { "YOU WIN!" } else { "AI WINS!" };
            let col = if self.score_player >= 7 { 0x004ADE80 } else { 0x00F87171 };
            canvas.draw_string(gbx + 85, gby + 16, msg, col);
            canvas.draw_string(gbx + 40, gby + 40, "Press 'R' to Play Again", 0x00E2E8F0);
        }
    }
}
