use anyhow::{anyhow, Result};

use crate::utils::platform::{TerminalCapabilities, TerminalFamily};

use super::display::DisplayMode;

#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum RenderBackend {
    Auto,
    Ansi,
    Kitty,
    ITerm2,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ActiveRenderBackend {
    AnsiAscii,
    AnsiRgb,
    KittyGraphics,
    ITerm2Image,
}

impl ActiveRenderBackend {
    pub fn requires_cell_buffer(self) -> bool {
        matches!(self, Self::AnsiRgb)
    }
}

pub fn select_render_backend(
    mode: DisplayMode,
    requested: RenderBackend,
    capabilities: &TerminalCapabilities,
) -> Result<ActiveRenderBackend> {
    match mode {
        DisplayMode::Ascii => {
            if matches!(requested, RenderBackend::Kitty | RenderBackend::ITerm2) {
                return Err(anyhow!(
                    "ASCII 모드는 ANSI 전용입니다. protocol renderer를 강제할 수 없습니다."
                ));
            }
            Ok(ActiveRenderBackend::AnsiAscii)
        }
        DisplayMode::Rgb => match requested {
            RenderBackend::Auto => Ok(select_rgb_backend(capabilities)),
            RenderBackend::Ansi => Ok(ActiveRenderBackend::AnsiRgb),
            RenderBackend::Kitty => {
                if capabilities.supports_kitty_graphics {
                    Ok(ActiveRenderBackend::KittyGraphics)
                } else {
                    Err(anyhow!(
                        "{} 터미널에서는 kitty graphics backend를 사용할 수 없습니다.",
                        capabilities.terminal_family.label()
                    ))
                }
            }
            RenderBackend::ITerm2 => {
                if capabilities.supports_iterm2_images {
                    Ok(ActiveRenderBackend::ITerm2Image)
                } else {
                    Err(anyhow!(
                        "{} 터미널에서는 iTerm2 image backend를 사용할 수 없습니다.",
                        capabilities.terminal_family.label()
                    ))
                }
            }
        },
    }
}

fn select_rgb_backend(capabilities: &TerminalCapabilities) -> ActiveRenderBackend {
    match capabilities.terminal_family {
        TerminalFamily::Ghostty | TerminalFamily::Kitty | TerminalFamily::WezTerm
            if capabilities.supports_kitty_graphics =>
        {
            ActiveRenderBackend::KittyGraphics
        }
        TerminalFamily::ITerm2 if capabilities.supports_iterm2_images => {
            ActiveRenderBackend::ITerm2Image
        }
        _ => ActiveRenderBackend::AnsiRgb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::platform::TerminalCapabilities;

    fn caps(family: TerminalFamily) -> TerminalCapabilities {
        TerminalCapabilities::for_family(family)
    }

    #[test]
    fn ascii_auto_is_always_ansi() {
        let selected =
            select_render_backend(DisplayMode::Ascii, RenderBackend::Auto, &caps(TerminalFamily::Ghostty))
                .unwrap();
        assert_eq!(selected, ActiveRenderBackend::AnsiAscii);
    }

    #[test]
    fn rgb_auto_prefers_kitty_for_ghostty() {
        let selected =
            select_render_backend(DisplayMode::Rgb, RenderBackend::Auto, &caps(TerminalFamily::Ghostty))
                .unwrap();
        assert_eq!(selected, ActiveRenderBackend::KittyGraphics);
    }

    #[test]
    fn rgb_auto_prefers_iterm_for_iterm2() {
        let selected =
            select_render_backend(DisplayMode::Rgb, RenderBackend::Auto, &caps(TerminalFamily::ITerm2))
                .unwrap();
        assert_eq!(selected, ActiveRenderBackend::ITerm2Image);
    }

    #[test]
    fn rgb_auto_falls_back_to_ansi_for_unknown() {
        let selected =
            select_render_backend(DisplayMode::Rgb, RenderBackend::Auto, &caps(TerminalFamily::Unknown))
                .unwrap();
        assert_eq!(selected, ActiveRenderBackend::AnsiRgb);
    }
}
