use crate::core::render_budget::FrameBudgetPolicy;
use crate::core::viewport::{ViewportLayout, ViewportMode};
use crate::decoder::{FrameData, RenderTarget};
use crate::renderer::cell::CellData;
use crate::renderer::{DisplayManager, FrameProcessor};
use anyhow::{anyhow, Result};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) enum ShutdownReason {
    Completed,
    UserRequested,
    DecoderError(anyhow::Error),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlaybackStats {
    pub frames_rendered: u64,
    pub frames_dropped: u64,
    pub started_at: Instant,
}

impl PlaybackStats {
    pub(crate) fn new() -> Self {
        Self {
            frames_rendered: 0,
            frames_dropped: 0,
            started_at: Instant::now(),
        }
    }
}

pub(crate) fn is_too_late(
    frame: &FrameData,
    playback_time: Duration,
    budget_policy: FrameBudgetPolicy,
) -> bool {
    playback_time
        .checked_sub(frame.timestamp)
        .map(|lag| lag > budget_policy.drop_threshold)
        .unwrap_or(false)
}

pub(crate) fn handle_resize(
    viewport_mode: ViewportMode,
    requested_width: Option<u32>,
    requested_height: Option<u32>,
    budget_policy: FrameBudgetPolicy,
    display: &mut DisplayManager,
    target: &Arc<RwLock<RenderTarget>>,
    layout: &mut ViewportLayout,
    pending_layout: &mut Option<ViewportLayout>,
    cols: u16,
    rows: u16,
    source_aspect: f64,
) -> Result<()> {
    let next_layout = ViewportLayout::calculate(
        cols,
        rows,
        viewport_mode,
        requested_width,
        requested_height,
        budget_policy,
        source_aspect,
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

pub(crate) fn classify_frame(
    frame: FrameData,
    display: &mut DisplayManager,
    target: &Arc<RwLock<RenderTarget>>,
    layout: &mut ViewportLayout,
    pending_layout: &mut Option<ViewportLayout>,
    processor: &mut Option<FrameProcessor>,
    cell_buffer: &mut Option<Vec<CellData>>,
) -> Result<Option<FrameData>> {
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

pub(crate) fn wait_for_resized_frame(
    receiver: &crossbeam_channel::Receiver<Result<FrameData>>,
    width: u32,
    height: u32,
) -> Result<FrameData> {
    loop {
        let frame = receiver
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| anyhow!("Failed to receive resized frame"))??;
        if frame.width == width && frame.height == height {
            return Ok(frame);
        }
    }
}

pub(crate) fn finalize(
    decoder_handle: std::thread::JoinHandle<Result<()>>,
    stats: PlaybackStats,
    reason: ShutdownReason,
) -> Result<()> {
    let decoder_result = decoder_handle
        .join()
        .map_err(|_| anyhow!("Decoder thread panicked"))?;

    let duration = stats.started_at.elapsed();
    match reason {
        ShutdownReason::UserRequested => {
            crate::utils::logger::info(&format!(
                "playback stopped by user: rendered={} dropped={} duration={:.2}s",
                stats.frames_rendered,
                stats.frames_dropped,
                duration.as_secs_f64()
            ));
            println!("\n사용자 종료");
        }
        ShutdownReason::Completed => {
            decoder_result?;
            crate::utils::logger::info(&format!(
                "playback completed: rendered={} dropped={} duration={:.2}s",
                stats.frames_rendered,
                stats.frames_dropped,
                duration.as_secs_f64()
            ));
            println!("\n재생 완료");
        }
        ShutdownReason::DecoderError(error) => {
            let _ = decoder_result;
            crate::utils::logger::error(&format!(
                "playback stopped by decoder error: rendered={} dropped={} duration={:.2}s error={}",
                stats.frames_rendered,
                stats.frames_dropped,
                duration.as_secs_f64(),
                error
            ));
            println!("\n디코더 오류로 종료");
            println!("렌더링 프레임: {}", stats.frames_rendered);
            println!("드롭 프레임: {}", stats.frames_dropped);
            println!("재생 시간: {:.2}초", duration.as_secs_f64());
            return Err(error);
        }
    }

    println!("렌더링 프레임: {}", stats.frames_rendered);
    println!("드롭 프레임: {}", stats.frames_dropped);
    println!("재생 시간: {:.2}초", duration.as_secs_f64());
    Ok(())
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
        *cells = vec![
            CellData::default();
            layout.pixel_width as usize * (layout.pixel_height as usize / 2)
        ];
    }
    display.invalidate_cache();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    #[test]
    fn dropping_receiver_unblocks_bounded_sender() {
        let (sender, receiver) = bounded::<Result<FrameData>>(1);
        sender
            .send(Ok(FrameData::new(vec![0, 0, 0], 1, 2, Duration::ZERO)))
            .unwrap();

        let handle = std::thread::spawn(move || {
            sender.send_timeout(
                Ok(FrameData::new(vec![0, 0, 0], 1, 2, Duration::ZERO)),
                Duration::from_millis(100),
            )
        });

        drop(receiver);
        let result = handle.join().unwrap();
        assert!(matches!(
            result,
            Err(crossbeam_channel::SendTimeoutError::Disconnected(_))
        ));
    }

    #[test]
    fn decoder_error_reason_returns_error_after_join() {
        let handle = std::thread::spawn(|| Ok(()));
        let stats = PlaybackStats::new();
        let result = finalize(
            handle,
            stats,
            ShutdownReason::DecoderError(anyhow!("decode failed")),
        );

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "decode failed");
    }
}
