//! 2048 Sliding Tile Puzzle for JainOS.

use alloc::format;
use crate::gui::canvas::Canvas;

pub struct Game2048 {
    pub grid: [[u32; 4]; 4],
    pub score: u32,
    pub high_score: u32,
    pub game_over: bool,
    pub won: bool,
}

impl Game2048 {
    pub fn new() -> Self {
        let mut g = Self {
            grid: [[0; 4]; 4],
            score: 0,
            high_score: 0,
            game_over: false,
            won: false,
        };
        g.reset();
        g
    }

    pub fn reset(&mut self) {
        self.grid = [[0; 4]; 4];
        self.score = 0;
        self.game_over = false;
        self.won = false;
        self.spawn_tile();
        self.spawn_tile();
    }

    pub fn spawn_tile(&mut self) {
        let mut empty = [(0usize, 0usize); 16];
        let mut count = 0;
        for r in 0..4 {
            for c in 0..4 {
                if self.grid[r][c] == 0 {
                    empty[count] = (r, c);
                    count += 1;
                }
            }
        }

        if count == 0 {
            return;
        }

        let seed = (crate::timer::current_ticks() as usize) ^ (self.score as usize);
        let idx = seed % count;
        let (r, c) = empty[idx];
        self.grid[r][c] = if seed % 10 == 0 { 4 } else { 2 };
    }

    pub fn handle_key(&mut self, c: char) {
        let moved = match c {
            'w' | 'W' => self.move_up(),
            's' | 'S' => self.move_down(),
            'a' | 'A' => self.move_left(),
            'd' | 'D' => self.move_right(),
            'r' | 'R' => {
                self.reset();
                return;
            }
            _ => false,
        };

        if moved {
            self.spawn_tile();
            if self.score > self.high_score {
                self.high_score = self.score;
            }
            self.check_game_over();
        }
    }

    fn slide_line(line: &mut [u32; 4]) -> (bool, u32) {
        let mut moved = false;
        let mut score_add = 0;
        let mut non_zero = [0u32; 4];
        let mut nz_count = 0;

        for &v in line.iter() {
            if v != 0 {
                non_zero[nz_count] = v;
                nz_count += 1;
            }
        }

        let mut merged = [0u32; 4];
        let mut m_idx = 0;
        let mut i = 0;
        while i < nz_count {
            if i + 1 < nz_count && non_zero[i] == non_zero[i + 1] {
                let sum = non_zero[i] * 2;
                merged[m_idx] = sum;
                score_add += sum;
                m_idx += 1;
                i += 2;
            } else {
                merged[m_idx] = non_zero[i];
                m_idx += 1;
                i += 1;
            }
        }

        if merged != *line {
            moved = true;
            *line = merged;
        }

        (moved, score_add)
    }

    pub fn move_left(&mut self) -> bool {
        let mut moved = false;
        for r in 0..4 {
            let (m, s) = Self::slide_line(&mut self.grid[r]);
            if m {
                moved = true;
                self.score += s;
            }
        }
        moved
    }

    pub fn move_right(&mut self) -> bool {
        let mut moved = false;
        for r in 0..4 {
            let mut line = [self.grid[r][3], self.grid[r][2], self.grid[r][1], self.grid[r][0]];
            let (m, s) = Self::slide_line(&mut line);
            if m {
                moved = true;
                self.score += s;
                self.grid[r] = [line[3], line[2], line[1], line[0]];
            }
        }
        moved
    }

    pub fn move_up(&mut self) -> bool {
        let mut moved = false;
        for c in 0..4 {
            let mut col = [self.grid[0][c], self.grid[1][c], self.grid[2][c], self.grid[3][c]];
            let (m, s) = Self::slide_line(&mut col);
            if m {
                moved = true;
                self.score += s;
                for r in 0..4 {
                    self.grid[r][c] = col[r];
                }
            }
        }
        moved
    }

