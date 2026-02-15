use anyhow::Result;
use std::path::PathBuf;
use crossterm::{
    event::{self, Event, KeyCode},
};
use std::time::Duration;

// Direct module imports
use crate::renderer::{DisplayManager, DisplayMode, FrameProcessor};
use crate::renderer::cell::CellData;
use crate::decoder::VideoDecoder;
use crate::audio::AudioPlayer;
use crate::sync::MasterClock;

/// 디버그 로그 파일에 메시지를 기록합니다.

pub fn run_game(
    video_path: PathBuf,
    audio_path: Option<PathBuf>,
    mode: DisplayMode,
    fill_screen: bool
) -> Result<()> {
    // 1. Terminal Setup
    let (terminal_w, terminal_h) = {
        let size = crossterm::terminal::size()?;
        (size.0 as u32, size.1 as u32)
    };

    // Calculate target dimensions
    // We want 16:9 aspect ratio if not filling screen
    let (target_w, target_h) = if fill_screen {
        (terminal_w, terminal_h)
    } else {
        // Visual 16:9 aspect ratio
        // Since we use Half-Block rendering (1 char = 2 vertical pixels),
        // the cell aspect ratio is effectively 1:2.
        // To achieve visual 16:9, the grid ratio must be 16:4.5 = 32:9 ≈ 3.55
        let target_ratio = 32.0 / 9.0;
        let terminal_ratio = terminal_w as f32 / terminal_h as f32;
        
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
        (w.saturating_sub(2), h)
    };

    println!("\n🚀 재생 시작: {} ({}x{} 픽셀, {})", 
        video_path.file_name().unwrap().to_string_lossy(),
        target_w, target_h,
        if fill_screen { "전체화면" } else { "16:9" }
    );

    // === START PRODUCER-CONSUMER IMPLEMENTATION WITH SYNC ===
    
    // Run ANSI rendering (optimized for all videos)
    eprintln!("🎨 ANSI 모드: 고성능 렌더링");
    run_ansi_mode(video_path, audio_path, mode, target_w, target_h, fill_screen)
}

/// ANSI rendering pipeline (optimized for all content)
fn run_ansi_mode(
    video_path: PathBuf,
    audio_path: Option<PathBuf>,
    mode: DisplayMode,
    target_w: u32,
    target_h: u32,
    fill_screen: bool
) -> Result<()> {
    // Initialize display manager
    let mut display = DisplayManager::new(mode)?;

    // Create video decoder
    // IMPORTANT: We use Half-Block rendering, so vertical resolution is 2x terminal rows
    let pixel_w = target_w;
    let pixel_h = target_h * 2;
    
    let decoder = VideoDecoder::new(
        video_path.to_str().unwrap(),
        pixel_w,
        pixel_h,
        fill_screen
    )?;
    
    let fps = decoder.get_fps();
    
    // Create bounded channel (120 frames = ~4-5 seconds buffer)
    let (frame_sender, frame_receiver) = crossbeam_channel::bounded(120);
    
    // Spawn decoder thread
    let decoder_handle = decoder.spawn_decoding_thread(frame_sender);

    // === SYNC SYSTEM ===
    let _clock = MasterClock::new();
    
    // Frame processor (expects pixel width and height)
    let processor = FrameProcessor::new(pixel_w as usize, pixel_h as usize);
    
    // Reusable buffer (pre-allocate with correct size for half-block rendering)
    let term_height = (pixel_h / 2) as usize;
    let mut cell_buffer = vec![CellData::default(); pixel_w as usize * term_height];
    
    crate::utils::logger::debug(&format!("Initialized cell buffer: {}x{} terminal = {} cells", 
        pixel_w, term_height, cell_buffer.len()));
    
    // Performance tracking
    let start_time = std::time::Instant::now();
    let frame_duration = std::time::Duration::from_secs_f64(1.0 / fps);
    
    // Adaptive frame skip with EWMA
    let _avg_frame_time = frame_duration;
    const EWMA_ALPHA: f64 = 0.3; // Weight for new samples
    
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
    
    // Video start time (will be set on first frame)
    let mut video_start_time: Option<std::time::Instant> = None;

    loop {
        // Input
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }
        
        // Determine current playback time
        let now = std::time::Instant::now();
        let playback_time = if let Some(start) = video_start_time {
            now.duration_since(start)
        } else {
            Duration::ZERO
        };

        let mut frame_to_render: Option<crate::decoder::FrameData> = None;

        // Drain queue to find the most recent valid frame
        loop {
            match frame_receiver.try_recv() {
                Ok(frame) => {
                    // If this is the very first frame, start the clock
                    if video_start_time.is_none() {
                        video_start_time = Some(now);
                    }

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
                    // Decoder finished
                    // If we have a frame pending, render it, then exit
                    break;
                }
            }
        }

        // Check if disconnected and empty
        if frame_to_render.is_none() && frame_receiver.is_empty() && frame_receiver.len() == 0 {
             // Check if channel is actually disconnected
             if let Err(crossbeam_channel::TryRecvError::Disconnected) = frame_receiver.try_recv() {
                  break;
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

            // Process frame (TrueColor)
            processor.process_frame_into(&frame.buffer, &mut cell_buffer);
            
            if let Err(e) = display.render_diff(&cell_buffer, target_w as usize) {
                crate::utils::logger::error(&format!("Render error: {}", e));
                return Err(e);
            }
            frame_idx += 1;
        } else {
            // No frame available yet, or we are waiting for buffer
            // Check if disconnected
            if let Err(crossbeam_channel::TryRecvError::Disconnected) = frame_receiver.try_recv() {
                 break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    
    // Cleanup
    crate::utils::logger::debug(&format!("Render loop ended. Frames: {}, Dropped: {}", frame_idx, frames_dropped));
    
    // Wait for decoder thread
    let _ = decoder_handle.join();
    
    // Stop audio
    drop(audio_player);
    
    let duration = start_time.elapsed();
    println!("\n✅ 재생 완료: (Absolute Timing - Drift-free)");
    println!("   • 렌더링: {} 프레임", frame_idx);
    println!("   • 드롭: {} 프레임", frames_dropped);
    println!("   • 재생 시간: {:.2}초", duration.as_secs_f64());
    println!("   • 평균 FPS: {:.2}", frame_idx as f64 / duration.as_secs_f64());

    Ok(())
}