//! Snake Arcade Game for JainOS.

use alloc::format;
use alloc::vec::Vec;
use crate::gui::canvas::Canvas;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub struct SnakeGame {
    pub snake: Vec<(i32, i32)>, // (grid_x, grid_y)
    pub dir: Direction,
    pub next_dir: Direction,
    pub food: (i32, i32),
    pub score: u32,
    pub high_score: u32,
    pub game_over: bool,
    pub grid_w: i32,
    pub grid_h: i32,
    pub last_tick: u64,
}

impl SnakeGame {
    pub fn new() -> Self {
        let mut game = Self {
            snake: Vec::new(),
            dir: Direction::Right,
            next_dir: Direction::Right,
            food: (12, 10),
            score: 0,
            high_score: 0,
            game_over: false,
            grid_w: 24,
            grid_h: 18,
            last_tick: 0,
        };
        game.reset();
        game
    }

    pub fn reset(&mut self) {
        self.snake.clear();
        self.snake.push((8, 9));
        self.snake.push((7, 9));
        self.snake.push((6, 9));
        self.dir = Direction::Right;
        self.next_dir = Direction::Right;
        self.score = 0;
        self.game_over = false;
        self.spawn_food();
    }

    pub fn spawn_food(&mut self) {
        let seed = crate::timer::current_ticks() as i32;
        let fx = ((seed * 7 + 13) % self.grid_w).abs();
        let fy = ((seed * 13 + 5) % self.grid_h).abs();
        self.food = (fx, fy);
    }

    pub fn handle_key(&mut self, c: char) {
        match c {
            'w' | 'W' => {
                if self.dir != Direction::Down {
                    self.next_dir = Direction::Up;
                }
            }
            's' | 'S' => {
                if self.dir != Direction::Up {
                    self.next_dir = Direction::Down;
                }
            }
            'a' | 'A' => {
                if self.dir != Direction::Right {
                    self.next_dir = Direction::Left;
                }
            }
            'd' | 'D' => {
                if self.dir != Direction::Left {
                    self.next_dir = Direction::Right;
                }
            }
            'r' | 'R' => {
                self.reset();
            }
            _ => {}
        }
    }

    pub fn update(&mut self) {
        if self.game_over {
            return;
        }

        let now = crate::timer::current_ticks();
        if now.saturating_sub(self.last_tick) < 8 {
            return; // Move every ~80ms
        }
        self.last_tick = now;

        self.dir = self.next_dir;
        let head = self.snake[0];
        let new_head = match self.dir {
            Direction::Up => (head.0, head.1 - 1),
            Direction::Down => (head.0, head.1 + 1),
            Direction::Left => (head.0 - 1, head.1),
            Direction::Right => (head.0 + 1, head.1),
        };

        // Wall collisions
        if new_head.0 < 0 || new_head.0 >= self.grid_w || new_head.1 < 0 || new_head.1 >= self.grid_h {
            self.game_over = true;
            return;
        }

        // Self collision
        for seg in &self.snake {
            if *seg == new_head {
                self.game_over = true;
                return;
            }
        }

        self.snake.insert(0, new_head);

        // Food collision
        if new_head == self.food {
            self.score += 10;
            if self.score > self.high_score {
                self.high_score = self.score;
            }
            self.spawn_food();
        } else {
            self.snake.pop();
        }
    }

    pub fn draw(&mut self, canvas: &mut Canvas, bx: i32, by: i32, bw: i32, bh: i32) {
        self.update();

        // Arena background
        canvas.fill_rect(bx, by, bw, bh, 0x000F172A);
        canvas.draw_rect(bx, by, bw, bh, 0x00334155);

        // Header bar with Score
        canvas.fill_rect(bx, by, bw, 28, 0x001E293B);
        let score_str = format!("SCORE: {:<4} | HIGH: {:<4}", self.score, self.high_score);
        canvas.draw_string(bx + 12, by + 8, &score_str, 0x0038BDF8);
        canvas.draw_string(bx + bw - 180, by + 8, "WASD: Move | R: Reset", 0x0094A3B8);

        // Grid parameters
        let cell_size = 14;
        let arena_x = bx + (bw - self.grid_w * cell_size) / 2;
        let arena_y = by + 34 + (bh - 40 - self.grid_h * cell_size) / 2;

        // Arena border
        canvas.fill_rect(arena_x - 2, arena_y - 2, self.grid_w * cell_size + 4, self.grid_h * cell_size + 4, 0x00020617);
        canvas.draw_rect(arena_x - 2, arena_y - 2, self.grid_w * cell_size + 4, self.grid_h * cell_size + 4, 0x0038BDF8);

        // Food
        let food_px = arena_x + self.food.0 * cell_size;
        let food_py = arena_y + self.food.1 * cell_size;
        canvas.fill_rect(food_px + 2, food_py + 2, cell_size - 4, cell_size - 4, 0x00EF4444);

        // Snake
        for (i, seg) in self.snake.iter().enumerate() {
            let sx = arena_x + seg.0 * cell_size;
            let sy = arena_y + seg.1 * cell_size;
            let color = if i == 0 { 0x0022C55E } else { 0x0016A34A };
            canvas.fill_rect(sx + 1, sy + 1, cell_size - 2, cell_size - 2, color);
        }

        // Game Over Banner
        if self.game_over {
            let banner_w = 240;
            let banner_h = 70;
            let gbx = arena_x + (self.grid_w * cell_size - banner_w) / 2;
            let gby = arena_y + (self.grid_h * cell_size - banner_h) / 2;
            canvas.fill_rect(gbx, gby, banner_w, banner_h, 0x001E1B4B);
            canvas.draw_rect(gbx, gby, banner_w, banner_h, 0x00EF4444);
            canvas.draw_string(gbx + 70, gby + 16, "GAME OVER!", 0x00F87171);
            canvas.draw_string(gbx + 40, gby + 40, "Press 'R' to Play Again", 0x00E2E8F0);
        }
    }
}
