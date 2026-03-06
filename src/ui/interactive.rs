use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use std::path::PathBuf;
use std::time::{Duration, Instant};

// Direct module imports
use crate::audio::AudioPlayer;
use crate::decoder::VideoDecoder;
use crate::renderer::cell::CellData;
use crate::renderer::{DisplayManager, DisplayMode, FrameProcessor};
use crate::shared::constants;
use crate::sync::MasterClock;

/// 디버그 로그 파일에 메시지를 기록합니다.

pub fn run_game(
    video_path: PathBuf,
    audio_path: Option<PathBuf>,
    mode: DisplayMode,
    fill_screen: bool,
) -> Result<()> {
    // 1. Terminal Setup
    let (terminal_w, terminal_h) = resolve_terminal_size(fill_screen)?;

    // Calculate target dimensions.
    let (target_w, target_h) = compute_target_dimensions(terminal_w, terminal_h, fill_screen);

    println!(
        "\n🚀 재생 시작: {} ({}x{} 픽셀, {})",
        video_path.file_name().unwrap().to_string_lossy(),
        target_w,
        target_h,
        if fill_screen { "전체화면" } else { "16:9" }
    );

    // === START PRODUCER-CONSUMER IMPLEMENTATION WITH SYNC ===

    // Run ANSI rendering (optimized for all videos)
    eprintln!("🎨 ANSI 모드: 고성능 렌더링");
    run_ansi_mode(
        video_path,
        audio_path,
        mode,
        target_w,
        target_h,
        fill_screen,
    )
}

fn compute_target_dimensions(terminal_w: u32, terminal_h: u32, fill_screen: bool) -> (u32, u32) {
    if fill_screen {
        return (terminal_w.max(1), terminal_h.max(1));
    }

    // Keep visual 16:9 by compensating with actual terminal cell geometry.
    // grid_ratio = aspect * (cell_height / cell_width)
    let cell_aspect = estimate_cell_aspect_ratio();
    let target_ratio = (16.0 / 9.0) * (1.0 / cell_aspect);
    let terminal_ratio = terminal_w as f32 / terminal_h.max(1) as f32;

    let (w, h) = if terminal_ratio > target_ratio {
        // Terminal is wider -> fit to height
        let h = terminal_h;
        let w = (h as f32 * target_ratio) as u32;
        (w, h)
    } else {
        // Terminal is taller -> fit to width
        let w = terminal_w;
        let h = (w as f32 / target_ratio) as u32;
        (w, h)
    };

    (w.max(1), h.max(1))
}

fn estimate_cell_aspect_ratio() -> f32 {
    // width / height of one terminal character cell.
    if let Ok(ws) = crossterm::terminal::window_size() {
        if ws.columns > 0 && ws.rows > 0 && ws.width > 0 && ws.height > 0 {
            let cw = ws.width as f32 / ws.columns as f32;
            let ch = ws.height as f32 / ws.rows as f32;
            if cw > 0.0 && ch > 0.0 {
                return (cw / ch).clamp(0.2, 1.5);
            }
        }
    }

    // Common monospaced terminal cell ratio fallback.
    0.5
}

fn resolve_terminal_size(fill_screen: bool) -> Result<(u32, u32)> {
    let initial = crossterm::terminal::size()?;

    if !fill_screen {
        return Ok((initial.0 as u32, initial.1 as u32));
    }

    // First request includes macOS OS-level automation.
    crate::utils::terminal_control::request_fullscreen(true);

    let start = Instant::now();
    let timeout = Duration::from_secs(8);
    let mut last = initial;
    let mut changed_at = Instant::now();
    let mut changed_once = false;
    let mut last_request = Instant::now() - Duration::from_secs(1);
    let mut max_seen = initial;

    while start.elapsed() < timeout {
        if last_request.elapsed() >= Duration::from_millis(350) {
            // Keep sending terminal-level fullscreen hints during startup.
            crate::utils::terminal_control::request_fullscreen(false);
            last_request = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(60));
        let size = crossterm::terminal::size()?;
        max_seen.0 = max_seen.0.max(size.0);
        max_seen.1 = max_seen.1.max(size.1);

        if size != last {
            last = size;
            changed_at = Instant::now();
            changed_once = true;
            continue;
        }

        // If we've already seen size changes and it has stabilized for a bit, start playback.
        if changed_once
            && changed_at.elapsed() >= Duration::from_millis(850)
            && start.elapsed() >= Duration::from_millis(1500)
        {
            break;
        }

        // If we're already at a large terminal and nothing changes, avoid full timeout.
        if !changed_once && start.elapsed() >= Duration::from_millis(1200) {
            let (cols, rows) = size;
            if cols >= 160 && rows >= 45 {
                break;
            }
        }
    }

    let final_size = crossterm::terminal::size().unwrap_or(last);
    let chosen = if max_seen.0 > final_size.0 && max_seen.1 > final_size.1 {
        max_seen
    } else {
        final_size
    };

    if chosen == initial {
        crate::utils::logger::info(&format!(
            "fullscreen size unchanged: {}x{}",
            chosen.0, chosen.1
        ));
    } else {
        crate::utils::logger::info(&format!(
            "fullscreen size updated: {}x{} -> {}x{} (max_seen={}x{})",
            initial.0, initial.1, chosen.0, chosen.1, max_seen.0, max_seen.1
        ));
    }

    Ok((chosen.0 as u32, chosen.1 as u32))
}

