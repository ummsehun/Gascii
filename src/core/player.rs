use crate::core::audio_manager::AudioManager;
use crate::decoder::{RenderTarget, ScaleMode, VideoDecoder};
use crate::renderer::{DisplayManager, DisplayMode, FrameProcessor};
use crate::renderer::cell::CellData;
use crate::sync::MasterClock;
use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

const DEFAULT_QUEUE_CAPACITY: usize = 120;
const FALLBACK_RESIZE_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportMode {
    Fullscreen,
    Cinema16x9,
}

#[derive(Debug, Clone)]
pub struct PlaybackConfig {
    pub video_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub requested_width: Option<u32>,
    pub requested_height: Option<u32>,
    pub requested_fps: Option<u32>,
    pub display_mode: DisplayMode,
    pub viewport_mode: ViewportMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportLayout {
    pub terminal_cols: u16,
    pub terminal_rows: u16,
    pub offset_x: u16,
    pub offset_y: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl ViewportLayout {
    fn calculate(
        terminal_cols: u16,
        terminal_rows: u16,
        viewport_mode: ViewportMode,
        requested_width: Option<u32>,
        requested_height: Option<u32>,
    ) -> Self {
        let terminal_cols = terminal_cols.max(1);
        let terminal_rows = terminal_rows.max(1);

        let max_pixel_width = terminal_cols as u32;
        let max_pixel_height = (terminal_rows as u32).saturating_mul(2).max(2);

        let (pixel_width, pixel_height) = match viewport_mode {
            ViewportMode::Fullscreen => {
                let width = requested_width
                    .map(|value| value.min(max_pixel_width).max(1))
                    .unwrap_or(max_pixel_width);
                let height = requested_height
                    .map(|value| value.min(max_pixel_height).max(2))
                    .unwrap_or(max_pixel_height);
                (width.max(1), make_even(height.max(2)))
            }
            ViewportMode::Cinema16x9 => {
                let fitted_width = max_pixel_width;
                let fitted_height = make_even(
                    ((fitted_width as f64 / (16.0 / 9.0)).floor() as u32)
                        .min(max_pixel_height)
                        .max(2),
                );

                let (bounded_width, bounded_height) = if fitted_height > max_pixel_height {
                    let height = max_pixel_height;
                    let width = ((height as f64 * (16.0 / 9.0)).floor() as u32)
                        .min(max_pixel_width)
                        .max(1);
                    (width, height)
                } else {
                    (fitted_width, fitted_height)
                };

                let limit_width = requested_width
                    .map(|value| value.min(bounded_width).max(1))
                    .unwrap_or(bounded_width);
                let limit_height = requested_height
                    .map(|value| value.min(bounded_height).max(2))
                    .unwrap_or(bounded_height);

                fit_aspect_16_9(limit_width, limit_height)
            }
        };

        let char_width = pixel_width as u16;
        let char_height = (pixel_height / 2) as u16;
        let offset_x = ((terminal_cols.saturating_sub(char_width)) / 2).max(0);
        let offset_y = ((terminal_rows.saturating_sub(char_height)) / 2).max(0);

        Self {
            terminal_cols,
            terminal_rows,
            offset_x,
            offset_y,
            pixel_width,
            pixel_height,
        }
    }
}

fn fit_aspect_16_9(max_width: u32, max_height: u32) -> (u32, u32) {
    let max_width = max_width.max(1);
    let max_height = make_even(max_height.max(2));
    let aspect = 16.0 / 9.0;
    let width_from_height = ((max_height as f64) * aspect).floor() as u32;

    if width_from_height <= max_width {
        (width_from_height.max(1), max_height)
    } else {
        let width = max_width;
        let height = make_even(((width as f64) / aspect).floor() as u32).max(2);
        (width.max(1), height)
    }
}

fn make_even(value: u32) -> u32 {
    let clamped = value.max(2);
    if clamped % 2 == 0 {
        clamped
    } else {
        clamped - 1
    }
}

pub fn play(config: PlaybackConfig) -> Result<()> {
    let mut display = DisplayManager::new(config.display_mode)?;
    let (term_cols, term_rows) = DisplayManager::current_terminal_size_chars()?;
    let mut layout = ViewportLayout::calculate(
        term_cols,
        term_rows,
        config.viewport_mode,
        config.requested_width,
        config.requested_height,
    );

    let target = Arc::new(RwLock::new(RenderTarget::new(
        layout.pixel_width,
        layout.pixel_height,
    )));
    let scale_mode = match config.viewport_mode {
        ViewportMode::Fullscreen => ScaleMode::CropToFill,
        ViewportMode::Cinema16x9 => ScaleMode::Fit,
    };

    let decoder = VideoDecoder::new(
        config.video_path.to_string_lossy().as_ref(),
        target.clone(),
        scale_mode,
    )?;
    let source_fps = decoder.get_fps();
    let playback_fps = config
        .requested_fps
        .filter(|value| *value > 0)
        .map(|value| value as f64)
        .unwrap_or(source_fps);

    let (frame_sender, frame_receiver) = crossbeam_channel::bounded(DEFAULT_QUEUE_CAPACITY);
    let decoder_handle = decoder.spawn_decoding_thread(frame_sender, playback_fps);

    let mut processor = FrameProcessor::new(layout.pixel_width as usize, layout.pixel_height as usize);
    let mut cell_buffer =
        vec![CellData::default(); layout.pixel_width as usize * (layout.pixel_height as usize / 2)];

    let mut pending_future = frame_receiver
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| anyhow!("Failed to receive first decoded frame"))?;

    if pending_future.width != layout.pixel_width || pending_future.height != layout.pixel_height {
        pending_future = wait_for_resized_frame(&frame_receiver, layout.pixel_width, layout.pixel_height)?;
    }

    let audio_manager = if config.audio_path.is_some() {
        Some(AudioManager::new()?)
    } else {
        None
    };
    let clock_start = if let (Some(audio), Some(audio_path)) = (&audio_manager, &config.audio_path) {
        audio.play(audio_path.to_string_lossy().as_ref())?
    } else {
        Instant::now()
    };
    let clock = MasterClock::from_start(clock_start);

    let mut frames_rendered = 0u64;
    let mut frames_dropped = 0u64;
    let mut decoder_disconnected = false;
    let started_at = Instant::now();
    let mut last_resize_probe = Instant::now();
    let mut last_terminal_size = (layout.terminal_cols, layout.terminal_rows);
    let mut future_frame = Some(pending_future);

    loop {
        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key) if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) => {
                    if let Some(audio) = &audio_manager {
                        let _ = audio.stop();
                    }
                    return finalize(decoder_handle, frames_rendered, frames_dropped, started_at);
                }
                Event::Resize(cols, rows) => {
                    last_terminal_size = (cols, rows);
                    relayout(
                        &config,
                        &mut display,
                        &target,
                        &mut layout,
                        &mut processor,
                        &mut cell_buffer,
                        cols,
                        rows,
                    )?;
                    future_frame = None;
                }
                _ => {}
            }
        }

        if last_resize_probe.elapsed() >= FALLBACK_RESIZE_POLL {
            let current_size = DisplayManager::current_terminal_size_chars()?;
            if current_size != last_terminal_size {
                last_terminal_size = current_size;
                relayout(
                    &config,
                    &mut display,
                    &target,
                    &mut layout,
                    &mut processor,
                    &mut cell_buffer,
                    current_size.0,
                    current_size.1,
                )?;
                future_frame = None;
            }
            last_resize_probe = Instant::now();
        }

        let playback_time = clock.elapsed();
        let mut frame_to_render = None;

        if let Some(frame) = future_frame.take() {
            if frame.width == layout.pixel_width && frame.height == layout.pixel_height {
                if frame.timestamp <= playback_time {
                    frame_to_render = Some(frame);
                } else {
                    future_frame = Some(frame);
                }
            }
        }

        loop {
            match frame_receiver.try_recv() {
                Ok(frame) => {
                    if frame.width != layout.pixel_width || frame.height != layout.pixel_height {
                        frames_dropped += 1;
                        continue;
                    }

                    if frame.timestamp <= playback_time {
                        if frame_to_render.is_some() {
                            frames_dropped += 1;
                        }
                        frame_to_render = Some(frame);
                    } else {
                        future_frame = Some(frame);
                        break;
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    decoder_disconnected = true;
                    break;
                }
            }
        }

        if frame_to_render.is_none() {
            if let Some(frame) = future_frame.take() {
                if frame.width == layout.pixel_width && frame.height == layout.pixel_height {
                    if frame.timestamp > playback_time {
                        let wait_time = frame.timestamp - playback_time;
                        if wait_time > Duration::from_millis(1) {
                            std::thread::sleep(wait_time);
                        }
                    }
                    frame_to_render = Some(frame);
                }
            }
        }

        if let Some(frame) = frame_to_render {
            processor.process_frame_into(&frame.buffer, &mut cell_buffer);
            display.render_diff(
                &cell_buffer,
                layout.pixel_width as usize,
                layout.offset_x,
                layout.offset_y,
                layout.terminal_cols,
                layout.terminal_rows,
            )?;
            frames_rendered += 1;
            continue;
        }

        if decoder_disconnected {
            let audio_done = match &audio_manager {
                Some(audio) => audio.is_finished().unwrap_or(true),
                None => true,
            };
            if audio_done {
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    if let Some(audio) = &audio_manager {
        let _ = audio.stop();
    }

    finalize(decoder_handle, frames_rendered, frames_dropped, started_at)
}

fn relayout(
    config: &PlaybackConfig,
    display: &mut DisplayManager,
    target: &Arc<RwLock<RenderTarget>>,
    layout: &mut ViewportLayout,
    processor: &mut FrameProcessor,
    cell_buffer: &mut Vec<CellData>,
    cols: u16,
    rows: u16,
) -> Result<()> {
    let next_layout = ViewportLayout::calculate(
        cols,
        rows,
        config.viewport_mode,
        config.requested_width,
        config.requested_height,
    );

    if next_layout == *layout {
        return Ok(());
    }

    *layout = next_layout;
    {
        let mut guard = target
            .write()
            .map_err(|_| anyhow!("render target lock poisoned"))?;
        *guard = RenderTarget::new(layout.pixel_width, layout.pixel_height);
    }

    *processor = FrameProcessor::new(layout.pixel_width as usize, layout.pixel_height as usize);
    *cell_buffer =
        vec![CellData::default(); layout.pixel_width as usize * (layout.pixel_height as usize / 2)];
    display.invalidate_cache();
    Ok(())
}

fn wait_for_resized_frame(
    receiver: &crossbeam_channel::Receiver<crate::decoder::FrameData>,
    width: u32,
    height: u32,
) -> Result<crate::decoder::FrameData> {
    loop {
        let frame = receiver
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| anyhow!("Failed to receive resized frame"))?;
        if frame.width == width && frame.height == height {
            return Ok(frame);
        }
    }
}

fn finalize(
    decoder_handle: std::thread::JoinHandle<Result<()>>,
    frames_rendered: u64,
    frames_dropped: u64,
    started_at: Instant,
) -> Result<()> {
    let _ = decoder_handle.join();
    let duration = started_at.elapsed();
    println!("\n재생 완료");
    println!("렌더링 프레임: {}", frames_rendered);
    println!("드롭 프레임: {}", frames_dropped);
    println!("재생 시간: {:.2}초", duration.as_secs_f64());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cinema_layout_keeps_16_9_ratio() {
        let layout = ViewportLayout::calculate(240, 68, ViewportMode::Cinema16x9, None, None);
        let ratio = layout.pixel_width as f64 / layout.pixel_height as f64;
        assert!((ratio - (16.0 / 9.0)).abs() < 0.05);
    }

    #[test]
    fn fullscreen_layout_uses_requested_limits() {
        let layout = ViewportLayout::calculate(
            240,
            68,
            ViewportMode::Fullscreen,
            Some(120),
            Some(80),
        );
        assert_eq!(layout.pixel_width, 120);
        assert_eq!(layout.pixel_height, 80);
    }

    #[test]
    fn cinema_layout_centers_output() {
        let layout = ViewportLayout::calculate(200, 60, ViewportMode::Cinema16x9, None, None);
        assert!(layout.offset_x > 0 || layout.offset_y > 0);
        assert_eq!(layout.pixel_height % 2, 0);
    }
}
