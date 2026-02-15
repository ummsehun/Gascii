mod core;
mod utils;

// New direct module imports
mod sync;
mod audio;
mod decoder;
mod renderer;
mod ui;
mod analyzer;

use clap::{Parser, Subcommand};
use anyhow::Result;

use crate::renderer::DisplayMode;
use crate::core::extractor;

// New module imports

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
        #[arg(short, long)]
        input: String,
        #[arg(short, long)]
        output_dir: String,
        #[arg(short, long, default_value_t = 265)]
        width: u32,
        #[arg(short, long, default_value_t = 65)]
        height: u32,
        #[arg(short, long, default_value_t = 60)]
        fps: u32,
    },
    /// Play the animation
    Play {
        #[arg(short, long)]
        frames_dir: String,
        #[arg(short, long)]
        audio: Option<String>,
        #[arg(short, long, default_value_t = 60)]
        fps: u32,
        #[arg(short, long, value_enum, default_value_t = DisplayMode::Rgb)]
        mode: DisplayMode,
    },
    /// Play video directly (real-time, no extraction)
    PlayLive {
        #[arg(short, long)]
        video: String,
        #[arg(short, long)]
        audio: Option<String>,
        #[arg(short, long, default_value_t = 265, help = "Requested width in pixels for video scaling (applies to the decoder and processor)")]
        width: u32,
        #[arg(short, long, default_value_t = 65, help = "Requested height in pixels for video scaling (applies to the decoder and processor)")]
        height: u32,
        #[arg(short, long, default_value_t = 0)]
        fps: u32,
        #[arg(short, long, value_enum, default_value_t = DisplayMode::Rgb)]
        mode: DisplayMode,
        #[arg(short, long, default_value_t = false, help = "If true, Fill mode: crop to fill 16:9 box (center crop)")]
        fill: bool,
    },
    /// Detect platform info
    Detect,
    /// Query the terminal size as crossterm sees it
    TerminalSize,
    /// Interactive Menu Mode (for UI scaling)
    Menu,
    /// Interactive Mode (Legacy)
    Interactive,
}

fn main() -> Result<()> {
    // 1. Initialize Logger (error.log)
    crate::utils::logger::init("error.log");
    
    // 2. Reset Terminal State (Fix "Staircase" issue from previous crashes)
    // We ignore errors here because the terminal might not be in raw mode
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

    let cli = Cli::parse();

    match &cli.command {
        Commands::Extract { input, output_dir, width, height, fps } => {
            extractor::extract_frames(input, output_dir, *width, *height, *fps)?;
        }
        Commands::Play { frames_dir: _, audio: _, fps: _, mode: _ } => {
            // Legacy play command
            println!("Legacy Play command. Use PlayLive for real-time playback.");
        }
        Commands::PlayLive { video, audio, width: _, height: _, fps: _, mode, fill } => {
             let video_path = std::path::PathBuf::from(video);
             let audio_path = audio.as_ref().map(|p| std::path::PathBuf::from(p));
             
             crate::ui::interactive::run_game(video_path, audio_path, *mode, *fill)?;
        }
        Commands::Detect => {
             // Detect command has no input field in the struct definition I saw?
             // Wait, let me check the struct definition again.
             // Line 79: Detect,
             // So it has no fields.
             let info = crate::utils::platform::PlatformInfo::detect()?;
             println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Commands::TerminalSize => {
            let (cols, rows) = crossterm::terminal::size()?;
            println!("{}x{}", cols, rows);
        }
        Commands::Interactive => {
            // Legacy interactive mode redirected to Menu
            crate::ui::menu::run_menu()?;
        }
        Commands::Menu => {
            crate::ui::menu::run_menu()?;
        }
    }

    Ok(())
}
    
