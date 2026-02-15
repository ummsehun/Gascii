use anyhow::{Result, Context};
use crossterm::terminal;
use std::time::{Duration, Instant};
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::renderer::{DisplayManager, DisplayMode, FrameProcessor};
use crate::decoder::VideoDecoder;
use crate::core::audio_manager::AudioManager;
use crate::core::frame_buffer::FrameBuffer;

pub fn play_realtime(
    video_path: &str,
    audio_path: Option<&str>,
    width: u32,
    height: u32,
    fps: u32,
    mode: DisplayMode,
    fill: bool,
) -> Result<()> {
    // 1. Initialize Display & Audio
    let mut display = DisplayManager::new(mode)?;
    let audio = AudioManager::new()?;

    // Keep requested values mutable for possible clamping
    let mut req_width = width;
    let mut req_height = height;

    // 2. Start Audio
    if let Some(path) = audio_path {
        audio.play(path)?;
    }

    // 2.1 Ensure width/height fit in the terminal; clamp to Display size
    {
        let (term_cols, term_rows) = display.terminal_size_chars()?;
        let max_img_w = term_cols as u32;
        let max_img_h = (term_rows as u32) * 2;
        if req_width > max_img_w || req_height > max_img_h {
            // Compute scale to fit
            let scale_w = max_img_w as f64 / width as f64;
            let scale_h = max_img_h as f64 / height as f64;
            let scale = scale_w.min(scale_h);
            let new_w = (req_width as f64 * scale).floor() as u32;
            let new_h = (req_height as f64 * scale).floor() as u32;
            eprintln!("⚠️  Requested video size {}x{} is larger than terminal max {}x{}: scaling to {}x{}",
                      width, height, max_img_w, max_img_h, new_w, new_h);
            // Replace width/height
            // NOTE: For safety we set to max 1
            req_width = new_w.max(1);
            req_height = new_h.max(1);
            // We'll shadow the local vars by using req_width/req_height for the rest of the function
        }
    }

    // 3. Start Video Decoder
    // println!("Initializing video decoder with target: {}x{}... fill={}", req_width, req_height, fill);
    let mut decoder = VideoDecoder::new(video_path, req_width, req_height, fill)?;
    let actual_fps = decoder.get_fps();
    // println!("Video decoder started. Detected FPS: {:.2}", actual_fps);
    
    // 4. Initialize Frame Processor (Rayon)
    let processor = FrameProcessor::new(req_width as usize, req_height as usize);

    // 5. Create Ring Buffer (2 seconds)
    let buffer_capacity = (actual_fps * 2.0) as usize;
    let frame_buffer = FrameBuffer::new(buffer_capacity);
    let queue = frame_buffer.clone_queue();

    // 6. Spawn OpenCV Reader Thread (Producer)
    let running_reader = Arc::new(AtomicBool::new(true));
    let r_clone = running_reader.clone();
    
    let reader_handle = thread::spawn(move || {
        let mut frames_read = 0u64;
        
        while r_clone.load(Ordering::SeqCst) {
            match decoder.read_frame() {
                Ok(Some(buffer)) => {
                    // BLOCKING push: Wait until buffer has space without cloning frames.
                    let mut buf = buffer;
                    loop {
                        match queue.push(buf) {
                            Ok(()) => break,
                            Err(returned_buf) => {
                                // Buffer full - wait for consumer to catch up
                                buf = returned_buf;
                                thread::sleep(Duration::from_micros(100));
                                // Check if we should exit
                                if !r_clone.load(Ordering::SeqCst) {
                                    return;
                                }
                            }
                        }
                    }
                    frames_read += 1;
                }
                Ok(None) => {
                    // EOF
                    println!("OpenCV reader: End of video");
                    break;
                }
                Err(e) => {
                    eprintln!("Error reading frame: {}", e);
                    break;
                }
            }
        }
        
        println!("OpenCV reader thread exited. Frames read: {}", frames_read);
    });

    // 7. Main Playback Loop (Consumer)
    // Wait briefly for buffer to fill
    thread::sleep(Duration::from_millis(200));
    
    // Warn if FPS mismatch
    // Warn if FPS mismatch (only if user explicitly requested a specific FPS)
    if fps > 0 && (actual_fps - fps as f64).abs() > 0.5 {
        // println!("⚠️  FPS MISMATCH DETECTED:");
        // println!("   User requested: {}fps", fps);
        // println!("   Video actual:   {:.2}fps", actual_fps);
        // println!("   Using actual video FPS for sync");
    } else if fps == 0 {
        // println!("ℹ️  Auto-detected Video FPS: {:.2}", actual_fps);
    }
    
    let frame_duration = Duration::from_secs_f64(1.0 / actual_fps);
    let start_time = Instant::now();
    let mut frame_idx = 0u64;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    // Performance metrics
    let mut last_fps_report = Instant::now();
    let mut frames_since_report = 0;
    
    // Precision timing tracking
    let mut _last_frame_time = Instant::now();
    let mut cumulative_drift = Duration::ZERO;
    let mut max_drift = Duration::ZERO;
    let mut total_sleep_time = Duration::ZERO;

    while running.load(Ordering::SeqCst) {
        // Input polling
        if crossterm::event::poll(Duration::from_millis(0))? {
             if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                 if key.code == crossterm::event::KeyCode::Char('q') {
                     break;
                 }
             }
         }

        // Try to get frame from buffer (non-blocking)
        if let Some(buffer) = frame_buffer.pop() {
            // ========== PRECISION TIMING SYSTEM ==========
            // Calculate target time for this frame (nanosecond precision)
            let target_time = start_time + frame_duration * (frame_idx as u32);
            let now = Instant::now();
            
            // Calculate drift (how far off we are from ideal timing)
            let drift = if now < target_time {
                // We're ahead - need to sleep
                target_time.duration_since(now)
            } else {
                // We're behind - no sleep, just track drift
                Duration::ZERO
            };
            
            // Track maximum drift for diagnostics
            if drift > max_drift {
                max_drift = drift;
            }
            cumulative_drift += drift;
            
            // ADAPTIVE SLEEP: Only sleep if drift is significant (>100μs)
            // This prevents sleeping for tiny amounts which is inaccurate
            if drift > Duration::from_micros(100) {
                thread::sleep(drift);
                total_sleep_time += drift;
            }
            
            // Record actual frame time
            let frame_start = Instant::now();

            // Render
            match mode {
                DisplayMode::Rgb => {
                    // 1. Process Frame (Parallel Quantization)
                    let cells = processor.process_frame(&buffer);
                    // 2. Render Diff (Optimized Output)
                    display.render_diff(&cells, width as usize)?;
                },
                DisplayMode::Ascii => {
                    // ASCII mode disabled
                },
            }

            let frame_end = Instant::now();
            let frame_render_time = frame_end.duration_since(frame_start);
            
            // Track frame timing
            _last_frame_time = frame_end;
            frame_idx += 1;
            frames_since_report += 1;

                // Report FPS and timing metrics every 2 seconds
                if last_fps_report.elapsed() >= Duration::from_secs(2) {
                    /*
                    let elapsed = last_fps_report.elapsed().as_secs_f64();
                    let fps_actual = frames_since_report as f64 / elapsed;
                    let buffer_fill = frame_buffer.fill_level();
                    let avg_drift = cumulative_drift.as_micros() / frames_since_report as u128;
                    let avg_render = frame_render_time.as_micros();
                    
                    println!("FPS: {:.1}/{:.1} | Buffer: {:.0}% | Drift: {}μs (max: {}μs) | Render: {}μs | Frame: {}", 
                             fps_actual, actual_fps, 
                             buffer_fill * 100.0, 
                             avg_drift,
                             max_drift.as_micros(),
                             avg_render,
                             frame_idx);
                    */
                    last_fps_report = Instant::now();
                    frames_since_report = 0;
                    cumulative_drift = Duration::ZERO;
                    max_drift = Duration::ZERO;
                }
        } else {
            // Buffer empty - wait briefly
            thread::sleep(Duration::from_micros(500));
        }
    }

    // 8. Cleanup
    running_reader.store(false, Ordering::SeqCst);
    reader_handle.join().ok();

    let total_time = start_time.elapsed();
    let expected_time = frame_duration * (frame_idx as u32);
    let final_drift = if total_time > expected_time {
        total_time - expected_time
    } else {
        expected_time - total_time
    };
    
    println!("\n=== Playback Complete ===");
    println!("Total frames: {}", frame_idx);
    println!("Total time: {:.2}s", total_time.as_secs_f64());
    println!("Expected time: {:.2}s", expected_time.as_secs_f64());
    println!("Final drift: {:.3}s ({:.1}%)", 
             final_drift.as_secs_f64(),
             (final_drift.as_secs_f64() / expected_time.as_secs_f64()) * 100.0);
    
    Ok(())
}
