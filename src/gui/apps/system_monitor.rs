//! System Monitor & Process Telemetry Application for JainOS.
//!
//! Provides real-time heap memory usage gauge, physical frame statistics,
//! system uptime, and live cooperative task scheduler status.

use alloc::format;
use crate::gui::canvas::Canvas;

pub struct SystemMonitorApp {}

impl SystemMonitorApp {
    pub fn new() -> Self {
        Self {}
    }

    /// Renders System Monitor UI.
    pub fn draw(&mut self, canvas: &mut Canvas, cx: i32, cy: i32, cw: i32, ch: i32) {
        // Background
        canvas.fill_rect(cx, cy, cw, ch, 0x000F172A);

        // Section 1: Kernel Heap Memory Gauge Box
        let box1_y = cy + 12;
        let box1_h = 96;
        canvas.fill_rect(cx + 12, box1_y, cw - 24, box1_h, 0x001E293B);
        canvas.draw_rect(cx + 12, box1_y, cw - 24, box1_h, 0x00334155);

        let stats = crate::allocator::ALLOCATOR.stats();
        let total_kb = stats.heap_size / 1024;
        let used_kb = stats.total_allocated_bytes / 1024;
        let free_kb = stats.total_free_bytes / 1024;
        let pct = if stats.heap_size > 0 {
            (stats.total_allocated_bytes * 100) / stats.heap_size
        } else {
            0
        };

        let mem_title = format!("Kernel Heap Memory: {} KiB Total ({}% Used)", total_kb, pct);
        canvas.draw_string(cx + 24, box1_y + 10, &mem_title, 0x0038BDF8);

        // Memory usage bar
        let bar_x = cx + 24;
        let bar_y = box1_y + 32;
        let bar_w = cw - 48;
        let bar_h = 22;

        canvas.fill_rect(bar_x, bar_y, bar_w, bar_h, 0x000A0E17);
        canvas.draw_rect(bar_x, bar_y, bar_w, bar_h, 0x00475569);

        let filled_w = ((bar_w as usize * stats.total_allocated_bytes) / stats.heap_size.max(1)) as i32;
        if filled_w > 0 {
            canvas.draw_gradient_v(bar_x + 1, bar_y + 1, filled_w, bar_h - 2, 0x002563EB, 0x001D4ED8);
        }

        let stat_detail = format!(
            "Used: {} KiB | Free: {} KiB | Allocs: {} | Frees: {}",
            used_kb, free_kb, stats.alloc_count, stats.dealloc_count
        );
        canvas.draw_string(cx + 24, box1_y + 64, &stat_detail, 0x00CBD5E1);

        // Section 2: Hardware & Uptime Telemetry Box
        let box2_y = box1_y + box1_h + 12;
        let box2_h = 58;
        canvas.fill_rect(cx + 12, box2_y, cw - 24, box2_h, 0x001E293B);
        canvas.draw_rect(cx + 12, box2_y, cw - 24, box2_h, 0x00334155);

        let ticks = crate::timer::current_ticks();
        let uptime_secs = ticks / 100;
        let uptime_str = format!("System Uptime: {} seconds (Total PIT Ticks: {})", uptime_secs, ticks);
        canvas.draw_string(cx + 24, box2_y + 10, &uptime_str, 0x004ADE80);

        if let Some(mem) = crate::memory::get_physical_memory_info() {
            let phys_str = format!(
                "Physical RAM: {} frames safe usable (~{} MiB) | {} BIOS regions",
                mem.total_usable_frames,
                mem.total_usable_bytes / (1024 * 1024),
                mem.total_regions
            );
            canvas.draw_string(cx + 24, box2_y + 32, &phys_str, 0x0094A3B8);
        }

        // Section 3: Cooperative Task Scheduler Table
        let table_y = box2_y + box2_h + 12;
        let table_h = cy + ch - table_y - 12;
        canvas.fill_rect(cx + 12, table_y, cw - 24, table_h, 0x00131B2E);
        canvas.draw_rect(cx + 12, table_y, cw - 24, table_h, 0x00334155);

        // Table header
        canvas.fill_rect(cx + 12, table_y, cw - 24, 24, 0x001E293B);
        canvas.draw_string(cx + 24, table_y + 5, "TID", 0x0038BDF8);
        canvas.draw_string(cx + 80, table_y + 5, "Task Name", 0x0038BDF8);
        canvas.draw_string(cx + 240, table_y + 5, "Status", 0x0038BDF8);
        canvas.draw_string(cx + 340, table_y + 5, "Ticks", 0x0038BDF8);

        let sched = crate::task::SCHEDULER.lock();
        let mut row_y = table_y + 32;
        sched.for_each_task(|tcb| {
            if row_y + 20 <= table_y + table_h {
                let tid_str = format!("{}", tcb.id.0);
                canvas.draw_string(cx + 24, row_y, &tid_str, 0x00FFFFFF);
                canvas.draw_string(cx + 80, row_y, tcb.name, 0x00F1F5F9);
                let (state_str, state_col) = match tcb.state() {
                    crate::task::TaskState::Running => ("RUNNING", 0x0022C55E),
                    crate::task::TaskState::Ready => ("READY", 0x0038BDF8),
                    crate::task::TaskState::Blocked => ("BLOCKED", 0x00E2E8F0),
                    crate::task::TaskState::Terminated => ("STOPPED", 0x0064748B),
                };
                canvas.draw_string(cx + 240, row_y, state_str, state_col);
                let tick_str = format!("{}", tcb.total_ticks);
                canvas.draw_string(cx + 340, row_y, &tick_str, 0x0094A3B8);

                row_y += 22;
            }
        });
    }
}
