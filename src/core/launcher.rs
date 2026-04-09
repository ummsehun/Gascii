use anyhow::Result;
use std::time::Duration;

use crate::core::player::{PlaybackConfig, ViewportMode};
use crate::shared::constants;

pub fn run() -> Result<()> {
    let Some(selection) = crate::ui::menu::run_menu()? else {
        println!("Menu cancelled.");
        return Ok(());
    };

    let mode = match selection.display_mode {
        crate::renderer::DisplayMode::Rgb => "rgb",
        crate::renderer::DisplayMode::Ascii => "ascii",
    };
    let viewport = match selection.viewport_mode {
        ViewportMode::Fullscreen => "fullscreen",
        ViewportMode::Cinema16x9 => "cinema-16:9",
    };

    crate::utils::logger::info(&format!(
        "launch selection: video={} mode={} viewport={} audio={}",
        selection.video_path.display(),
        mode,
        viewport,
        selection
            .audio_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    ));

    if matches!(selection.viewport_mode, ViewportMode::Fullscreen) {
        crate::utils::terminal_control::request_fullscreen(true);
    } else {
        crate::utils::terminal_control::request_resize(
            constants::WINDOWED_COLUMNS,
            constants::WINDOWED_ROWS,
        );
    }

    std::thread::sleep(Duration::from_millis(150));

    crate::core::player::play(PlaybackConfig {
        video_path: selection.video_path,
        audio_path: selection.audio_path,
        requested_width: None,
        requested_height: None,
        requested_fps: None,
        display_mode: selection.display_mode,
        viewport_mode: selection.viewport_mode,
    })?;

    Ok(())
}
