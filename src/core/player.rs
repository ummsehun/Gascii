use crate::core::playback_runtime::{
    classify_frame, finalize, handle_resize, is_too_late, wait_for_resized_frame, PlaybackStats,
    ShutdownReason,
};
use crate::core::render_budget::FrameBudgetPolicy;
use crate::core::viewport::ViewportLayout;
use crate::core::audio_manager::AudioManager;
use crate::decoder::{RenderTarget, ScaleMode, VideoDecoder};
use crate::renderer::cell::CellData;
use crate::renderer::{
    ActiveRenderBackend, DisplayManager, DisplayMode, FrameProcessor, TruecolorPolicy,
};
use crate::sync::MasterClock;
use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub use crate::core::render_budget::RenderQuality;
pub use crate::core::viewport::ViewportMode;

const DEFAULT_QUEUE_CAPACITY: usize = 16;
const FALLBACK_RESIZE_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct PlaybackConfig {
    pub video_path: PathBuf,
    pub audio_path: Option<PathBuf>,
    pub requested_width: Option<u32>,
    pub requested_height: Option<u32>,
    pub requested_fps: Option<u32>,
    pub display_mode: DisplayMode,
    pub viewport_mode: ViewportMode,
    pub quality: RenderQuality,
    pub truecolor_policy: TruecolorPolicy,
}

