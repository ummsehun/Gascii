use anyhow::Result;
use std::time::Duration;

use crate::renderer::DisplayMode;
use crate::shared::constants;

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
        "launch selection: video={} mode={} fill={} audio={}",
        selection.video_path.display(),
        mode,
        selection.fill_screen,
        selection
            .audio_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    ));

    if selection.fill_screen {
        crate::utils::terminal_control::request_fullscreen(true);
    } else {
        crate::utils::terminal_control::request_resize(
            constants::WINDOWED_COLUMNS,
            constants::WINDOWED_ROWS,
        );
    }

    std::thread::sleep(Duration::from_millis(150));

    crate::ui::interactive::run_game(
        selection.video_path,
        selection.audio_path,
        selection.mode,
        selection.fill_screen,
    )?;

    Ok(())
}
