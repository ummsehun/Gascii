use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalFamily {
    Ghostty,
    Kitty,
    WezTerm,
    ITerm2,
    Unknown,
}

impl TerminalFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ghostty => "Ghostty",
            Self::Kitty => "kitty",
            Self::WezTerm => "WezTerm",
            Self::ITerm2 => "iTerm2",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCapabilities {
    pub terminal_family: TerminalFamily,
    pub supports_ansi: bool,
    pub supports_truecolor: bool,
    pub supports_sync_output: bool,
    pub supports_kitty_graphics: bool,
    pub supports_iterm2_images: bool,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let term_program = env::var("TERM_PROGRAM").ok();
        let term = env::var("TERM").ok();
        let colorterm = env::var("COLORTERM").ok();
        Self::from_env(term_program.as_deref(), term.as_deref(), colorterm.as_deref())
    }

    pub fn for_family(terminal_family: TerminalFamily) -> Self {
        match terminal_family {
            TerminalFamily::Ghostty => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: true,
                supports_kitty_graphics: true,
                supports_iterm2_images: false,
            },
            TerminalFamily::Kitty => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: true,
                supports_kitty_graphics: true,
                supports_iterm2_images: false,
            },
            TerminalFamily::WezTerm => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: true,
                supports_kitty_graphics: true,
                supports_iterm2_images: false,
            },
            TerminalFamily::ITerm2 => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: true,
                supports_kitty_graphics: false,
                supports_iterm2_images: true,
            },
            TerminalFamily::Unknown => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: false,
                supports_sync_output: false,
                supports_kitty_graphics: false,
                supports_iterm2_images: false,
            },
        }
    }

    pub fn from_env(
        term_program: Option<&str>,
        term: Option<&str>,
        colorterm: Option<&str>,
    ) -> Self {
        let terminal_family = detect_terminal_family(term_program, term);
        let mut capabilities = Self::for_family(terminal_family);

        capabilities.supports_truecolor = capabilities.supports_truecolor
            || colorterm
                .map(|value| {
                    let lower = value.to_ascii_lowercase();
                    lower.contains("truecolor") || lower.contains("24bit")
                })
                .unwrap_or(false);

        capabilities
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub terminal: String,
    pub terminal_family: TerminalFamily,
    pub shell: String,
    pub terminal_width: u16,
    pub terminal_height: u16,
    pub terminal_pixel_width: u32,
    pub terminal_pixel_height: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub char_width: u32,
    pub char_height: u32,
    pub char_aspect_ratio: f32,
    pub supports_ansi: bool,
    pub supports_truecolor: bool,
    pub supports_sync_output: bool,
    pub supports_kitty_graphics: bool,
    pub supports_iterm2_images: bool,
    pub cpu_cores: usize,
    pub memory_mb: u64,
}

impl PlatformInfo {
    pub fn detect() -> Result<Self> {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        let capabilities = TerminalCapabilities::detect();
        let (screen_w, screen_h) = Self::detect_screen_resolution();

        let char_w = 10;
        let char_h = 20;
        let pixel_width = width as u32 * char_w;
        let pixel_height = height as u32 * char_h;
        let aspect = char_w as f32 / char_h as f32;

        Ok(Self {
            os_name: std::env::consts::OS.to_string(),
            os_version: Self::detect_os_version(),
            arch: std::env::consts::ARCH.to_string(),
            terminal: Self::detect_terminal(),
            terminal_family: capabilities.terminal_family,
            shell: Self::detect_shell(),
            terminal_width: width,
            terminal_height: height,
            terminal_pixel_width: pixel_width,
            terminal_pixel_height: pixel_height,
            screen_width: screen_w,
            screen_height: screen_h,
            char_width: char_w,
            char_height: char_h,
            char_aspect_ratio: aspect,
            supports_ansi: capabilities.supports_ansi,
            supports_truecolor: capabilities.supports_truecolor,
            supports_sync_output: capabilities.supports_sync_output,
            supports_kitty_graphics: capabilities.supports_kitty_graphics,
            supports_iterm2_images: capabilities.supports_iterm2_images,
            cpu_cores: num_cpus::get(),
            memory_mb: Self::detect_memory(),
        })
    }

