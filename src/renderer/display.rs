use anyhow::Result;
use crossterm::{
    cursor,
    style::Print,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{BufWriter, Stdout, Write};

use super::backend::ActiveRenderBackend;
use super::cell::CellData;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum DisplayMode {
    Ascii,
    Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderViewport {
    pub offset_x: u16,
    pub offset_y: u16,
    pub terminal_cols: u16,
    pub terminal_rows: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl RenderViewport {
}

pub struct DisplayManager {
    stdout: BufWriter<Stdout>,
    active_backend: ActiveRenderBackend,
    last_cells: Option<Vec<CellData>>,
    last_ascii: Option<Vec<char>>,
    render_buffer: Vec<u8>,
    clear_next_frame: bool,
}

fn normalize_terminal_size(mut term_cols: u16, mut term_rows: u16) -> (u16, u16) {
    if let (Ok(cw_str), Ok(ch_str)) = (std::env::var("CHAR_WIDTH"), std::env::var("CHAR_HEIGHT")) {
        if let (Ok(cw), Ok(ch)) = (cw_str.parse::<u16>(), ch_str.parse::<u16>()) {
            if term_cols > cw * 16 {
                term_cols = (term_cols / cw).max(1);
            }
            if term_rows > ch * 8 {
                term_rows = (term_rows / ch).max(1);
            }
        }
    }

    (term_cols.max(1), term_rows.max(1))
}

fn ascii_char_for_brightness(brightness: u32) -> char {
    const ASCII_CHARS: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    let char_idx = ((brightness.min(255) * (ASCII_CHARS.len() as u32 - 1)) / 255) as usize;
    ASCII_CHARS[char_idx]
}

#[cfg(test)]
fn ascii_char_for(cell: &CellData) -> char {
    let top = (cell.fg.0 as u32 * 299 + cell.fg.1 as u32 * 587 + cell.fg.2 as u32 * 114) / 1000;
    let bottom =
        (cell.bg.0 as u32 * 299 + cell.bg.1 as u32 * 587 + cell.bg.2 as u32 * 114) / 1000;
    ascii_char_for_brightness((top + bottom) / 2)
}

impl DisplayManager {
    pub fn new(
        _mode: DisplayMode,
        active_backend: ActiveRenderBackend,
    ) -> Result<Self> {
        let stdout = BufWriter::with_capacity(4 * 1024 * 1024, std::io::stdout());
        let mut dm = Self {
            stdout,
            active_backend,
            last_cells: None,
            last_ascii: None,
            render_buffer: Vec::with_capacity(4 * 1024 * 1024),
            clear_next_frame: true,
        };

        dm.initialize_terminal()?;
        Ok(dm)
    }

    fn initialize_terminal(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        self.stdout.execute(EnterAlternateScreen)?;
        self.stdout.execute(cursor::Hide)?;
        self.stdout.execute(Print("\x1b[?7l"))?;
        self.stdout.execute(Print("\x1b[?2026h"))?;

        self.stdout.execute(Print("\x1b[?12l"))?;
        self.stdout.flush()?;
        Ok(())
    }

    pub fn current_terminal_size_chars() -> Result<(u16, u16)> {
        let (term_cols, term_rows) = terminal::size()?;
        Ok(normalize_terminal_size(term_cols, term_rows))
    }

    pub fn invalidate_cache(&mut self) {
        self.last_cells = None;
        self.last_ascii = None;
        self.clear_next_frame = true;
    }

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

    pub fn render(
        &mut self,
        rgb_buffer: &[u8],
        rgb_cells: Option<&[CellData]>,
        viewport: RenderViewport,
    ) -> Result<()> {
        match self.active_backend {
            ActiveRenderBackend::AnsiAscii => self.render_ascii(rgb_buffer, viewport),
            ActiveRenderBackend::AnsiRgb => self.render_rgb_diff(rgb_cells.unwrap_or(&[]), viewport),
        }
    }

    fn render_ascii(&mut self, rgb_buffer: &[u8], viewport: RenderViewport) -> Result<()> {
        let width = viewport.pixel_width as usize;
        let height = viewport.pixel_height as usize;
        let cell_count = width * (height / 2);

        if rgb_buffer.len() < width * height * 3 {
            return Ok(());
        }

        self.render_buffer.clear();
        let buffer = &mut self.render_buffer;

        buffer.extend_from_slice(b"\x1b[?2026h");

        let last_ascii = self.last_ascii.get_or_insert_with(|| vec!['\0'; cell_count]);
        let mut force_redraw = false;
        if last_ascii.len() != cell_count {
            *last_ascii = vec!['\0'; cell_count];
            force_redraw = true;
        }
        if self.clear_next_frame {
            buffer.extend_from_slice(b"\x1b[2J");
            force_redraw = true;
            self.clear_next_frame = false;
        }

        let (term_cols, term_rows) =
            normalize_terminal_size(viewport.terminal_cols, viewport.terminal_rows);
        let mut cursor_x: i32 = -1;
        let mut cursor_y: i32 = -1;

        for cell_index in 0..cell_count {
            let cx = cell_index % width;
            let cy = cell_index / width;
            let top_offset = (cy * 2 * width + cx) * 3;
            let bottom_offset = ((cy * 2 + 1) * width + cx) * 3;

            let top = (rgb_buffer[top_offset] as u32 * 299
                + rgb_buffer[top_offset + 1] as u32 * 587
                + rgb_buffer[top_offset + 2] as u32 * 114)
                / 1000;
            let bottom = (rgb_buffer[bottom_offset] as u32 * 299
                + rgb_buffer[bottom_offset + 1] as u32 * 587
                + rgb_buffer[bottom_offset + 2] as u32 * 114)
                / 1000;
            let ascii_char = ascii_char_for_brightness((top + bottom) / 2);

            if force_redraw || last_ascii[cell_index] != ascii_char {
                let target_x = viewport.offset_x + cx as u16;
                let target_y = viewport.offset_y + cy as u16;

                if target_x >= term_cols || target_y >= term_rows {
                    cursor_x = -1;
                    continue;
                }

                if cursor_x != target_x as i32 || cursor_y != target_y as i32 {
                    buffer.extend_from_slice(b"\x1b[");
                    Self::write_u16_fast(buffer, target_y + 1);
                    buffer.push(b';');
                    Self::write_u16_fast(buffer, target_x + 1);
                    buffer.push(b'H');
                    cursor_x = target_x as i32;
                    cursor_y = target_y as i32;
                }

                let mut bytes = [0u8; 4];
                buffer.extend_from_slice(ascii_char.encode_utf8(&mut bytes).as_bytes());
                last_ascii[cell_index] = ascii_char;
                cursor_x += 1;
            } else {
                cursor_x = -1;
            }
        }

        buffer.extend_from_slice(b"\x1b[0m");
        buffer.extend_from_slice(b"\x1b[?2026l");
        self.stdout.write_all(buffer)?;
        self.stdout.flush()?;
        Ok(())
    }

    fn render_rgb_diff(&mut self, cells: &[CellData], viewport: RenderViewport) -> Result<()> {
        let width = viewport.pixel_width as usize;

        self.render_buffer.clear();
        let buffer = &mut self.render_buffer;

        buffer.extend_from_slice(b"\x1b[?2026h");

        let mut force_redraw = false;
        if self.last_cells.as_ref().map(|v| v.len()).unwrap_or(0) != cells.len() {
            self.last_cells = Some(vec![CellData::default(); cells.len()]);
            force_redraw = true;
        }
        if self.clear_next_frame {
            buffer.extend_from_slice(b"\x1b[2J");
            force_redraw = true;
            self.clear_next_frame = false;
        }

        let last_cells = match &mut self.last_cells {
            Some(v) => v,
            None => return Ok(()),
        };

        let (term_cols, term_rows) =
            normalize_terminal_size(viewport.terminal_cols, viewport.terminal_rows);
        let mut last_fg: Option<(u8, u8, u8)> = None;
        let mut last_bg: Option<(u8, u8, u8)> = None;
        let mut cursor_x: i32 = -1;
        let mut cursor_y: i32 = -1;

        for (i, cell) in cells.iter().enumerate() {
            let old_cell = &last_cells[i];
            let is_different = if force_redraw {
                true
            } else if cell.char != old_cell.char {
                true
            } else {
                cell.fg != old_cell.fg || cell.bg != old_cell.bg
            };

            if !is_different {
                cursor_x = -1;
                continue;
            }

            let x = (i % width) as u16;
            let y = (i / width) as u16;
            let target_x = x + viewport.offset_x;
            let target_y = y as u16 + viewport.offset_y;

            if target_x >= term_cols || target_y >= term_rows {
                cursor_x = -1;
                continue;
            }

            if cursor_x != target_x as i32 || cursor_y != target_y as i32 {
                buffer.extend_from_slice(b"\x1b[");
                Self::write_u16_fast(buffer, target_y + 1);
                buffer.push(b';');
                Self::write_u16_fast(buffer, target_x + 1);
                buffer.push(b'H');
                cursor_x = target_x as i32;
                cursor_y = target_y as i32;
            }

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

            let mut bytes = [0u8; 4];
            buffer.extend_from_slice(cell.char.encode_utf8(&mut bytes).as_bytes());
            last_cells[i] = *cell;
            cursor_x += 1;
        }

        buffer.extend_from_slice(b"\x1b[0m");
        buffer.extend_from_slice(b"\x1b[?2026l");
        self.stdout.write_all(buffer)?;
        self.stdout.flush()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_brightness_mapping_is_stable() {
        assert_eq!(ascii_char_for_brightness(0), ' ');
        assert_eq!(ascii_char_for_brightness(255), '@');
    }

    #[test]
    fn ascii_cell_mapping_uses_average_brightness() {
        let cell = CellData {
            char: '▀',
            fg: (255, 255, 255),
            bg: (255, 255, 255),
        };
        assert_eq!(ascii_char_for(&cell), '@');
    }
}
