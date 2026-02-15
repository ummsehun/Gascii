use anyhow::{Result, anyhow};
use opencv::{
    prelude::*,
    videoio,
    imgproc,
    core,
};
use std::fs::OpenOptions;
use std::io::Write;
use crossbeam_channel::Sender;
use super::frame_data::FrameData;
use fast_image_resize as fr;
use fr::images::Image;

pub struct VideoDecoder {
    capture: videoio::VideoCapture,
    width: u32,
    height: u32,
    fps: f64,
    fill_mode: bool,
}

impl VideoDecoder {
    pub fn new(path: &str, width: u32, height: u32, fill_mode: bool) -> Result<Self> {
        // Setup logging with absolute path
        let mut log_path = std::env::current_dir()?;
        log_path.push("debug.log");
        
        let mut log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)?;
        
        writeln!(log_file, "=== OpenCV Video Decoder Initialization ===")?;
        writeln!(log_file, "Video: {}", path)?;
        writeln!(log_file, "Target Resolution: {}x{}", width, height)?;
        
        writeln!(log_file, "DEBUG: Opening video with OpenCV...")?;
        
        // CAP_ANY allows OpenCV to choose the best backend
        // macOS: AVFoundation (VideoToolbox GPU decode)
        // Windows: Media Foundation (GPU decode)
        // Linux: V4L2/GStreamer
        let mut capture = videoio::VideoCapture::from_file(path, videoio::CAP_ANY)?;
        
        // Try to enforce HW acceleration
        // Note: This might not work on all backends/platforms, but it's worth setting
        let _ = capture.set(videoio::CAP_PROP_HW_ACCELERATION, videoio::VIDEO_ACCELERATION_ANY as f64);
        
        if !capture.is_opened()? {
            let err_msg = format!("Failed to open video file: {}", path);
            writeln!(log_file, "ERROR: {}", err_msg)?;
            return Err(anyhow!(err_msg));
        }

        let fps = capture.get(videoio::CAP_PROP_FPS)?;
        let orig_width = capture.get(videoio::CAP_PROP_FRAME_WIDTH)? as u32;
        let orig_height = capture.get(videoio::CAP_PROP_FRAME_HEIGHT)? as u32;
        
        writeln!(log_file, "SUCCESS: OpenCV VideoCapture opened")?;
        writeln!(log_file, "  Original: {}x{}", orig_width, orig_height)?;
        writeln!(log_file, "  FPS: {}", fps)?;
        writeln!(log_file, "  Backend: AVFoundation (GPU decode)")?;
        writeln!(log_file, "=========================")?;
        
        println!("DEBUG: OpenCV VideoCapture opened successfully. Detected FPS: {}", fps);