    fn detect_screen_resolution() -> (u32, u32) {
        if cfg!(target_os = "macos") {
            if let Ok(output) = Command::new("system_profiler")
                .args(["SPDisplaysDataType"])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    if let Some(after_colon) = line.split("Resolution:").nth(1) {
                        let parts: Vec<&str> =
                            after_colon.trim().split('x').map(str::trim).collect();
                        if parts.len() >= 2 {
                            let width_str = parts[0];
                            let height_str = parts[1].split_whitespace().next().unwrap_or("");
                            if let (Ok(w), Ok(h)) =
                                (width_str.parse::<u32>(), height_str.parse::<u32>())
                            {
                                return (w, h);
                            }
                        }
                    }
                }
            }
        }

        (1920, 1080)
    }

    fn detect_os_version() -> String {
        if let Ok(output) = Command::new("uname").arg("-r").output() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            "Unknown".to_string()
        }
    }

    fn detect_terminal() -> String {
        if let Ok(term) = env::var("TERM_PROGRAM") {
            return term;
        }
        if let Ok(term) = env::var("TERM") {
            return term;
        }
        "Unknown".to_string()
    }

    fn detect_shell() -> String {
        if let Ok(shell) = env::var("SHELL") {
            shell.split('/').last().unwrap_or("unknown").to_string()
        } else {
            "unknown".to_string()
        }
    }

    fn detect_memory() -> u64 {
        if cfg!(target_os = "macos") {
            if let Ok(output) = Command::new("sysctl").arg("-n").arg("hw.memsize").output() {
                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(bytes) = s.parse::<u64>() {
                    return bytes / (1024 * 1024);
                }
            }
        }
        0
    }
}

fn detect_terminal_family(term_program: Option<&str>, term: Option<&str>) -> TerminalFamily {
    let term_program = term_program
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term = term.unwrap_or_default().to_ascii_lowercase();

    if term_program.contains("ghostty") {
        TerminalFamily::Ghostty
    } else if term_program.contains("wezterm") || term.contains("wezterm") {
        TerminalFamily::WezTerm
    } else if term_program.contains("iterm") || term.contains("iterm") {
        TerminalFamily::ITerm2
    } else if term.contains("kitty") {
        TerminalFamily::Kitty
    } else {
        TerminalFamily::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ghostty() {
        let caps = TerminalCapabilities::from_env(Some("ghostty"), Some("xterm-ghostty"), None);
        assert_eq!(caps.terminal_family, TerminalFamily::Ghostty);
        assert!(caps.supports_kitty_graphics);
        assert!(caps.supports_sync_output);
    }

    #[test]
    fn detects_kitty() {
        let caps = TerminalCapabilities::from_env(None, Some("xterm-kitty"), Some("truecolor"));
        assert_eq!(caps.terminal_family, TerminalFamily::Kitty);
        assert!(caps.supports_kitty_graphics);
        assert!(caps.supports_truecolor);
    }

    #[test]
    fn detects_wezterm() {
        let caps = TerminalCapabilities::from_env(Some("WezTerm"), Some("wezterm"), None);
        assert_eq!(caps.terminal_family, TerminalFamily::WezTerm);
        assert!(caps.supports_kitty_graphics);
    }

    #[test]
    fn detects_iterm2() {
        let caps = TerminalCapabilities::from_env(Some("iTerm.app"), Some("xterm-256color"), None);
        assert_eq!(caps.terminal_family, TerminalFamily::ITerm2);
        assert!(caps.supports_iterm2_images);
    }

    #[test]
    fn unknown_terminal_is_conservative() {
        let caps = TerminalCapabilities::from_env(None, Some("xterm-256color"), None);
        assert_eq!(caps.terminal_family, TerminalFamily::Unknown);
        assert!(!caps.supports_kitty_graphics);
        assert!(!caps.supports_iterm2_images);
    }
}
