//! Tic-Tac-Toe Game for JainOS.

use alloc::format;
use crate::gui::canvas::Canvas;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    None,
    X, // Human
    O, // AI
}

pub struct TicTacToeGame {
    pub board: [Player; 9],
    pub winner: Option<Player>,
    pub is_draw: bool,
    pub wins_x: u32,
    pub wins_o: u32,
    pub draws: u32,
}

impl TicTacToeGame {
    pub fn new() -> Self {
        Self {
            board: [Player::None; 9],
            winner: None,
            is_draw: false,
            wins_x: 0,
            wins_o: 0,
            draws: 0,
        }
    }

    pub fn reset(&mut self) {
        self.board = [Player::None; 9];
        self.winner = None;
        self.is_draw = false;
    }

    pub fn handle_key(&mut self, c: char) {
        if c == 'r' || c == 'R' {
            self.reset();
        }
    }

    pub fn handle_click(&mut self, rel_x: i32, rel_y: i32, grid_x: i32, grid_y: i32, cell_size: i32) {
        // [ New Game ] button click
        if rel_y < 30 && rel_x >= 320 && rel_x <= 420 {
            self.reset();
            return;
        }

        if self.winner.is_some() || self.is_draw {
            self.reset();
            return;
        }

        let gx = rel_x - grid_x;
        let gy = rel_y - grid_y;
        if gx < 0 || gy < 0 || gx >= cell_size * 3 || gy >= cell_size * 3 {
            return;
        }

        let col = (gx / cell_size) as usize;
        let row = (gy / cell_size) as usize;
        let idx = row * 3 + col;

        if idx < 9 && self.board[idx] == Player::None {
            // Human move
            self.board[idx] = Player::X;
            self.check_game_state();

            if self.winner.is_none() && !self.is_draw {
                // AI move (first available cell or smart center/corner)
                self.make_ai_move();
                self.check_game_state();
            }
        }
    }

    fn make_ai_move(&mut self) {
        // Simple strategic AI: take center if open, or block, or first empty
        let pref_order = [4, 0, 2, 6, 8, 1, 3, 5, 7];
        for &cell in &pref_order {
            if self.board[cell] == Player::None {
                self.board[cell] = Player::O;
                return;
            }
        }
    }

    fn check_game_state(&mut self) {
        let lines = [
            [0, 1, 2], [3, 4, 5], [6, 7, 8], // Rows
            [0, 3, 6], [1, 4, 7], [2, 5, 8], // Cols
            [0, 4, 8], [2, 4, 6],             // Diagonals
        ];

        for &line in &lines {
            let p = self.board[line[0]];
            if p != Player::None && p == self.board[line[1]] && p == self.board[line[2]] {
                self.winner = Some(p);
                if p == Player::X {
                    self.wins_x += 1;
                } else {
                    self.wins_o += 1;
                }
                return;
            }
        }

        // Check draw
        if !self.board.iter().any(|&c| c == Player::None) {
            self.is_draw = true;
            self.draws += 1;
        }
    }

    pub fn draw(&mut self, canvas: &mut Canvas, bx: i32, by: i32, bw: i32, bh: i32) {
        // Background
        canvas.fill_rect(bx, by, bw, bh, 0x000F172A);
        canvas.draw_rect(bx, by, bw, bh, 0x00334155);

        // Header
        canvas.fill_rect(bx, by, bw, 28, 0x001E293B);
        let score_str = format!("X (You): {}  |  O (AI): {}  |  Ties: {}", self.wins_x, self.wins_o, self.draws);
        canvas.draw_string(bx + 14, by + 8, &score_str, 0x0038BDF8);

        // Reset button
        canvas.fill_rect(bx + bw - 110, by + 4, 96, 20, 0x002563EB);
        canvas.draw_string(bx + bw - 100, by + 6, "[ New Game ]", 0x00FFFFFF);

        // 3x3 Grid
        let cell_size = 54;
        let grid_w = cell_size * 3;
        let grid_x = bx + (bw - grid_w) / 2;
        let grid_y = by + 46;

        canvas.fill_rect(grid_x, grid_y, grid_w, grid_w, 0x00020617);
        canvas.draw_rect(grid_x, grid_y, grid_w, grid_w, 0x0038BDF8);

        // Grid lines
        for i in 1..3 {
            canvas.fill_rect(grid_x + i * cell_size, grid_y, 2, grid_w, 0x00334155);
            canvas.fill_rect(grid_x, grid_y + i * cell_size, grid_w, 2, 0x00334155);
        }

        // Draw symbols
        for r in 0..3 {
            for c in 0..3 {
                let p = self.board[r * 3 + c];
                let cx = grid_x + (c as i32) * cell_size + 20;
                let cy = grid_y + (r as i32) * cell_size + 18;

                match p {
                    Player::X => {
                        canvas.draw_string(cx, cy, "X", 0x0038BDF8);
                    }
                    Player::O => {
                        canvas.draw_string(cx, cy, "O", 0x00F87171);
                    }
                    Player::None => {}
                }
            }
        }

        // Status banner
        let status_y = grid_y + grid_w + 14;
        if let Some(w) = self.winner {
            let msg = if w == Player::X { "VICTORY! You defeated the AI." } else { "DEFEAT! AI wins this round." };
            let col = if w == Player::X { 0x004ADE80 } else { 0x00F87171 };
            canvas.draw_string(bx + 40, status_y, msg, col);
            canvas.draw_string(bx + 40, status_y + 18, "Click board or press 'R' to play again.", 0x00CBD5E1);
        } else if self.is_draw {
            canvas.draw_string(bx + 60, status_y, "DRAW GAME! Well contested.", 0x00FACC15);
            canvas.draw_string(bx + 40, status_y + 18, "Click board or press 'R' to play again.", 0x00CBD5E1);
        } else {
            canvas.draw_string(bx + 60, status_y, "Click any cell to place your 'X'", 0x0094A3B8);
        }
    }
}
