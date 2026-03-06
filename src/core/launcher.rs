use anyhow::Result;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::renderer::DisplayMode;
use crate::shared::constants;
use crate::ui::menu::MenuSelection;

pub fn run() -> Result<()> {
    let Some(selection) = crate::ui::menu::run_menu()? else {
        println!("Menu cancelled.");
        return Ok(());
    };

    let mode = match selection.mode {
        DisplayMode::Rgb => "rgb",
        DisplayMode::Ascii => "ascii",
    };

    crate::utils::logger::info(&format!(
        "launch selection: video={} mode={} fill={} font_size={} audio={}",
        selection.video_path.display(),
        mode,
        selection.fill_screen,
        selection.font_size,
        selection
            .audio_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    ));

    if try_launch_ghostty(&selection)? {
        return Ok(());
    }

    eprintln!("⚠️ Ghostty 실행에 실패하여 현재 터미널에서 재생합니다.");
    crate::utils::logger::info(
        "ghostty not found or launch failed; using current terminal fallback",
    );

    // Always request true fullscreen window for menu-launched playback.
    // "fill_screen" controls content layout only (cover vs 16:9 fit).
    crate::utils::terminal_control::request_fullscreen(true);
    std::thread::sleep(Duration::from_millis(150));

    crate::ui::interactive::run_game(
        selection.video_path,
        selection.audio_path,
        selection.mode,
        selection.fill_screen,
    )?;

    Ok(())
}

fn try_launch_ghostty(selection: &MenuSelection) -> Result<bool> {
    let runtime_args = build_ghostty_runtime_args(selection)?;
    let args_for_log = runtime_args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    crate::utils::logger::debug(&format!("ghostty runtime args: {}", args_for_log));

    #[cfg(target_os = "macos")]
    {
        if let Some(bundle_path) = resolve_ghostty_bundle_path() {
            if try_open_ghostty_bundle(&bundle_path, &runtime_args)? {
                return Ok(true);
            }
        }
    }

    let Some(ghostty_bin) = resolve_ghostty_binary() else {
        return Ok(false);
    };

    try_spawn_ghostty_binary(&ghostty_bin, &runtime_args)
}

fn build_ghostty_runtime_args(selection: &MenuSelection) -> Result<Vec<OsString>> {
    let current_exe = std::env::current_exe()?;
    let config_path = absolute_path(Path::new(constants::GASCCI_CONFIG_FILE));
    let video_path = absolute_path(&selection.video_path);

    let mode = match selection.mode {
        DisplayMode::Rgb => "rgb",
        DisplayMode::Ascii => "ascii",
    };

    let mut args = vec![OsString::from(format!(
        "--config-file={}",
        config_path.display()
    ))];
    args.push(constants::GHOSTTY_WINDOW_INHERIT_FONT_SIZE_ARG.into());
    args.push(OsString::from(format!(
        "--font-size={}",
        selection.font_size
    )));

    // Always launch Ghostty as fullscreen window.
    args.push(constants::GHOSTTY_FULLSCREEN_ARG.into());

    args.push(OsString::from("-e"));
    args.push(current_exe.into_os_string());
    args.push(constants::PLAY_SUBCOMMAND.into());
    args.push("--video".into());
    args.push(video_path.into_os_string());
    args.push("--mode".into());
    args.push(mode.into());

    if let Some(audio_path) = &selection.audio_path {
        args.push("--audio".into());
        args.push(absolute_path(audio_path).into_os_string());
    }

    if selection.fill_screen {
        args.push("--fill".into());
    }

    Ok(args)
}

#[cfg(target_os = "macos")]
fn try_open_ghostty_bundle(bundle_path: &Path, runtime_args: &[OsString]) -> Result<bool> {
    let mut cmd = Command::new("open");
    cmd.arg("-na").arg(bundle_path).arg("--args");
    cmd.args(runtime_args);
    if let Ok(project_root) = std::env::current_dir() {
        cmd.env("GASCII_PROJECT_ROOT", project_root);
    }

    match cmd.status() {
        Ok(status) if status.success() => {
            crate::utils::logger::info(&format!(
                "launched ghostty via open: {}",
                bundle_path.display()
            ));
            Ok(true)
        }
        Ok(status) => {
            crate::utils::logger::error(&format!(
                "failed to launch ghostty via open (status: {})",
                status
            ));
            Ok(false)
        }
        Err(e) => {
            crate::utils::logger::error(&format!("failed to run open for ghostty: {}", e));
            Ok(false)
        }
    }
}

fn try_spawn_ghostty_binary(ghostty_bin: &Path, runtime_args: &[OsString]) -> Result<bool> {
    let mut cmd = Command::new(ghostty_bin);
    cmd.args(runtime_args);
    if let Ok(project_root) = std::env::current_dir() {
        cmd.env("GASCII_PROJECT_ROOT", project_root);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            // Detect immediate crash (e.g. Ghostty init failure) and fallback.
            std::thread::sleep(Duration::from_millis(400));
            match child.try_wait() {
                Ok(Some(status)) => {
                    crate::utils::logger::error(&format!(
                        "ghostty exited immediately with status {}",
                        status
                    ));
                    Ok(false)
                }
                Ok(None) => {
                    crate::utils::logger::info(&format!(
                        "launched ghostty binary: {}",
                        ghostty_bin.display()
                    ));
                    Ok(true)
                }
                Err(e) => {
                    crate::utils::logger::error(&format!(
                        "failed to check ghostty process state: {}",
                        e
                    ));
                    Ok(false)
                }
            }
        }
        Err(e) => {
            crate::utils::logger::error(&format!("failed to launch ghostty binary: {}", e));
            Ok(false)
        }
    }
}

fn resolve_ghostty_binary() -> Option<PathBuf> {
    if let Some(path) = find_binary_in_path(constants::GHOSTTY_BIN_NAME) {
        return Some(path);
    }

    let mac_path = PathBuf::from(constants::GHOSTTY_MACOS_APP_BIN);
    if mac_path.exists() {
        return Some(mac_path);
    }

    None
}

#[cfg(target_os = "macos")]
fn resolve_ghostty_bundle_path() -> Option<PathBuf> {
    let bundle = PathBuf::from(constants::GHOSTTY_MACOS_APP_BUNDLE);
    if bundle.exists() {
        return Some(bundle);
    }

    if let Some(bin) = resolve_ghostty_binary() {
        return derive_app_bundle_path(&bin);
    }

    None
}

#[cfg(target_os = "macos")]
fn derive_app_bundle_path(binary_path: &Path) -> Option<PathBuf> {
    for ancestor in binary_path.ancestors() {
        let Some(ext) = ancestor.extension().and_then(|v| v.to_str()) else {
            continue;
        };
        if ext.eq_ignore_ascii_case("app") {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn find_binary_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}
