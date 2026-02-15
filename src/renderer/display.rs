use anyhow::Result;
use crossterm::{
    cursor,
    style::Print,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{BufWriter, Stdout, Write};

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
    last_cells: Option<Vec<CellData>>,
    render_buffer: Vec<u8>,
}

impl DisplayManager {
    pub fn new(mode: DisplayMode) -> Result<Self> {
        // Use BufWriter
        // Massive output buffer to minimize system call overhead (4MB for smooth 3D rendering)
        let stdout = BufWriter::with_capacity(4 * 1024 * 1024, std::io::stdout());
        let mut dm = Self {
            stdout,
            mode,
            last_cells: None,
            render_buffer: Vec::with_capacity(4 * 1024 * 1024), // Pre-allocate 4MB buffer
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

    /// Return terminal size in character columns and rows, converting from pixels when needed.
    pub fn terminal_size_chars(&self) -> Result<(u16, u16)> {
        let (mut term_cols, mut term_rows) = terminal::size()?;
        if let (Ok(cw_str), Ok(ch_str)) =
            (std::env::var("CHAR_WIDTH"), std::env::var("CHAR_HEIGHT"))
        {
            if let (Ok(cw), Ok(ch)) = (cw_str.parse::<u16>(), ch_str.parse::<u16>()) {
                if term_cols > cw * 16 {
                    term_cols = (term_cols / cw).max(1);
                }
                if term_rows > ch * 8 {
                    term_rows = (term_rows / ch).max(1);
                }
            }
        }
        Ok((term_cols, term_rows))
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
        let (mut term_cols, mut term_rows) = terminal::size().unwrap_or((80, 24));

        // If environment provides CHAR_WIDTH/CHAR_HEIGHT (pixel size per char), convert if
        // terminal::size returned pixel dimensions rather than char counts.
        if let (Ok(cw_str), Ok(ch_str)) =
            (std::env::var("CHAR_WIDTH"), std::env::var("CHAR_HEIGHT"))
        {
            if let (Ok(cw), Ok(ch)) = (cw_str.parse::<u16>(), ch_str.parse::<u16>()) {
                // If the terminal reports a very large value for term_cols/term_rows, assume it's pixels
                if term_cols > cw * 16 {
                    // threshold: more than ~16 columns per default
                    term_cols = (term_cols / cw).max(1);
                }
                if term_rows > ch * 8 {
                    term_rows = (term_rows / ch).max(1);
                }
            }
        }
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

        // Debug logging
        if std::env::var("BAD_APPLE_DEBUG").is_ok() {
            use std::fs::OpenOptions;
            use std::io::Write;
            let mut log_path = std::env::current_dir().unwrap_or_default();
            log_path.push(constants::DEBUG_LOG_FILE);
            if let Ok(mut file) = OpenOptions::new().append(true).open(log_path) {
                let _ = writeln!(
                    file,
                    "RENDER DEBUG: term={}x{} (after conversion) content={}x{} offset={}x{}",
                    term_cols, term_rows, content_width, content_height, offset_x, offset_y
                );
            }
        }

        // OPTIMIZATION: Unified loop for both redraw and diff
        for (i, cell) in cells.iter().enumerate() {
            let old_cell = &last_cells[i];

            let is_different = if force_redraw {
                true
            } else if cell.char != old_cell.char {
                true
            } else {
                cell.fg != old_cell.fg || cell.bg != old_cell.bg
            };

            if is_different {
                let x = (i % width) as u16;
                let y = (i / width) as u16;

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
                        // ASCII mode: No colors, convert to grayscale ASCII art
                        // Convert RGB to grayscale brightness: 0.299*R + 0.587*G + 0.114*B
                        // We use the foreground color for brightness calculation
                        let brightness = (cell.fg.0 as u32 * 299
                            + cell.fg.1 as u32 * 587
                            + cell.fg.2 as u32 * 114)
                            / 1000;

                        // ASCII character set from darkest to brightest
                        const ASCII_CHARS: &[char] =
                            &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

                        // Map brightness (0-255) to character index (0-9)
                        let char_idx =
                            ((brightness * (ASCII_CHARS.len() as u32 - 1)) / 255) as usize;
                        let ascii_char = ASCII_CHARS[char_idx];

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
        if total_time.as_millis() > 10 {
            use std::fs::OpenOptions;
            use std::io::Write;
            let mut log_path = std::env::current_dir().unwrap_or_default();
            log_path.push(constants::DEBUG_LOG_FILE);

            if let Ok(mut file) = OpenOptions::new().append(true).open(log_path) {
                let _ = writeln!(
                    file,
                    "FAST RENDER: Total={}us | Diff={}us | IO={}us | Cells: {}",
                    total_time.as_micros(),
                    diff_time.as_micros(),
                    io_time.as_micros(),
                    cells.len()
                );
            }
        }

        Ok(())
    }
}

impl Drop for DisplayManager {
    fn drop(&mut self) {
        let _ = self.stdout.execute(cursor::Show);
        let _ = self.stdout.execute(LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
