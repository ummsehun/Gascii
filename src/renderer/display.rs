use anyhow::Result;
use crossterm::{
    cursor,
    style::Print,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{BufWriter, Stdout, Write};
use std::time::{Duration, Instant};

use super::cell::CellData;
use crate::shared::constants;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum DisplayMode {
    Ascii,
    Rgb,
}

pub struct DisplayManager {
    stdout: BufWriter<Stdout>,
    mode: DisplayMode,
    rgb_diff_threshold: u8,
    last_cells: Option<Vec<CellData>>,
    render_buffer: Vec<u8>,
    perf_window_start: Instant,
    perf_frames: u64,
    perf_slow_frames: u64,
    perf_total_us: u128,
    perf_diff_us: u128,
    perf_io_us: u128,
}

impl DisplayManager {
    pub fn new(mode: DisplayMode) -> Result<Self> {
        // Use BufWriter
        // Massive output buffer to minimize system call overhead (4MB for smooth 3D rendering)
        let stdout = BufWriter::with_capacity(4 * 1024 * 1024, std::io::stdout());
        let mut dm = Self {
            stdout,
            mode,
            rgb_diff_threshold: constants::RGB_COLOR_DELTA_THRESHOLD,
            last_cells: None,
            render_buffer: Vec::with_capacity(4 * 1024 * 1024), // Pre-allocate 4MB buffer
            perf_window_start: Instant::now(),
            perf_frames: 0,
            perf_slow_frames: 0,
            perf_total_us: 0,
            perf_diff_us: 0,
            perf_io_us: 0,
        };

        dm.initialize_terminal()?;

        Ok(dm)
    }

    fn initialize_terminal(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        self.stdout.execute(EnterAlternateScreen)?;
        self.stdout.execute(cursor::Hide)?;

        // Disable line wrapping (DECRAWM) to prevent scrolling at edges
        self.stdout.execute(Print("\x1b[?7l"))?;

        // === STRONGER V-SYNC ENFORCEMENT ===
        // Enable synchronized updates mode (DECSM 2026)
        // This ensures terminal waits for complete frame before rendering
        self.stdout.execute(Print("\x1b[?2026h"))?;

        // Disable cursor blinking (reduces screen tearing)
        self.stdout.execute(Print("\x1b[?12l"))?;

        // Request high refresh rate mode if supported
        self.stdout.execute(Print("\x1b[?1049h"))?; // Alternative screen buffer

        Ok(())
    }

    /// Return terminal size in character columns and rows.
    pub fn terminal_size_chars(&self) -> Result<(u16, u16)> {
        // crossterm::terminal::size() already returns character-cell units.
        // Avoid heuristic conversion from env vars, which can mis-detect
        // fullscreen terminals as pixel sizes and shrink the render area.
        terminal::size().map_err(Into::into)
    }

    pub fn set_rgb_diff_threshold(&mut self, threshold: u8) {
        self.rgb_diff_threshold = threshold
            .max(constants::RGB_COLOR_DELTA_THRESHOLD)
            .min(constants::RGB_COLOR_DELTA_THRESHOLD_MAX);
    }

    // Helper for zero-allocation integer writing
    #[inline(always)]
    fn write_u8_fast(buffer: &mut Vec<u8>, mut n: u8) {
        if n == 0 {
            buffer.push(b'0');
            return;
        }
        if n >= 100 {
            buffer.push(b'0' + (n / 100));
            n %= 100;
            buffer.push(b'0' + (n / 10));
            n %= 10;
            buffer.push(b'0' + n);
        } else if n >= 10 {
            buffer.push(b'0' + (n / 10));
            n %= 10;
            buffer.push(b'0' + n);
        } else {
            buffer.push(b'0' + n);
        }
    }

    // Helper for zero-allocation u16 writing
    #[inline(always)]
    fn write_u16_fast(buffer: &mut Vec<u8>, mut n: u16) {
        if n >= 10000 {
            buffer.push(b'0' + (n / 10000) as u8);
            n %= 10000;
            buffer.push(b'0' + (n / 1000) as u8);
            n %= 1000;
            buffer.push(b'0' + (n / 100) as u8);
            n %= 100;
            buffer.push(b'0' + (n / 10) as u8);
            n %= 10;
            buffer.push(b'0' + n as u8);
        } else if n >= 1000 {
            buffer.push(b'0' + (n / 1000) as u8);
            n %= 1000;
            buffer.push(b'0' + (n / 100) as u8);
            n %= 100;
            buffer.push(b'0' + (n / 10) as u8);
            n %= 10;
            buffer.push(b'0' + n as u8);
        } else if n >= 100 {
            buffer.push(b'0' + (n / 100) as u8);
            n %= 100;
            buffer.push(b'0' + (n / 10) as u8);
            n %= 10;
            buffer.push(b'0' + n as u8);
        } else if n >= 10 {
            buffer.push(b'0' + (n / 10) as u8);
            n %= 10;
            buffer.push(b'0' + n as u8);
        } else {
            buffer.push(b'0' + n as u8);
        }
    }

    #[inline(always)]
    fn color_changed(a: (u8, u8, u8), b: (u8, u8, u8), threshold: u8) -> bool {
        let th = threshold as i16;
        (a.0 as i16 - b.0 as i16).abs() > th
            || (a.1 as i16 - b.1 as i16).abs() > th
            || (a.2 as i16 - b.2 as i16).abs() > th
    }

    #[inline(always)]
    fn ascii_index(cell: &CellData, x: usize, y: usize) -> usize {
        let top = (cell.fg.0 as u16 * 299 + cell.fg.1 as u16 * 587 + cell.fg.2 as u16 * 114) as f32
            / 1000.0;
        let bottom = (cell.bg.0 as u16 * 299 + cell.bg.1 as u16 * 587 + cell.bg.2 as u16 * 114)
            as f32
            / 1000.0;
        let mut brightness = (top + bottom) * 0.5;

        // Subtle ordered dithering keeps detail in low-contrast areas.
        const BAYER_4X4: [f32; 16] = [
            0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0,
        ];
        let d = BAYER_4X4[(y & 3) * 4 + (x & 3)];
        brightness = (brightness + (d - 7.5) * 1.4).clamp(0.0, 255.0);

        let normalized = (brightness / 255.0).powf(constants::ASCII_GAMMA);
        let gradient_len = constants::ASCII_GRADIENT.len();
        ((normalized * (gradient_len.saturating_sub(1)) as f32).round() as usize)
            .min(gradient_len.saturating_sub(1))
    }

    // Optimized Diffing Renderer with Zero-Allocation
    pub fn render_diff(&mut self, cells: &[CellData], width: usize) -> Result<()> {
        let start_render = std::time::Instant::now();

        // Reuse buffer
        self.render_buffer.clear();
        let buffer = &mut self.render_buffer;

        // VSync Begin (Directly into buffer)
        buffer.extend_from_slice(b"\x1b[?2026h");

        let mut force_redraw = false;
        if self.last_cells.as_ref().map(|v| v.len()).unwrap_or(0) != cells.len() {
            // Clear screen directly into buffer
            buffer.extend_from_slice(b"\x1b[2J");
            self.last_cells = Some(vec![CellData::default(); cells.len()]);
            force_redraw = true;
        }

        let last_cells = match &mut self.last_cells {
            Some(v) => v,
            None => {
                return Ok(());
            }
        };

        let mut last_fg: Option<(u8, u8, u8)> = None;
        let mut last_bg: Option<(u8, u8, u8)> = None;

        // ... (centering logic) ...
        let (term_cols, term_rows) = terminal::size().unwrap_or((80, 24));

        let content_width = width as u16;
        let content_height = (cells.len() / width) as u16;

        let offset_x = if term_cols > content_width {
            (term_cols - content_width) / 2
        } else {
            0
        };
        let offset_y = if term_rows > content_height {
            (term_rows - content_height) / 2
        } else {
            0
        };

        // Track virtual cursor position
        let mut cursor_x: i32 = -1;
        let mut cursor_y: i32 = -1;

        // OPTIMIZATION: Unified loop for both redraw and diff
        for (i, cell) in cells.iter().enumerate() {
            let old_cell = &last_cells[i];
            let x = i % width;
            let y = i / width;
            let mut ascii_idx_new: Option<usize> = None;

            let is_different = if force_redraw {
                true
            } else {
                match self.mode {
                    DisplayMode::Rgb => {
                        cell.char != old_cell.char
                            || Self::color_changed(cell.fg, old_cell.fg, self.rgb_diff_threshold)
                            || Self::color_changed(cell.bg, old_cell.bg, self.rgb_diff_threshold)
                    }
                    DisplayMode::Ascii => {
                        let idx = Self::ascii_index(cell, x, y);
                        ascii_idx_new = Some(idx);
                        idx != Self::ascii_index(old_cell, x, y)
                    }
                }
            };

            if is_different {
                let x = x as u16;
                let y = y as u16;

                let target_x = x + offset_x;
                let target_y = y + offset_y;

                // BOUNDS CHECKING: Skip if outside terminal
                if target_x >= term_cols || target_y >= term_rows {
                    cursor_x = -1;
                    continue;
                }

                // Zero-Allocation Cursor Move
                if cursor_x != target_x as i32 || cursor_y != target_y as i32 {
                    buffer.extend_from_slice(b"\x1b[");
                    Self::write_u16_fast(buffer, target_y + 1);
                    buffer.push(b';');
                    Self::write_u16_fast(buffer, target_x + 1);
                    buffer.push(b'H');

                    cursor_x = target_x as i32;
                    cursor_y = target_y as i32;
                }

                // Render based on mode
                match self.mode {
                    DisplayMode::Rgb => {
                        // Zero-Allocation Color Updates (TrueColor)
                        // FG: \x1b[38;2;R;G;Bm
                        if Some(cell.fg) != last_fg {
                            buffer.extend_from_slice(b"\x1b[38;2;");
                            Self::write_u8_fast(buffer, cell.fg.0);
                            buffer.push(b';');
                            Self::write_u8_fast(buffer, cell.fg.1);
                            buffer.push(b';');
                            Self::write_u8_fast(buffer, cell.fg.2);
                            buffer.push(b'm');
                            last_fg = Some(cell.fg);
                        }
                        // BG: \x1b[48;2;R;G;Bm
                        if Some(cell.bg) != last_bg {
                            buffer.extend_from_slice(b"\x1b[48;2;");
                            Self::write_u8_fast(buffer, cell.bg.0);
                            buffer.push(b';');
                            Self::write_u8_fast(buffer, cell.bg.1);
                            buffer.push(b';');
                            Self::write_u8_fast(buffer, cell.bg.2);
                            buffer.push(b'm');
                            last_bg = Some(cell.bg);
                        }
                    }
                    DisplayMode::Ascii => {
                        let idx = ascii_idx_new
                            .unwrap_or_else(|| Self::ascii_index(cell, x as usize, y as usize));
                        let ascii_char = constants::ASCII_GRADIENT
                            .as_bytes()
                            .get(idx)
                            .copied()
                            .unwrap_or(b' ') as char;

                        // Write the ASCII character directly (no color codes)
                        let mut b_dst = [0u8; 4];
                        buffer.extend_from_slice(ascii_char.encode_utf8(&mut b_dst).as_bytes());

                        last_cells[i] = *cell;
                        cursor_x += 1;

                        // Skip the normal character write below
                        continue;
                    }
                }

                // Write character (RGB mode only, ASCII mode already wrote above)
                let mut b_dst = [0u8; 4];
                buffer.extend_from_slice(cell.char.encode_utf8(&mut b_dst).as_bytes());

                last_cells[i] = *cell;

                // Advance virtual cursor
                cursor_x += 1;
            } else {
                // If cell didn't change, invalidate cursor tracker
                cursor_x = -1;
            }
        }

        buffer.extend_from_slice(b"\x1b[0m");

        // VSync End (Directly into buffer)
        buffer.extend_from_slice(b"\x1b[?2026l");

        let diff_time = start_render.elapsed();

        // I/O Measurement
        let start_io = std::time::Instant::now();
        self.stdout.write_all(buffer)?;
        self.stdout.flush()?;
        let io_time = start_io.elapsed();

        let total_time = start_render.elapsed();
        self.record_render_stats(total_time, diff_time, io_time, cells.len());

        Ok(())
    }

    fn record_render_stats(
        &mut self,
        total_time: Duration,
        diff_time: Duration,
        io_time: Duration,
        cells: usize,
    ) {
        self.perf_frames += 1;
        self.perf_total_us += total_time.as_micros();
        self.perf_diff_us += diff_time.as_micros();
        self.perf_io_us += io_time.as_micros();
        if total_time > Duration::from_millis(16) {
            self.perf_slow_frames += 1;
        }

        if self.perf_frames % constants::PERF_LOG_EVERY_FRAMES != 0 {
            return;
        }

        let elapsed = self.perf_window_start.elapsed().as_secs_f64().max(0.001);
        let fps = self.perf_frames as f64 / elapsed;
        let avg_total_ms = self.perf_total_us as f64 / self.perf_frames as f64 / 1000.0;
        let avg_diff_ms = self.perf_diff_us as f64 / self.perf_frames as f64 / 1000.0;
        let avg_io_ms = self.perf_io_us as f64 / self.perf_frames as f64 / 1000.0;

        crate::utils::logger::debug(&format!(
            "render perf: fps={:.1} avg_total={:.2}ms avg_diff={:.2}ms avg_io={:.2}ms slow={}/{} cells={}",
            fps,
            avg_total_ms,
            avg_diff_ms,
            avg_io_ms,
            self.perf_slow_frames,
            self.perf_frames,
            cells
        ));

        self.perf_window_start = Instant::now();
        self.perf_frames = 0;
        self.perf_slow_frames = 0;
        self.perf_total_us = 0;
        self.perf_diff_us = 0;
        self.perf_io_us = 0;
    }
}

impl Drop for DisplayManager {
    fn drop(&mut self) {
        let _ = self.stdout.execute(cursor::Show);
        let _ = self.stdout.execute(LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