pub fn play(config: PlaybackConfig) -> Result<()> {
    let requested_backend = ActiveRenderBackend::for_mode(config.display_mode);
    let mut display = DisplayManager::new(
        config.display_mode,
        requested_backend,
        config.truecolor_policy,
    )?;
    let active_backend = display.active_backend();
    let budget_policy =
        FrameBudgetPolicy::for_backend(config.display_mode, active_backend, config.quality);
    let (term_cols, term_rows) = DisplayManager::current_terminal_size_chars()?;
    let target = Arc::new(RwLock::new(RenderTarget::new(1, 2)));

    let decoder = VideoDecoder::new(
        config.video_path.to_string_lossy().as_ref(),
        target.clone(),
        ScaleMode::Fit,
    )?;
    let source_aspect = decoder.source_aspect_ratio();
    let mut layout = ViewportLayout::calculate(
        term_cols,
        term_rows,
        config.viewport_mode,
        config.requested_width,
        config.requested_height,
        budget_policy,
        source_aspect,
    );
    {
        let mut guard = target
            .write()
            .map_err(|_| anyhow!("render target lock poisoned"))?;
        *guard = RenderTarget::new(layout.pixel_width, layout.pixel_height);
    }

    let source_fps = decoder.get_fps();
    let playback_fps = config
        .requested_fps
        .filter(|value| *value > 0)
        .map(|value| value as f64)
        .unwrap_or(source_fps);

    let (frame_sender, frame_receiver) = crossbeam_channel::bounded(DEFAULT_QUEUE_CAPACITY);
    let decoder_handle = decoder.spawn_decoding_thread(frame_sender, playback_fps);
    let mut frame_receiver = Some(frame_receiver);
    let receiver = frame_receiver
        .as_ref()
        .ok_or_else(|| anyhow!("frame receiver not initialized"))?;

    let mut processor = active_backend
        .requires_cell_buffer()
        .then(|| FrameProcessor::new(layout.pixel_width as usize, layout.pixel_height as usize));
    let mut cell_buffer = active_backend.requires_cell_buffer().then(|| {
        vec![CellData::default(); layout.pixel_width as usize * (layout.pixel_height as usize / 2)]
    });

    let mut pending_future = receiver
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| anyhow!("Failed to receive first decoded frame"))??;

    if pending_future.width != layout.pixel_width || pending_future.height != layout.pixel_height {
        pending_future = wait_for_resized_frame(receiver, layout.pixel_width, layout.pixel_height)?;
    }

    let audio_manager = if config.audio_path.is_some() {
        Some(AudioManager::new()?)
    } else {
        None
    };
    let clock_start = if let (Some(audio), Some(audio_path)) = (&audio_manager, &config.audio_path)
    {
        audio.play(audio_path.to_string_lossy().as_ref())?
    } else {
        Instant::now()
    };
    let clock = MasterClock::from_start(clock_start);

    let mut stats = PlaybackStats::new();
    let mut shutdown_reason = ShutdownReason::Completed;
    let mut decoder_disconnected = false;
    let mut last_resize_probe = Instant::now();
    let mut last_terminal_size = (layout.terminal_cols, layout.terminal_rows);
    let mut pending_layout: Option<ViewportLayout> = None;
    let mut future_frame = Some(pending_future);

    loop {
        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(key) if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) => {
                    shutdown_reason = ShutdownReason::UserRequested;
                    break;
                }
                Event::Resize(cols, rows) => {
                    last_terminal_size = (cols, rows);
                    resize_playback(
                        &config,
                        budget_policy,
                        &mut display,
                        &target,
                        &mut layout,
                        &mut pending_layout,
                        cols,
                        rows,
                        source_aspect,
                    )?;
                }
                _ => {}
            }
        }
        if matches!(shutdown_reason, ShutdownReason::UserRequested) {
            break;
        }

        if last_resize_probe.elapsed() >= FALLBACK_RESIZE_POLL {
            let current_size = DisplayManager::current_terminal_size_chars()?;
            if current_size != last_terminal_size {
                last_terminal_size = current_size;
                resize_playback(
                    &config,
                    budget_policy,
                    &mut display,
                    &target,
                    &mut layout,
                    &mut pending_layout,
                    current_size.0,
                    current_size.1,
                    source_aspect,
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
                        stats.frames_dropped += 1;
                    } else if processed_frame.timestamp <= playback_time {
                        frame_to_render = Some(processed_frame);
                    } else {
                        future_frame = Some(processed_frame);
                    }
                } else {
                    stats.frames_dropped += 1;
                }
            }
        }

        let receiver = frame_receiver
            .as_ref()
            .ok_or_else(|| anyhow!("frame receiver closed before playback loop ended"))?;
        loop {
            match receiver.try_recv() {
                Ok(frame) => {
                    let frame = match frame {
                        Ok(frame) => frame,
                        Err(error) => {
                            shutdown_reason = ShutdownReason::DecoderError(error);
                            break;
                        }
                    };
                    let Some(frame) = classify_frame(
                        frame,
                        &mut display,
                        &target,
                        &mut layout,
                        &mut pending_layout,
                        &mut processor,
                        &mut cell_buffer,
                    )?
                    else {
                        stats.frames_dropped += 1;
                        continue;
                    };

                    if is_too_late(&frame, playback_time, budget_policy) {
                        stats.frames_dropped += 1;
                        continue;
                    }

                    if frame.timestamp <= playback_time {
                        if frame_to_render.is_some() {
                            stats.frames_dropped += 1;
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
        if matches!(shutdown_reason, ShutdownReason::DecoderError(_)) {
            break;
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
            stats.frames_rendered += 1;
            continue;
        }

        if decoder_disconnected && audio_is_done(&audio_manager) {
            break;
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    if let Some(audio) = &audio_manager {
        let _ = audio.stop();
    }
    drop(frame_receiver.take());

    finalize(decoder_handle, stats, shutdown_reason)
}

fn resize_playback(
    config: &PlaybackConfig,
    budget_policy: FrameBudgetPolicy,
    display: &mut DisplayManager,
    target: &Arc<RwLock<RenderTarget>>,
    layout: &mut ViewportLayout,
    pending_layout: &mut Option<ViewportLayout>,
    cols: u16,
    rows: u16,
    source_aspect: f64,
) -> Result<()> {
    handle_resize(
        config.viewport_mode,
        config.requested_width,
        config.requested_height,
        budget_policy,
        display,
        target,
        layout,
        pending_layout,
        cols,
        rows,
        source_aspect,
    )
}

fn audio_is_done(audio_manager: &Option<AudioManager>) -> bool {
    match audio_manager {
        Some(audio) => audio.is_finished().unwrap_or(true),
        None => true,
    }
}
