use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::env;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub terminal: String,
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
    pub supports_kitty: bool,
    pub supports_sixel: bool,
    pub cpu_cores: usize,
    pub memory_mb: u64,
}

impl PlatformInfo {
    pub fn detect() -> Result<Self> {
        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        
        // Detect actual screen resolution
        let (screen_w, screen_h) = Self::detect_screen_resolution();
        
        // Use simple defaults for character size (most terminals use roughly 1:2 ratio)
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
            supports_ansi: true, // Most modern terminals support ANSI
            supports_truecolor: Self::detect_truecolor(),
            supports_kitty: Self::detect_kitty(),
            supports_sixel: Self::detect_sixel(),
            cpu_cores: num_cpus::get(),
            memory_mb: Self::detect_memory(),
        })
    }

    fn detect_screen_resolution() -> (u32, u32) {
        // macOS: use system_profiler
        if cfg!(target_os = "macos") {
            if let Ok(output) = Command::new("system_profiler")
                .args(&["SPDisplaysDataType"])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                
                // Look for "Resolution:" line
                for line in output_str.lines() {
                    if line.contains("Resolution:") {
                        eprintln!("DEBUG: Found resolution line: '{}'", line);
                        
                        // Extract everything after "Resolution:"
                        if let Some(after_colon) = line.split("Resolution:").nth(1) {
                            let resolution_str = after_colon.trim();
                            eprintln!("DEBUG: Resolution str: '{}'", resolution_str);
                            
                            // Split by 'x' -> ["2880 ", " 1864 Retina"]
                            let parts: Vec<&str> = resolution_str.split('x').map(|s| s.trim()).collect();
                            eprintln!("DEBUG: Parts: {:?}", parts);
                            
                            if parts.len() >= 2 {
                                let width_str = parts[0];
                                // Take only the number part from "1864 Retina" -> "1864"
                                let height_str = parts[1].split_whitespace().next().unwrap_or("");
                                
                                eprintln!("DEBUG: Parsing - Width: '{}', Height: '{}'", width_str, height_str);
                                
                                if let (Ok(w), Ok(h)) = (width_str.parse::<u32>(), height_str.parse::<u32>()) {
                                    eprintln!("DEBUG: Successfully parsed: {}x{}", w, h);
                                    return (w, h);
                                } else {
                                    eprintln!("DEBUG: Parse failed");
                                }
                            }
                        }
                    }
                }
                eprintln!("DEBUG: No resolution found, using fallback");
            } else {
                eprintln!("DEBUG: system_profiler command failed");
            }
        }
        
        // Fallback: assume 1920x1080
        (1920, 1080)
    }

    #[allow(dead_code)]
    fn detect_pixel_size(cols: u16, rows: u16) -> (u32, u32, u32, u32) {
        use std::io::{Write, Read};
        use std::time::Duration;
        
        // Try CSI 14t (terminal window size in pixels)
        // This might not work on all terminals
        if let Ok(mut term) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
        {
            // Query terminal window size in pixels: CSI 14 t
            let _ = term.write_all(b"\x1b[14t");
            let _ = term.flush();
            
            // Give terminal time to respond
            std::thread::sleep(Duration::from_millis(50));
            
            let mut response = vec![0u8; 64];
            if let Ok(n) = term.read(&mut response) {
                let resp_str = String::from_utf8_lossy(&response[..n]);
                // Response format: ESC [ 4 ; height ; width t
                if let Some(captures) = resp_str.strip_prefix("\x1b[4;") {
                    let parts: Vec<&str> = captures.trim_end_matches('t').split(';').collect();
                    if parts.len() == 2 {
                        if let (Ok(h), Ok(w)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                            let char_w = w / cols as u32;
                            let char_h = h / rows as u32;
                            return (w, h, char_w.max(1), char_h.max(1));
                        }
                    }
                }
            }
        }
        
        // Fallback: assume standard terminal character ratio (1:2)
        // Most terminal fonts have characters that are roughly half as wide as they are tall
        let char_w = 10;
        let char_h = 20;
        let pixel_w = cols as u32 * char_w;
        let pixel_h = rows as u32 * char_h;
        
        (pixel_w, pixel_h, char_w, char_h)
    }

    fn detect_os_version() -> String {
        // Simple detection using uname -r
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

    fn detect_truecolor() -> bool {
        env::var("COLORTERM").map(|v| v.contains("truecolor") || v.contains("24bit")).unwrap_or(false)
    }

    fn detect_kitty() -> bool {
        env::var("TERM").map(|v| v.contains("kitty")).unwrap_or(false)
    }

    fn detect_sixel() -> bool {
        // Basic check for iTerm2 or known sixel terminals
        env::var("TERM_PROGRAM").map(|v| v.contains("iTerm")).unwrap_or(false)
    }

    fn detect_memory() -> u64 {
        // macOS specific
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
