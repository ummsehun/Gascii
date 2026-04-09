mod core;
mod decoder;
mod renderer;
mod shared;
mod sync;
mod ui;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::IsTerminal;

use crate::core::extractor;
use crate::renderer::{DisplayMode, RenderBackend};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract frames from video
    Extract {
        #[arg(short = 'i', long)]
        input: String,
        #[arg(short = 'o', long)]
        output_dir: String,
        #[arg(short = 'w', long, default_value_t = 265)]
        width: u32,
        #[arg(short = 'H', long, default_value_t = 65)]
        height: u32,
        #[arg(short = 'p', long, default_value_t = 60)]
        fps: u32,
    },
    /// Play the animation
    Play {
        #[arg(short = 'd', long)]
        frames_dir: String,
        #[arg(short = 'a', long)]
        audio: Option<String>,
        #[arg(short = 'p', long, default_value_t = 60)]
        fps: u32,
        #[arg(short = 'm', long, value_enum, default_value_t = DisplayMode::Rgb)]
        mode: DisplayMode,
    },
    /// Play video directly (real-time, no extraction)
    PlayLive {
        #[arg(short = 'v', long)]
        video: String,
        #[arg(short = 'a', long)]
        audio: Option<String>,
        #[arg(
            short = 'w',
            long,
            default_value_t = 265,
            help = "Requested width in pixels for video scaling (applies to the decoder and processor)"
        )]
        width: u32,
        #[arg(
            short = 'H',
            long,
            default_value_t = 65,
            help = "Requested height in pixels for video scaling (applies to the decoder and processor)"
        )]
        height: u32,
        #[arg(short = 'p', long, default_value_t = 0)]
        fps: u32,
        #[arg(short = 'm', long, value_enum, default_value_t = DisplayMode::Rgb)]
        mode: DisplayMode,
        #[arg(long, value_enum, default_value_t = RenderBackend::Auto)]
        renderer: RenderBackend,
        #[arg(
            short = 'F',
            long,
            default_value_t = false,
            help = "If true, Fill mode: crop to fill 16:9 box (center crop)"
        )]
        fill: bool,
    },
    /// Detect platform info
    Detect,
    /// Query the terminal size as crossterm sees it
    TerminalSize,
    /// Interactive launch menu (menu + playback)
    Menu,
    /// Interactive Mode (Legacy)
    Interactive,
}

fn main() -> Result<()> {
    // 1. Initialize log files
    crate::utils::logger::init();

    // 2. Reset Terminal State (Fix "Staircase" issue from previous crashes)
    // We ignore errors here because the terminal might not be in raw mode
    let _ = crossterm::terminal::disable_raw_mode();
    if std::io::stdout().is_terminal() {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }

    let cli = Cli::parse();

    match &cli.command {
        Commands::Extract {
            input,
            output_dir,
            width,
            height,
            fps,
        } => {
            extractor::extract_frames(input, output_dir, *width, *height, *fps)?;
        }
        Commands::Play {
            frames_dir: _,
            audio: _,
            fps: _,
            mode: _,
        } => {
            // Legacy play command
            println!("Legacy Play command. Use PlayLive for real-time playback.");
        }
        Commands::PlayLive {
            video,
            audio,
            width,
            height,
            fps,
            mode,
            renderer,
            fill,
        } => {
            crate::core::player::play(crate::core::player::PlaybackConfig {
                video_path: std::path::PathBuf::from(video),
                audio_path: audio.as_ref().map(std::path::PathBuf::from),
                requested_width: Some(*width),
                requested_height: Some(*height),
                requested_fps: if *fps > 0 { Some(*fps) } else { None },
                display_mode: *mode,
                viewport_mode: if *fill {
                    crate::core::player::ViewportMode::Fullscreen
                } else {
                    crate::core::player::ViewportMode::Cinema16x9
                },
                render_backend: *renderer,
            })?;
        }
        Commands::Detect => {
            let info = crate::utils::platform::PlatformInfo::detect()?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Commands::TerminalSize => {
            let (cols, rows) = crossterm::terminal::size()?;
            println!("{}x{}", cols, rows);
        }
        Commands::Menu => {
            crate::core::launcher::run()?;
        }
        Commands::Interactive => {
            // Legacy alias
            crate::core::launcher::run()?;
        }
    }

    Ok(())
}