        Ok(Self {
            capture,
            width,
            height,
            fps,
            fill_mode,
        })
    }

    pub fn get_fps(&self) -> f64 {
        self.fps
    }

    pub fn spawn_decoding_thread(mut self, sender: Sender<FrameData>) -> std::thread::JoinHandle<Result<()>> {
        std::thread::spawn(move || {
            crate::utils::logger::debug("Decoder thread started");
            let mut frame_counter: u64 = 0;
            loop {
                let mut buffer = Vec::new();
                match self.read_frame_into(&mut buffer) {
                    Ok(true) => {
                        // Calculate timestamp based on frame count and FPS
                        let timestamp = std::time::Duration::from_secs_f64(frame_counter as f64 / self.fps);
                        frame_counter += 1;

                        let frame = FrameData {
                            buffer,
                            width: self.width,
                            height: self.height,
                            timestamp,
                        };
                        if sender.send(frame).is_err() {
                            crate::utils::logger::debug("Decoder sender error (receiver dropped)");
                            break; // Receiver dropped
                        }
                    }
                    Ok(false) => {
                        crate::utils::logger::debug("Decoder EOF");
                        break; // EOF
                    }
                    Err(e) => {
                        crate::utils::logger::error(&format!("Decoding error: {}", e));
                        break;
                    }
                }
            }
            crate::utils::logger::debug("Decoder thread exiting");
            Ok(())
        })
    }

    pub fn read_frame(&mut self) -> Result<Option<Vec<u8>>> {
        let mut buffer = Vec::new();
        if self.read_frame_into(&mut buffer)? {
            Ok(Some(buffer))
        } else {
            Ok(None)
        }
    }

    pub fn read_frame_into(&mut self, buffer: &mut Vec<u8>) -> Result<bool> {
        let start_total = std::time::Instant::now();
        let mut frame = Mat::default();
        
        // 1. Decode (GPU/CPU)
        let start_decode = std::time::Instant::now();
        if !self.capture.read(&mut frame)? {
            return Ok(false); // EOF
        }
        let decode_time = start_decode.elapsed();
        
        if frame.empty() {
            return Ok(false);
        }

        // 2. SIMD-optimized Resize with fast_image_resize
        let start_resize = std::time::Instant::now();
        
        let orig_w = frame.cols() as u32;
        let orig_h = frame.rows() as u32;
        
        // Calculate aspect ratio preserving dimensions
        let scale_w = self.width as f64 / orig_w as f64;
        let scale_h = self.height as f64 / orig_h as f64;
        let scale = if self.fill_mode { scale_w.max(scale_h) } else { scale_w.min(scale_h) };
        let new_w = ((orig_w as f64 * scale).round() as u32).max(1);
        let new_h = ((orig_h as f64 * scale).round() as u32).max(1);
        
        // Convert OpenCV Mat (BGR) to fast_image_resize Image (RGB24)
        // First convert BGR to RGB
        let mut rgb_opencv = Mat::default();
        #[cfg(target_os = "macos")]
        imgproc::cvt_color(&frame, &mut rgb_opencv, imgproc::COLOR_BGR2RGB, 0, core::AlgorithmHint::ALGO_HINT_DEFAULT)?;
        
        #[cfg(not(target_os = "macos"))]
        imgproc::cvt_color(&frame, &mut rgb_opencv, imgproc::COLOR_BGR2RGB, 0)?;
        
        // Get raw bytes
        if !rgb_opencv.is_continuous() {
            return Err(anyhow!("Frame is not continuous"));
        }
        let rgb_bytes = rgb_opencv.data_bytes()?;
        
        // Create source image
        let src_image = Image::from_vec_u8(
            orig_w,
            orig_h,
            rgb_bytes.to_vec(),
            fr::PixelType::U8x3,
        )?;
        
        // Create destination image
        let mut dst_image = Image::new(
            new_w,
            new_h,
            fr::PixelType::U8x3,
        );
        
        // Create resizer (uses SIMD when available)
        let mut resizer = fr::Resizer::new();
        resizer.resize(&src_image, &mut dst_image, None)?;
        
        let resize_time = start_resize.elapsed();

        // 3. Letterbox/Crop to exact target dimensions
        let start_letterbox = std::time::Instant::now();
        
        let mut canvas = vec![0u8; (self.width * self.height * 3) as usize];
        
        if new_w > self.width || new_h > self.height {
            // Crop center
            let crop_x = ((new_w - self.width) / 2) as usize;
            let crop_y = ((new_h - self.height) / 2) as usize;
            
            for y in 0..self.height {
                let src_y = crop_y + y as usize;
                let src_offset = (src_y * new_w as usize + crop_x) * 3;
                let dst_offset = (y * self.width) as usize * 3;
                let copy_len = (self.width as usize * 3).min(dst_image.buffer().len() - src_offset);
                
                if src_offset + copy_len <= dst_image.buffer().len() 
                    && dst_offset + copy_len <= canvas.len() {
                    canvas[dst_offset..dst_offset + copy_len]
                        .copy_from_slice(&dst_image.buffer()[src_offset..src_offset + copy_len]);
                }
            }
        } else {
            // Letterbox (center)
            let x_off = ((self.width - new_w) / 2) as usize;
            let y_off = ((self.height - new_h) / 2) as usize;
            
            for y in 0..new_h {
                let src_offset = (y * new_w) as usize * 3;
                let dst_y = y_off + y as usize;
                let dst_offset = (dst_y * self.width as usize + x_off) * 3;
                let copy_len = (new_w as usize * 3).min(dst_image.buffer().len() - src_offset);
                
                if src_offset + copy_len <= dst_image.buffer().len()
                    && dst_offset + copy_len <= canvas.len() {
                    canvas[dst_offset..dst_offset + copy_len]
                        .copy_from_slice(&dst_image.buffer()[src_offset..src_offset + copy_len]);
                }
            }
        }
        
        let letterbox_time = start_letterbox.elapsed();
        
        // Return the canvas buffer
        buffer.clear();
        buffer.extend_from_slice(&canvas);
        
        let total_time = start_total.elapsed();

        // Log slow frames (> 10ms) to debug.log
        if total_time.as_millis() > 10 {
            let mut log_path = std::env::current_dir().unwrap_or_default();
            log_path.push("debug.log");

            if let Ok(mut file) = OpenOptions::new().append(true).open(log_path) {
                let _ = writeln!(file, "SIMD_FRAME: Total={}us | Decode={}us | Resize={}us | Letterbox={}us", 
                    total_time.as_micros(),
                    decode_time.as_micros(),
                    resize_time.as_micros(),
                    letterbox_time.as_micros()
                );
            }
        }

        Ok(true)
    }
}