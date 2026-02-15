#![allow(dead_code, unused_variables, unused_imports)]
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::time::Instant;

pub fn extract_frames(_input: &str, _output_dir: &str, _width: u32, _height: u32, _fps: u32) -> Result<()> {
    // FFmpeg extraction functionality has been disabled in Rust side.
    // Use provided Python scripts or OpenCV VideoDecoder for real-time playback.
    // Return a clear error instead of panicking to keep the CLI stable.
    anyhow::bail!("The `extract` command is not implemented in Rust. Use 'scripts/extract_*' scripts or the 'play-live' command for real-time playback.")
}