/// ANSI rendering pipeline (optimized for all content)
fn run_ansi_mode(
    video_path: PathBuf,
    audio_path: Option<PathBuf>,
    mode: DisplayMode,
    target_w: u32,
    target_h: u32,
    fill_screen: bool,
) -> Result<()> {
    // Initialize display manager
    let mut display = DisplayManager::new(mode)?;

    // Fullscreen animations can continue resizing after startup.
    // Re-check actual terminal size in the render context and recompute targets.
    let (live_cols, live_rows) = display
        .terminal_size_chars()
        .unwrap_or((target_w as u16, target_h as u16));
    let (target_w, target_h) =
        compute_target_dimensions(live_cols as u32, live_rows as u32, fill_screen);

    // Create video decoder
    // IMPORTANT: We use Half-Block rendering, so vertical resolution is 2x terminal rows
    let pixel_w = target_w;
    let pixel_h = target_h * 2;

    crate::utils::logger::info(&format!(
        "render target resolved: {}x{} (live_term={}x{} fill={})",
        target_w, target_h, live_cols, live_rows, fill_screen
    ));

    let allow_upscale = match mode {
        DisplayMode::Rgb => constants::RGB_ALLOW_UPSCALE,
        DisplayMode::Ascii => constants::ASCII_ALLOW_UPSCALE,
    };

    // Keep source aspect in both modes to avoid perceived over-zoom.
    // fill_screen controls target box size (full terminal vs 16:9 box).
    let decoder_fill_mode = false;

    let decoder = VideoDecoder::new(
        video_path.to_str().unwrap(),
        pixel_w,
        pixel_h,
        decoder_fill_mode,
        allow_upscale,
    )?;

    // Keep queue short to reduce latency/memory pressure on very large terminals.
    let (frame_sender, frame_receiver) =
        crossbeam_channel::bounded(constants::FRAME_QUEUE_CAPACITY);

    // Spawn decoder thread
    let decoder_handle = decoder.spawn_decoding_thread(frame_sender);

    // === SYNC SYSTEM ===
    let _clock = MasterClock::new();

    // Frame processor (expects pixel width and height)
    let processor = FrameProcessor::new(pixel_w as usize, pixel_h as usize);

    // Reusable buffer (pre-allocate with correct size for half-block rendering)
    let term_height = (pixel_h / 2) as usize;
    let mut cell_buffer = vec![CellData::default(); pixel_w as usize * term_height];

    crate::utils::logger::debug(&format!(
        "Initialized cell buffer: {}x{} terminal = {} cells",
        pixel_w,
        term_height,
        cell_buffer.len()
    ));

    // Performance tracking
    let start_time = std::time::Instant::now();
    // CONSUMER LOOP WITH DYNAMIC FRAME SKIP (A/V Sync)
    let mut frame_idx = 0u64;
    let mut frames_dropped = 0u64;

    // Start audio if provided
    let audio_player = if let Some(path) = audio_path {
        Some(AudioPlayer::new(path.to_str().unwrap())?)
    } else {
        None
    };

    crate::utils::logger::debug("Starting render loop");

    let audio_sync_comp = if audio_player.is_some() {
        Duration::from_millis(constants::AUDIO_SYNC_COMP_MS)
    } else {
        Duration::ZERO
    };
    let playback_start = std::time::Instant::now();
    let high_density_rgb = mode == DisplayMode::Rgb
        && target_w.saturating_mul(target_h) >= constants::RGB_HIGH_DENSITY_CELL_COUNT;
    let min_render_interval = if high_density_rgb {
        Some(Duration::from_secs_f64(
            1.0 / constants::RGB_HIGH_DENSITY_MAX_FPS as f64,
        ))
    } else {
        None
    };
    let mut last_render_at = std::time::Instant::now() - Duration::from_secs(1);
    if high_density_rgb {
        crate::utils::logger::info(&format!(
            "rgb high-density mode enabled: cells={} fps_cap={}",
            target_w.saturating_mul(target_h),
            constants::RGB_HIGH_DENSITY_MAX_FPS
        ));
    }

    loop {
        // Input
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }

        // Determine current playback time (audio-compensated when audio is enabled)
        let now = std::time::Instant::now();
        let elapsed = now
            .checked_duration_since(playback_start)
            .unwrap_or(Duration::ZERO);
        let playback_time = elapsed.saturating_sub(audio_sync_comp);
        let queue_depth_before_drain = frame_receiver.len();

        let mut frame_to_render: Option<crate::decoder::FrameData> = None;
        let mut decoder_disconnected = false;

        // Drain queue to find the most recent valid frame
        loop {
            match frame_receiver.try_recv() {
                Ok(frame) => {
                    // If frame is in the future, save it and stop draining
                    if frame.timestamp > playback_time {
                        frame_to_render = Some(frame);
                        break;
                    }

                    // If frame is in the past or present, it's a candidate.
                    // We keep looping to see if there's a newer one.
                    // If we overwrite a previous candidate, that means we dropped a frame.
                    if frame_to_render.is_some() {
                        frames_dropped += 1;
                    }
                    frame_to_render = Some(frame);
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    break;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    decoder_disconnected = true;
                    break;
                }
            }
        }

        // If we found a frame, render it
        if let Some(frame) = frame_to_render {
            // If the frame is WAY in the future (e.g. > 100ms), we should wait?
            // But we already broke the loop if frame.timestamp > playback_time.
            // So frame is either:
            // 1. In the past (we are lagging, render immediately)
            // 2. In the future (we caught up, wait until it's time)

            if frame.timestamp > playback_time {
                let wait_time = frame.timestamp - playback_time;
                if wait_time > Duration::from_millis(1) {
                    std::thread::sleep(wait_time);
                }
            }

            // Adaptive RGB diff threshold: if consumer falls behind, relax tiny color changes
            // to reduce terminal I/O while preserving structural detail.
            if mode == DisplayMode::Rgb {
                display.set_rgb_diff_threshold(adaptive_rgb_threshold(
                    queue_depth_before_drain,
                    high_density_rgb,
                ));
            }

            // On very large RGB surfaces, cap render cadence to keep interaction responsive.
            if let Some(interval) = min_render_interval {
                if last_render_at.elapsed() < interval {
                    frames_dropped += 1;
                    continue;
                }
            }

            // Process frame (TrueColor)
            processor.process_frame_into(&frame.buffer, &mut cell_buffer);

            if let Err(e) = display.render_diff(&cell_buffer, target_w as usize) {
                crate::utils::logger::error(&format!("Render error: {}", e));
                return Err(e);
            }
            last_render_at = std::time::Instant::now();
            frame_idx += 1;
        } else {
            if decoder_disconnected {
                break;
            }
            // No frame available yet, wait for decoder
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    // Cleanup
    crate::utils::logger::debug(&format!(
        "Render loop ended. Frames: {}, Dropped: {}",
        frame_idx, frames_dropped
    ));

    // Drop receiver before join so decoder thread blocked on send can exit.
    drop(frame_receiver);

    // Wait for decoder thread
    let _ = decoder_handle.join();

    // Stop audio
    drop(audio_player);

    let duration = start_time.elapsed();
    println!("\n✅ 재생 완료: (Absolute Timing - Drift-free)");
    println!("   • 렌더링: {} 프레임", frame_idx);
    println!("   • 드롭: {} 프레임", frames_dropped);
    println!("   • 재생 시간: {:.2}초", duration.as_secs_f64());
    println!(
        "   • 평균 FPS: {:.2}",
        frame_idx as f64 / duration.as_secs_f64()
    );

    Ok(())
}

fn adaptive_rgb_threshold(queue_depth: usize, high_density_rgb: bool) -> u8 {
    let base = constants::RGB_COLOR_DELTA_THRESHOLD;
    let max = constants::RGB_COLOR_DELTA_THRESHOLD_MAX;

    let mut threshold = if queue_depth <= constants::RGB_ADAPTIVE_THRESHOLD_QUEUE_START {
        base
    } else {
        let start = constants::RGB_ADAPTIVE_THRESHOLD_QUEUE_START as f32;
        let full = constants::RGB_ADAPTIVE_THRESHOLD_QUEUE_FULL as f32;
        let depth = queue_depth as f32;
        let t = ((depth - start) / (full - start).max(1.0)).clamp(0.0, 1.0);
        let span = (max.saturating_sub(base)) as f32;
        (base as f32 + span * t).round() as u8
    };

    if high_density_rgb {
        threshold = threshold.max(base.saturating_add(2));
    }

    threshold.min(max)
}
