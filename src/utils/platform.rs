use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalFamily {
    WindowsTerminal,
    PowerShell,
    Cmd,
    ConEmu,
    Mintty,
    AppleTerminal,
    Ghostty,
    Kitty,
    WezTerm,
    ITerm2,
    Unknown,
}

impl TerminalFamily {
    pub fn label(self) -> &'static str {
        match self {
            Self::WindowsTerminal => "Windows Terminal",
            Self::PowerShell => "PowerShell",
            Self::Cmd => "CMD",
            Self::ConEmu => "ConEmu",
            Self::Mintty => "mintty",
            Self::AppleTerminal => "Apple Terminal",
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

#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalEnvironment<'a> {
    pub os: &'a str,
    pub term_program: Option<&'a str>,
    pub term: Option<&'a str>,
    pub colorterm: Option<&'a str>,
    pub wt_session: Option<&'a str>,
    pub conemu_ansi: Option<&'a str>,
    pub ansicon: Option<&'a str>,
    pub ps_module_path: Option<&'a str>,
    pub comspec: Option<&'a str>,
    pub ghostty_resources_dir: Option<&'a str>,
    pub kitty_window_id: Option<&'a str>,
    pub wezterm_executable: Option<&'a str>,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let term_program = env::var("TERM_PROGRAM").ok();
        let term = env::var("TERM").ok();
        let colorterm = env::var("COLORTERM").ok();
        let wt_session = env::var("WT_SESSION").ok();
        let conemu_ansi = env::var("ConEmuANSI").ok();
        let ansicon = env::var("ANSICON").ok();
        let ps_module_path = env::var("PSModulePath").ok();
        let comspec = env::var("ComSpec").ok();
        let ghostty_resources_dir = env::var("GHOSTTY_RESOURCES_DIR").ok();
        let kitty_window_id = env::var("KITTY_WINDOW_ID").ok();
        let wezterm_executable = env::var("WEZTERM_EXECUTABLE").ok();

        Self::from_environment(TerminalEnvironment {
            os: std::env::consts::OS,
            term_program: term_program.as_deref(),
            term: term.as_deref(),
            colorterm: colorterm.as_deref(),
            wt_session: wt_session.as_deref(),
            conemu_ansi: conemu_ansi.as_deref(),
            ansicon: ansicon.as_deref(),
            ps_module_path: ps_module_path.as_deref(),
            comspec: comspec.as_deref(),
            ghostty_resources_dir: ghostty_resources_dir.as_deref(),
            kitty_window_id: kitty_window_id.as_deref(),
            wezterm_executable: wezterm_executable.as_deref(),
        })
    }

