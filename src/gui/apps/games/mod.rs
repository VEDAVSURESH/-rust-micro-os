//! JainOS Games Arcade Suite.
//!
//! Includes 4 playable native games:
//! 1. Snake: Classic snake arena with score tracking
//! 2. Pong: Real-time paddle battle vs reactive AI
//! 3. Tic-Tac-Toe: 3x3 interactive board vs strategic AI
//! 4. 2048: Sliding tile numerical puzzle

pub mod game2048;
pub mod pong;
pub mod snake;
pub mod tictactoe;

use crate::gui::apps::games::game2048::Game2048;
use crate::gui::apps::games::pong::PongGame;
use crate::gui::apps::games::snake::SnakeGame;
use crate::gui::apps::games::tictactoe::TicTacToeGame;
use crate::gui::canvas::Canvas;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveGame {
    Launcher,
    Snake,
    Pong,
    TicTacToe,
    Game2048,
}

pub struct GamesApp {
    pub active_game: ActiveGame,
    pub snake: SnakeGame,
    pub pong: PongGame,
    pub tictactoe: TicTacToeGame,
    pub game2048: Game2048,
}

impl GamesApp {
    pub fn new() -> Self {
        Self {
            active_game: ActiveGame::Launcher,
            snake: SnakeGame::new(),
            pong: PongGame::new(),
            tictactoe: TicTacToeGame::new(),
            game2048: Game2048::new(),
        }
    }