    pub fn move_down(&mut self) -> bool {
        let mut moved = false;
        for c in 0..4 {
            let mut col = [self.grid[3][c], self.grid[2][c], self.grid[1][c], self.grid[0][c]];
            let (m, s) = Self::slide_line(&mut col);
            if m {
                moved = true;
                self.score += s;
                for r in 0..4 {
                    self.grid[r][c] = col[3 - r];
                }
            }
        }
        moved
    }

    pub fn check_game_over(&mut self) {
        for r in 0..4 {
            for c in 0..4 {
                if self.grid[r][c] == 2048 {
                    self.won = true;
                }
                if self.grid[r][c] == 0 {
                    return;
                }
                if c + 1 < 4 && self.grid[r][c] == self.grid[r][c + 1] {
                    return;
                }
                if r + 1 < 4 && self.grid[r][c] == self.grid[r + 1][c] {
                    return;
                }
            }
        }
        self.game_over = true;
    }

    pub fn draw(&mut self, canvas: &mut Canvas, bx: i32, by: i32, bw: i32, bh: i32) {
        canvas.fill_rect(bx, by, bw, bh, 0x000F172A);
        canvas.draw_rect(bx, by, bw, bh, 0x00334155);

        // Header
        canvas.fill_rect(bx, by, bw, 28, 0x001E293B);
        let score_str = format!("2048 | SCORE: {:<5} | HIGH: {:<5}", self.score, self.high_score);
        canvas.draw_string(bx + 14, by + 8, &score_str, 0x0038BDF8);
        canvas.draw_string(bx + bw - 180, by + 8, "WASD: Slide | R: Reset", 0x0094A3B8);

        // 4x4 Grid
        let cell_size = 44;
        let pad = 6;
        let grid_size = cell_size * 4 + pad * 5;
        let gx = bx + (bw - grid_size) / 2;
        let gy = by + 34 + (bh - 40 - grid_size) / 2;

        canvas.fill_rect(gx, gy, grid_size, grid_size, 0x001E293B);
        canvas.draw_rect(gx, gy, grid_size, grid_size, 0x00334155);

        for r in 0..4 {
            for c in 0..4 {
                let cx = gx + pad + (c as i32) * (cell_size + pad);
                let cy = gy + pad + (r as i32) * (cell_size + pad);
                let val = self.grid[r][c];

                let (bg_col, txt_col) = match val {
                    0 => (0x00334155, 0x00000000),
                    2 => (0x0038BDF8, 0x000F172A),
                    4 => (0x000EA5E9, 0x00FFFFFF),
                    8 => (0x002563EB, 0x00FFFFFF),
                    16 => (0x001D4ED8, 0x00FFFFFF),
                    32 => (0x0010B981, 0x00FFFFFF),
                    64 => (0x00059669, 0x00FFFFFF),
                    128 => (0x00EAB308, 0x000F172A),
                    256 => (0x00CA8A04, 0x00FFFFFF),
                    512 => (0x00F97316, 0x00FFFFFF),
                    1024 => (0x00EF4444, 0x00FFFFFF),
                    2048 => (0x00DC2626, 0x00FFFFFF),
                    _ => (0x008B5CF6, 0x00FFFFFF),
                };

                canvas.fill_rect(cx, cy, cell_size, cell_size, bg_col);

                if val > 0 {
                    let s = format!("{}", val);
                    let tx = cx + (cell_size - (s.len() as i32 * 8)) / 2;
                    let ty = cy + (cell_size - 12) / 2;
                    canvas.draw_string(tx, ty, &s, txt_col);
                }
            }
        }

        // Overlay Game Over
        if self.game_over {
            let banner_w = 220;
            let banner_h = 60;
            let ox = gx + (grid_size - banner_w) / 2;
            let oy = gy + (grid_size - banner_h) / 2;
            canvas.fill_rect(ox, oy, banner_w, banner_h, 0x001E1B4B);
            canvas.draw_rect(ox, oy, banner_w, banner_h, 0x00EF4444);
            canvas.draw_string(ox + 60, oy + 12, "NO MOVES LEFT!", 0x00F87171);
            canvas.draw_string(ox + 35, oy + 34, "Press 'R' to Play Again", 0x00E2E8F0);
        }
    }
}
