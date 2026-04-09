use crate::core::audio_manager::AudioManager;
use crate::decoder::{RenderTarget, ScaleMode, VideoDecoder};
use crate::renderer::cell::CellData;
use crate::renderer::{
    select_render_backend, ActiveRenderBackend, DisplayManager, DisplayMode, FrameProcessor,
    RenderBackend, RenderViewport,
};
use crate::sync::MasterClock;
use crate::utils::platform::TerminalCapabilities;
use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

const DEFAULT_QUEUE_CAPACITY: usize = 16;
const FALLBACK_RESIZE_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportMode {
    Fullscreen,
    Cinema16x9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderQuality {
    High,
    Balanced,
    Performance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBudgetPolicy {
    pub quality: RenderQuality,
    pub max_render_cells: u32,
    pub drop_threshold: Duration,
}

impl FrameBudgetPolicy {
    pub fn for_backend(mode: DisplayMode, backend: ActiveRenderBackend) -> Self {
        match (mode, backend) {
            (DisplayMode::Ascii, _) => Self {
                quality: RenderQuality::Balanced,
                max_render_cells: u32::MAX,
                drop_threshold: Duration::from_millis(90),
            },
            (DisplayMode::Rgb, ActiveRenderBackend::KittyGraphics)
            | (DisplayMode::Rgb, ActiveRenderBackend::ITerm2Image) => Self {
                quality: RenderQuality::High,
                max_render_cells: 60_000,
                drop_threshold: Duration::from_millis(75),
            },
            (DisplayMode::Rgb, ActiveRenderBackend::AnsiRgb) => Self {
                quality: RenderQuality::Performance,
                max_render_cells: 18_000,
                drop_threshold: Duration::from_millis(60),
            },
            (DisplayMode::Rgb, ActiveRenderBackend::AnsiAscii) => Self {
                quality: RenderQuality::Balanced,
                max_render_cells: 24_000,
                drop_threshold: Duration::from_millis(75),
            },
        }
    }

    fn apply_to_dimensions(self, width: u32, height: u32, mode: ViewportMode) -> (u32, u32) {
        if self.max_render_cells == u32::MAX {
            return (width.max(1), make_even(height.max(2)));
        }

        let current_cells = width.saturating_mul((height / 2).max(1));
        if current_cells <= self.max_render_cells {
            return (width.max(1), make_even(height.max(2)));
        }

        let scale = (self.max_render_cells as f64 / current_cells as f64).sqrt();
        let scaled_width = ((width as f64) * scale).floor() as u32;
        let scaled_height = ((height as f64) * scale).floor() as u32;
        let scaled_width = scaled_width.max(1);
        let scaled_height = make_even(scaled_height.max(2));

        match mode {
            ViewportMode::Fullscreen => (scaled_width, scaled_height),
            ViewportMode::Cinema16x9 => fit_aspect_16_9(scaled_width, scaled_height),
        }
    }
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
    pub render_backend: RenderBackend,
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
        budget_policy: FrameBudgetPolicy,
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
                budget_policy.apply_to_dimensions(width, height, viewport_mode)
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

                let (width, height) = fit_aspect_16_9(limit_width, limit_height);
                budget_policy.apply_to_dimensions(width, height, viewport_mode)
            }
        };

        let char_width = pixel_width as u16;
        let char_height = (pixel_height / 2) as u16;
        let offset_x = (terminal_cols.saturating_sub(char_width)) / 2;
        let offset_y = (terminal_rows.saturating_sub(char_height)) / 2;

        Self {
            terminal_cols,
            terminal_rows,
            offset_x,
            offset_y,
            pixel_width,
            pixel_height,
        }
    }

    fn recentered_for_terminal(self, terminal_cols: u16, terminal_rows: u16) -> Self {
        let terminal_cols = terminal_cols.max(1);
        let terminal_rows = terminal_rows.max(1);
        let char_width = self.pixel_width as u16;
        let char_height = (self.pixel_height / 2) as u16;

        Self {
            terminal_cols,
            terminal_rows,
            offset_x: (terminal_cols.saturating_sub(char_width)) / 2,
            offset_y: (terminal_rows.saturating_sub(char_height)) / 2,
            ..self
        }
    }

    fn as_render_viewport(self) -> RenderViewport {
        RenderViewport {
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            terminal_cols: self.terminal_cols,
            terminal_rows: self.terminal_rows,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
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
    let capabilities = TerminalCapabilities::detect();
    let active_backend =
        select_render_backend(config.display_mode, config.render_backend, &capabilities)?;
    let budget_policy = FrameBudgetPolicy::for_backend(config.display_mode, active_backend);

    let mut display = DisplayManager::new(config.display_mode, active_backend, capabilities)?;
    let (term_cols, term_rows) = DisplayManager::current_terminal_size_chars()?;
    let mut layout = ViewportLayout::calculate(
        term_cols,
        term_rows,
        config.viewport_mode,
        config.requested_width,
        config.requested_height,
        budget_policy,
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

    let mut processor = active_backend
        .requires_cell_buffer()
        .then(|| FrameProcessor::new(layout.pixel_width as usize, layout.pixel_height as usize));
    let mut cell_buffer = active_backend.requires_cell_buffer().then(|| {
        vec![CellData::default(); layout.pixel_width as usize * (layout.pixel_height as usize / 2)]
    });

    let mut pending_future = frame_receiver
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| anyhow!("Failed to receive first decoded frame"))?;

    if pending_future.width != layout.pixel_width || pending_future.height != layout.pixel_height {
        pending_future =
            wait_for_resized_frame(&frame_receiver, layout.pixel_width, layout.pixel_height)?;
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
    let mut pending_layout: Option<ViewportLayout> = None;
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
                    handle_resize(
                        &config,
                        budget_policy,
                        &mut display,
                        &target,
                        &mut layout,
                        &mut pending_layout,
                        cols,
                        rows,
                    )?;
                }
                _ => {}
            }
        }

        if last_resize_probe.elapsed() >= FALLBACK_RESIZE_POLL {
            let current_size = DisplayManager::current_terminal_size_chars()?;
            if current_size != last_terminal_size {
                last_terminal_size = current_size;
                handle_resize(
                    &config,
                    budget_policy,
                    &mut display,
                    &target,
                    &mut layout,
                    &mut pending_layout,
                    current_size.0,
                    current_size.1,
                )?;
            }
            last_resize_probe = Instant::now();
        }

        let playback_time = clock.elapsed();
        let mut frame_to_render = None;

        if let Some(frame) = future_frame.take() {
            if let Some(processed_frame) = classify_frame(
                frame,
                &mut display,
                &target,
                &mut layout,
                &mut pending_layout,
                &mut processor,
                &mut cell_buffer,
            )? {
                if processed_frame.width == layout.pixel_width
                    && processed_frame.height == layout.pixel_height
                {
                    if is_too_late(&processed_frame, playback_time, budget_policy) {
                        frames_dropped += 1;
                    } else if processed_frame.timestamp <= playback_time {
                        frame_to_render = Some(processed_frame);
                    } else {
                        future_frame = Some(processed_frame);
                    }
                } else {
                    frames_dropped += 1;
                }
            }
        }

        loop {
            match frame_receiver.try_recv() {
                Ok(frame) => {
                    let Some(frame) = classify_frame(
                        frame,
                        &mut display,
                        &target,
                        &mut layout,
                        &mut pending_layout,
                        &mut processor,
                        &mut cell_buffer,
                    )? else {
                        frames_dropped += 1;
                        continue;
                    };

                    if is_too_late(&frame, playback_time, budget_policy) {
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
            let render_viewport = layout.as_render_viewport();
            let rgb_cells = if active_backend.requires_cell_buffer() {
                let processor = processor
                    .as_mut()
                    .ok_or_else(|| anyhow!("RGB ANSI renderer missing frame processor"))?;
                let cells = cell_buffer
                    .as_mut()
                    .ok_or_else(|| anyhow!("RGB ANSI renderer missing cell buffer"))?;
                processor.process_frame_into(&frame.buffer, cells);
                Some(cells.as_slice())
            } else {
                None
            };

            display.render(&frame.buffer, rgb_cells, render_viewport)?;
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

fn is_too_late(
    frame: &crate::decoder::FrameData,
    playback_time: Duration,
    budget_policy: FrameBudgetPolicy,
) -> bool {
    playback_time
        .checked_sub(frame.timestamp)
        .map(|lag| lag > budget_policy.drop_threshold)
        .unwrap_or(false)
}

fn relayout(
    display: &mut DisplayManager,
    target: &Arc<RwLock<RenderTarget>>,
    layout: &mut ViewportLayout,
    processor: &mut Option<FrameProcessor>,
    cell_buffer: &mut Option<Vec<CellData>>,
    next_layout: ViewportLayout,
) -> Result<()> {
    *layout = next_layout;
    {
        let mut guard = target
            .write()
            .map_err(|_| anyhow!("render target lock poisoned"))?;
        *guard = RenderTarget::new(layout.pixel_width, layout.pixel_height);
    }

    if let Some(processor) = processor {
        *processor = FrameProcessor::new(layout.pixel_width as usize, layout.pixel_height as usize);
    }
    if let Some(cells) = cell_buffer {
        *cells =
            vec![CellData::default(); layout.pixel_width as usize * (layout.pixel_height as usize / 2)];
    }
    display.invalidate_cache();
    Ok(())
}

fn handle_resize(
    config: &PlaybackConfig,
    budget_policy: FrameBudgetPolicy,
    display: &mut DisplayManager,
    target: &Arc<RwLock<RenderTarget>>,
    layout: &mut ViewportLayout,
    pending_layout: &mut Option<ViewportLayout>,
    cols: u16,
    rows: u16,
) -> Result<()> {
    let next_layout = ViewportLayout::calculate(
        cols,
        rows,
        config.viewport_mode,
        config.requested_width,
        config.requested_height,
        budget_policy,
    );

    let recentered = (*layout).recentered_for_terminal(cols, rows);
    let desired_changed = next_layout.pixel_width != layout.pixel_width
        || next_layout.pixel_height != layout.pixel_height;
    let offset_changed = recentered.terminal_cols != layout.terminal_cols
        || recentered.terminal_rows != layout.terminal_rows
        || recentered.offset_x != layout.offset_x
        || recentered.offset_y != layout.offset_y;

    if !desired_changed && !offset_changed {
        return Ok(());
    }

    if offset_changed {
        *layout = recentered;
        display.invalidate_cache();
    }

    if desired_changed {
        let mut guard = target
            .write()
            .map_err(|_| anyhow!("render target lock poisoned"))?;
        *guard = RenderTarget::new(next_layout.pixel_width, next_layout.pixel_height);
        *pending_layout = Some(next_layout);
    }

    Ok(())
}

fn classify_frame(
    frame: crate::decoder::FrameData,
    display: &mut DisplayManager,
    target: &Arc<RwLock<RenderTarget>>,
    layout: &mut ViewportLayout,
    pending_layout: &mut Option<ViewportLayout>,
    processor: &mut Option<FrameProcessor>,
    cell_buffer: &mut Option<Vec<CellData>>,
) -> Result<Option<crate::decoder::FrameData>> {
    if frame.width == layout.pixel_width && frame.height == layout.pixel_height {
        return Ok(Some(frame));
    }

    if let Some(next_layout) = pending_layout {
        if frame.width == next_layout.pixel_width && frame.height == next_layout.pixel_height {
            let next_layout = *next_layout;
            *pending_layout = None;
            relayout(display, target, layout, processor, cell_buffer, next_layout)?;
            return Ok(Some(frame));
        }
    }

    Ok(None)
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
        let layout = ViewportLayout::calculate(
            240,
            68,
            ViewportMode::Cinema16x9,
            None,
            None,
            FrameBudgetPolicy::for_backend(DisplayMode::Rgb, ActiveRenderBackend::KittyGraphics),
        );
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
            FrameBudgetPolicy::for_backend(DisplayMode::Rgb, ActiveRenderBackend::KittyGraphics),
        );
        assert_eq!(layout.pixel_width, 120);
        assert_eq!(layout.pixel_height, 80);
    }

    #[test]
    fn ansi_rgb_budget_scales_down_large_viewports() {
        let layout = ViewportLayout::calculate(
            320,
            120,
            ViewportMode::Fullscreen,
            None,
            None,
            FrameBudgetPolicy::for_backend(DisplayMode::Rgb, ActiveRenderBackend::AnsiRgb),
        );
        let cells = layout.pixel_width * (layout.pixel_height / 2);
        assert!(cells <= FrameBudgetPolicy::for_backend(DisplayMode::Rgb, ActiveRenderBackend::AnsiRgb).max_render_cells);
    }
}
