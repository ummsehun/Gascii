use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

#[cfg(target_os = "macos")]
fn request_macos_enter_fullscreen() {
    // Use menu action "Enter Full Screen" only.
    // This avoids accidentally toggling out of fullscreen.
    let script_lines = [
        "tell application \"System Events\"",
        "set frontApp to name of first application process whose frontmost is true",
        "tell process frontApp",
        "if exists menu bar 1 then",
        "if exists menu \"View\" of menu bar 1 then",
        "tell menu \"View\" of menu bar 1",
        "if exists menu item \"Enter Full Screen\" then",
        "click menu item \"Enter Full Screen\"",
        "else if exists menu item \"전체 화면으로 전환\" then",
        "click menu item \"전체 화면으로 전환\"",
        "end if",
        "end tell",
        "else if exists menu \"보기\" of menu bar 1 then",
        "tell menu \"보기\" of menu bar 1",
        "if exists menu item \"전체 화면으로 전환\" then",
        "click menu item \"전체 화면으로 전환\"",
        "else if exists menu item \"Enter Full Screen\" then",
        "click menu item \"Enter Full Screen\"",
        "end if",
        "end tell",
        "end if",
        "end if",
        "end tell",
        "end tell",
    ];

    let mut cmd = Command::new("osascript");
    for line in script_lines {
        cmd.arg("-e").arg(line);
    }

    match cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => crate::utils::logger::info(&format!(
            "macOS fullscreen request returned non-zero status: {}",
            status
        )),
        Err(e) => crate::utils::logger::info(&format!("macOS fullscreen request failed: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
fn request_macos_enter_fullscreen() {}

pub fn request_fullscreen(os_level: bool) {
    let mut stdout = io::stdout();
    if !stdout.is_terminal() {
        return;
    }

    if os_level {
        request_macos_enter_fullscreen();
    }

    // xterm window op: maximize/fullscreen request.
    // Terminals that do not support this simply ignore it.
    let _ = write!(stdout, "\x1b[9;1t");
    let _ = stdout.flush();
}

pub fn request_resize(cols: u16, rows: u16) {
    let mut stdout = io::stdout();
    if !stdout.is_terminal() {
        return;
    }

    // xterm window op: resize request.
    // Unsupported terminals ignore this sequence.
    let _ = write!(stdout, "\x1b[8;{};{}t", rows, cols);
    let _ = stdout.flush();
}
