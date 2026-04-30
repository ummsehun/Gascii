use super::display::DisplayMode;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ActiveRenderBackend {
    AnsiAscii,
    AnsiRgb,
}

impl ActiveRenderBackend {
    pub fn for_mode(mode: DisplayMode) -> Self {
        match mode {
            DisplayMode::Ascii => Self::AnsiAscii,
            DisplayMode::Rgb => Self::AnsiRgb,
        }
    }

    pub fn requires_cell_buffer(self) -> bool {
        matches!(self, Self::AnsiRgb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_ansi_ascii() {
        assert_eq!(
            ActiveRenderBackend::for_mode(DisplayMode::Ascii),
            ActiveRenderBackend::AnsiAscii
        );
    }

    #[test]
    fn rgb_is_ansi_rgb() {
        assert_eq!(
            ActiveRenderBackend::for_mode(DisplayMode::Rgb),
            ActiveRenderBackend::AnsiRgb
        );
    }
}