    pub fn for_family(terminal_family: TerminalFamily) -> Self {
        match terminal_family {
            TerminalFamily::WindowsTerminal => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: false,
                supports_kitty_graphics: false,
                supports_iterm2_images: false,
            },
            TerminalFamily::PowerShell => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: false,
                supports_kitty_graphics: false,
                supports_iterm2_images: false,
            },
            TerminalFamily::Cmd => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: false,
                supports_kitty_graphics: false,
                supports_iterm2_images: false,
            },
            TerminalFamily::ConEmu => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: false,
                supports_kitty_graphics: false,
                supports_iterm2_images: false,
            },
            TerminalFamily::Mintty => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: false,
                supports_kitty_graphics: false,
                supports_iterm2_images: false,
            },
            TerminalFamily::AppleTerminal => Self {
                terminal_family,
                supports_ansi: true,
                supports_truecolor: true,
                supports_sync_output: false,
                supports_kitty_graphics: false,
                supports_iterm2_images: false,
            },
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
        Self::from_environment(TerminalEnvironment {
            os: std::env::consts::OS,
            term_program,
            term,
            colorterm,
            ..TerminalEnvironment::default()
        })
    }

    pub fn from_environment(environment: TerminalEnvironment<'_>) -> Self {
        let terminal_family = detect_terminal_family(environment);
        let mut capabilities = Self::for_family(terminal_family);

        capabilities.supports_truecolor = capabilities.supports_truecolor
            || environment
                .colorterm
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
        if let Ok(term) = env::var("WT_SESSION") {
            if !term.is_empty() {
                return "Windows Terminal".to_string();
            }
        }
        if let Ok(term) = env::var("TERM_PROGRAM") {
            return term;
        }
        if let Ok(term) = env::var("TERM") {
            return term;
        }
        "Unknown".to_string()
    }

    fn detect_shell() -> String {
        if let Ok(shell) = env::var("ComSpec") {
            return shell.split('\\').last().unwrap_or("unknown").to_string();
        }
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

fn detect_terminal_family(environment: TerminalEnvironment<'_>) -> TerminalFamily {
    let os = environment.os.to_ascii_lowercase();
    let term_program = environment
        .term_program
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term = environment.term.unwrap_or_default().to_ascii_lowercase();
    let comspec = environment.comspec.unwrap_or_default().to_ascii_lowercase();

    if has_value(environment.wt_session) {
        TerminalFamily::WindowsTerminal
    } else if has_value(environment.ghostty_resources_dir) || term_program.contains("ghostty") {
        TerminalFamily::Ghostty
    } else if has_value(environment.wezterm_executable)
        || term_program.contains("wezterm")
        || term.contains("wezterm")
    {
        TerminalFamily::WezTerm
    } else if term_program.contains("iterm") || term.contains("iterm") {
        TerminalFamily::ITerm2
    } else if has_value(environment.kitty_window_id) || term.contains("kitty") {
        TerminalFamily::Kitty
    } else if term_program.contains("apple_terminal") {
        TerminalFamily::AppleTerminal
    } else if term.contains("mintty") || term_program.contains("mintty") {
        TerminalFamily::Mintty
    } else if has_value(environment.conemu_ansi) {
        TerminalFamily::ConEmu
    } else if os == "windows" && has_value(environment.ansicon) {
        TerminalFamily::Cmd
    } else if os == "windows" && has_value(environment.ps_module_path) {
        TerminalFamily::PowerShell
    } else if os == "windows" && comspec.contains("cmd") {
        TerminalFamily::Cmd
    } else {
        TerminalFamily::Unknown
    }
}

fn has_value(value: Option<&str>) -> bool {
    value.map(|value| !value.is_empty()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ghostty() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term_program: Some("ghostty"),
            term: Some("xterm-ghostty"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::Ghostty);
        assert!(caps.supports_kitty_graphics);
        assert!(caps.supports_sync_output);
    }

    #[test]
    fn detects_kitty() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term: Some("xterm-kitty"),
            colorterm: Some("truecolor"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::Kitty);
        assert!(caps.supports_kitty_graphics);
        assert!(caps.supports_truecolor);
    }

    #[test]
    fn detects_wezterm() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term_program: Some("WezTerm"),
            term: Some("wezterm"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::WezTerm);
        assert!(caps.supports_kitty_graphics);
    }

    #[test]
    fn detects_iterm2() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term_program: Some("iTerm.app"),
            term: Some("xterm-256color"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::ITerm2);
        assert!(caps.supports_iterm2_images);
    }

    #[test]
    fn detects_windows_terminal() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "windows",
            wt_session: Some("session"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::WindowsTerminal);
        assert!(caps.supports_truecolor);
        assert!(!caps.supports_sync_output);
    }

    #[test]
    fn detects_apple_terminal() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term_program: Some("Apple_Terminal"),
            term: Some("xterm-256color"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::AppleTerminal);
        assert!(caps.supports_truecolor);
        assert!(!caps.supports_sync_output);
    }

    #[test]
    fn detects_kitty_from_window_id() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term: Some("xterm-256color"),
            kitty_window_id: Some("1"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::Kitty);
        assert!(caps.supports_truecolor);
    }

    #[test]
    fn colorterm_truecolor_overrides_unknown_family() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term: Some("xterm-256color"),
            colorterm: Some("24bit"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::Unknown);
        assert!(caps.supports_truecolor);
    }

    #[test]
    fn unknown_terminal_is_conservative() {
        let caps = TerminalCapabilities::from_environment(TerminalEnvironment {
            os: "macos",
            term: Some("xterm-256color"),
            ..TerminalEnvironment::default()
        });
        assert_eq!(caps.terminal_family, TerminalFamily::Unknown);
        assert!(!caps.supports_truecolor);
        assert!(!caps.supports_kitty_graphics);
        assert!(!caps.supports_iterm2_images);
    }
}