    pub fn handle_char(&mut self, c: char) {
        match self.active_game {
            ActiveGame::Launcher => match c {
                '1' => self.active_game = ActiveGame::Snake,
                '2' => self.active_game = ActiveGame::Pong,
                '3' => self.active_game = ActiveGame::TicTacToe,
                '4' => self.active_game = ActiveGame::Game2048,
                _ => {}
            },
            ActiveGame::Snake => {
                if c == '\x1b' || c == 'q' || c == 'Q' {
                    self.active_game = ActiveGame::Launcher;
                } else {
                    self.snake.handle_key(c);
                }
            }
            ActiveGame::Pong => {
                if c == '\x1b' || c == 'q' || c == 'Q' {
                    self.active_game = ActiveGame::Launcher;
                } else {
                    self.pong.handle_key(c);
                }
            }
            ActiveGame::TicTacToe => {
                if c == '\x1b' || c == 'q' || c == 'Q' {
                    self.active_game = ActiveGame::Launcher;
                } else {
                    self.tictactoe.handle_key(c);
                }
            }
            ActiveGame::Game2048 => {
                if c == '\x1b' || c == 'q' || c == 'Q' {
                    self.active_game = ActiveGame::Launcher;
                } else {
                    self.game2048.handle_key(c);
                }
            }
        }
    }

    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32) {
        // Common top navigation: [ Back to Games Menu ]
        if self.active_game != ActiveGame::Launcher {
            if rel_y >= 6 && rel_y <= 30 && rel_x >= 10 && rel_x <= 130 {
                self.active_game = ActiveGame::Launcher;
                return;
            }
        }

        match self.active_game {
            ActiveGame::Launcher => {
                // 4 Game Cards arranged in 2x2 grid
                // Card 1: Snake (16, 50, 240, 120)
                if rel_x >= 16 && rel_x <= 256 && rel_y >= 50 && rel_y <= 170 {
                    self.snake.reset();
                    self.active_game = ActiveGame::Snake;
                }
                // Card 2: Pong (270, 50, 240, 120)
                else if rel_x >= 270 && rel_x <= 510 && rel_y >= 50 && rel_y <= 170 {
                    self.pong.reset_game();
                    self.active_game = ActiveGame::Pong;
                }
                // Card 3: Tic-Tac-Toe (16, 184, 240, 120)
                else if rel_x >= 16 && rel_x <= 256 && rel_y >= 184 && rel_y <= 304 {
                    self.tictactoe.reset();
                    self.active_game = ActiveGame::TicTacToe;
                }
                // Card 4: 2048 (270, 184, 240, 120)
                else if rel_x >= 270 && rel_x <= 510 && rel_y >= 184 && rel_y <= 304 {
                    self.game2048.reset();
                    self.active_game = ActiveGame::Game2048;
                }
            }
            ActiveGame::TicTacToe => {
                // Pass click coordinates to Tic-Tac-Toe
                let cell_size = 54;
                let grid_w = cell_size * 3;
                let grid_x = (540 - grid_w) / 2;
                let grid_y = 46 + 34; // offset for top bar
                self.tictactoe.handle_click(rel_x, rel_y, grid_x, grid_y, cell_size);
            }
            _ => {}
        }
    }

    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        match self.active_game {
            ActiveGame::Launcher => {
                // Master background
                canvas.fill_rect(cx, cy, cw, ch, 0x000F172A);

                // Header Banner
                canvas.draw_gradient_v(cx, cy, cw, 40, 0x001E3A8A, 0x001E40AF);
                canvas.fill_rect(cx, cy + 40, cw, 1, 0x0038BDF8);
                canvas.draw_string(cx + 16, cy + 8, "JAINOS ARCADE - NATIVE RUST RETRO GAMES", 0x00FFFFFF);
                canvas.draw_string(cx + 16, cy + 24, "Click any game card or press [1 - 4] to play", 0x0093C5FD);

                // Card 1: Snake
                self.draw_card(canvas, cx + 16, cy + 50, 240, 120, "1. SNAKE", "Classic arcade serpent", "WASD / Arrows to navigate", 0x0022C55E);

                // Card 2: Pong
                self.draw_card(canvas, cx + 270, cy + 50, 240, 120, "2. PONG", "Retro table tennis vs AI", "W / S to move paddle", 0x0038BDF8);

                // Card 3: Tic-Tac-Toe
                self.draw_card(canvas, cx + 16, cy + 184, 240, 120, "3. TIC-TAC-TOE", "3x3 tactical battle", "Mouse click to place X", 0x00F59E0B);

                // Card 4: 2048
                self.draw_card(canvas, cx + 270, cy + 184, 240, 120, "4. 2048 PUZZLE", "Slide & merge 4x4 numbers", "WASD to shift grid", 0x00EC4899);

                // Footer
                let foot_y = cy + ch - 24;
                canvas.fill_rect(cx, foot_y, cw, 24, 0x00020617);
                canvas.draw_string(cx + 16, foot_y + 6, "Press 'Q' inside any game to return to this Arcade menu.", 0x0094A3B8);
            }
            ActiveGame::Snake => {
                self.draw_game_nav(canvas, cx, cy, cw, "Snake Arcade");
                self.snake.draw(canvas, cx, cy + 32, cw, ch - 32);
            }
            ActiveGame::Pong => {
                self.draw_game_nav(canvas, cx, cy, cw, "Pong Battle");
                self.pong.draw(canvas, cx, cy + 32, cw, ch - 32);
            }
            ActiveGame::TicTacToe => {
                self.draw_game_nav(canvas, cx, cy, cw, "Tic-Tac-Toe Tactical");
                self.tictactoe.draw(canvas, cx, cy + 32, cw, ch - 32);
            }
            ActiveGame::Game2048 => {
                self.draw_game_nav(canvas, cx, cy, cw, "2048 Puzzle");
                self.game2048.draw(canvas, cx, cy + 32, cw, ch - 32);
            }
        }
    }

    fn draw_game_nav(&self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, title: &str) {
        canvas.fill_rect(cx, cy, cw, 32, 0x001E293B);
        canvas.fill_rect(cx, cy + 32, cw, 1, 0x00334155);

        // [ < Menu ] button
        canvas.draw_gradient_v(cx + 10, cy + 4, 110, 24, 0x002563EB, 0x001D4ED8);
        canvas.draw_rect(cx + 10, cy + 4, 110, 24, 0x0060A5FA);
        canvas.draw_string(cx + 18, cy + 8, "< Arcade Hub", 0x00FFFFFF);

        canvas.draw_string(cx + 140, cy + 8, title, 0x0038BDF8);
    }

    fn draw_card(&self, canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, title: &str, desc: &str, sub: &str, border_col: u32) {
        canvas.fill_rect(x, y, w, h, 0x001E293B);
        canvas.draw_rect(x, y, w, h, border_col);

        // Header strip
        canvas.fill_rect(x + 1, y + 1, w - 2, 26, 0x00334155);
        canvas.draw_string(x + 12, y + 7, title, 0x00FFFFFF);

        canvas.draw_string(x + 12, y + 42, desc, 0x00E2E8F0);
        canvas.draw_string(x + 12, y + 64, sub, 0x0094A3B8);

        // Click to Play button
        canvas.fill_rect(x + 12, y + 86, 100, 22, 0x000F172A);
        canvas.draw_rect(x + 12, y + 86, 100, 22, border_col);
        canvas.draw_string(x + 22, y + 90, "PLAY NOW", border_col);
    }
}
