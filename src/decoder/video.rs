use super::frame_data::FrameData;
use crate::shared::constants;
use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use fast_image_resize as fr;
use fr::images::Image;
use opencv::{core, imgproc, prelude::*, videoio};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderTarget {
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl RenderTarget {
    pub fn new(pixel_width: u32, pixel_height: u32) -> Self {
        Self {
            pixel_width: pixel_width.max(1),
            pixel_height: pixel_height.max(2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    CropToFill,
    Fit,
}

pub struct VideoDecoder {
    capture: videoio::VideoCapture,
    fps: f64,
    scale_mode: ScaleMode,
    target: Arc<RwLock<RenderTarget>>,
}

impl VideoDecoder {
    pub fn new(
        path: &str,
        target: Arc<RwLock<RenderTarget>>,
        scale_mode: ScaleMode,
    ) -> Result<Self> {
        let mut log_path = std::env::current_dir()?;
        log_path.push(constants::DEBUG_LOG_FILE);

        let mut log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)?;

        writeln!(log_file, "=== OpenCV Video Decoder Initialization ===")?;
        writeln!(log_file, "Video: {}", path)?;

        let mut capture = videoio::VideoCapture::from_file(path, videoio::CAP_ANY)?;
        let _ = capture.set(
            videoio::CAP_PROP_HW_ACCELERATION,
            videoio::VIDEO_ACCELERATION_ANY as f64,
        );

        if !capture.is_opened()? {
            let err_msg = format!("Failed to open video file: {}", path);
            writeln!(log_file, "ERROR: {}", err_msg)?;
            return Err(anyhow!(err_msg));
        }

        let fps = capture.get(videoio::CAP_PROP_FPS)?;
        let fps = if fps.is_finite() && fps > 0.0 { fps } else { 30.0 };
        let orig_width = capture.get(videoio::CAP_PROP_FRAME_WIDTH)? as u32;
        let orig_height = capture.get(videoio::CAP_PROP_FRAME_HEIGHT)? as u32;

        writeln!(log_file, "SUCCESS: OpenCV VideoCapture opened")?;
        writeln!(log_file, "  Original: {}x{}", orig_width, orig_height)?;
        writeln!(log_file, "  FPS: {}", fps)?;

        Ok(Self {
            capture,
            fps,
            scale_mode,
            target,
        })
    }

    pub fn get_fps(&self) -> f64 {
        self.fps
    }

    pub fn spawn_decoding_thread(
        mut self,
        sender: Sender<FrameData>,
        playback_fps: f64,
    ) -> std::thread::JoinHandle<Result<()>> {
        std::thread::spawn(move || {
            crate::utils::logger::debug("Decoder thread started");
            let mut frame_counter: u64 = 0;

            loop {
                let mut buffer = Vec::new();
                match self.read_frame_into(&mut buffer) {
                    Ok(Some(target)) => {
                        let timestamp =
                            std::time::Duration::from_secs_f64(frame_counter as f64 / playback_fps);
                        frame_counter += 1;

                        let frame = FrameData::new(
                            buffer,
                            target.pixel_width,
                            target.pixel_height,
                            timestamp,
                        );
                        if sender.send(frame).is_err() {
                            crate::utils::logger::debug("Decoder sender error (receiver dropped)");
                            break;
                        }
                    }
                    Ok(None) => {
                        crate::utils::logger::debug("Decoder EOF");
                        break;
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

    pub fn read_frame_into(&mut self, buffer: &mut Vec<u8>) -> Result<Option<RenderTarget>> {
        let start_total = std::time::Instant::now();
        let mut frame = Mat::default();

        let start_decode = std::time::Instant::now();
        if !self.capture.read(&mut frame)? {
            return Ok(None);
        }
        let decode_time = start_decode.elapsed();

        if frame.empty() {
            return Ok(None);
        }

        let target = *self
            .target
            .read()
            .map_err(|_| anyhow!("render target lock poisoned"))?;

        let start_resize = std::time::Instant::now();
        let orig_w = frame.cols() as u32;
        let orig_h = frame.rows() as u32;

        let scale_w = target.pixel_width as f64 / orig_w as f64;
        let scale_h = target.pixel_height as f64 / orig_h as f64;
        let scale = match self.scale_mode {
            ScaleMode::CropToFill => scale_w.max(scale_h),
            ScaleMode::Fit => scale_w.min(scale_h),
        };
        let new_w = ((orig_w as f64 * scale).round() as u32).max(1);
        let new_h = ((orig_h as f64 * scale).round() as u32).max(1);

        let mut rgb_opencv = Mat::default();
        #[cfg(target_os = "macos")]
        imgproc::cvt_color(
            &frame,
            &mut rgb_opencv,
            imgproc::COLOR_BGR2RGB,
            0,
            core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        #[cfg(not(target_os = "macos"))]
        imgproc::cvt_color(&frame, &mut rgb_opencv, imgproc::COLOR_BGR2RGB, 0)?;

        if !rgb_opencv.is_continuous() {
            return Err(anyhow!("Frame is not continuous"));
        }
        let rgb_bytes = rgb_opencv.data_bytes()?;
        let src_image =
            Image::from_vec_u8(orig_w, orig_h, rgb_bytes.to_vec(), fr::PixelType::U8x3)?;
        let mut dst_image = Image::new(new_w, new_h, fr::PixelType::U8x3);
        let mut resizer = fr::Resizer::new();
        resizer.resize(&src_image, &mut dst_image, None)?;
        let resize_time = start_resize.elapsed();

        let start_letterbox = std::time::Instant::now();
        let mut canvas =
            vec![0u8; (target.pixel_width * target.pixel_height * 3) as usize];

        if new_w > target.pixel_width || new_h > target.pixel_height {
            let crop_x = ((new_w - target.pixel_width) / 2) as usize;
            let crop_y = ((new_h - target.pixel_height) / 2) as usize;

            for y in 0..target.pixel_height {
                let src_y = crop_y + y as usize;
                let src_offset = (src_y * new_w as usize + crop_x) * 3;
                let dst_offset = (y * target.pixel_width) as usize * 3;
                let copy_len =
                    (target.pixel_width as usize * 3).min(dst_image.buffer().len() - src_offset);

                if src_offset + copy_len <= dst_image.buffer().len()
                    && dst_offset + copy_len <= canvas.len()
                {
                    canvas[dst_offset..dst_offset + copy_len]
                        .copy_from_slice(&dst_image.buffer()[src_offset..src_offset + copy_len]);
                }
            }
        } else {
            let x_off = ((target.pixel_width - new_w) / 2) as usize;
            let y_off = ((target.pixel_height - new_h) / 2) as usize;

            for y in 0..new_h {
                let src_offset = (y * new_w) as usize * 3;
                let dst_y = y_off + y as usize;
                let dst_offset = (dst_y * target.pixel_width as usize + x_off) * 3;
                let copy_len = (new_w as usize * 3).min(dst_image.buffer().len() - src_offset);

                if src_offset + copy_len <= dst_image.buffer().len()
                    && dst_offset + copy_len <= canvas.len()
                {
                    canvas[dst_offset..dst_offset + copy_len]
                        .copy_from_slice(&dst_image.buffer()[src_offset..src_offset + copy_len]);
                }
            }
        }

        let letterbox_time = start_letterbox.elapsed();

        buffer.clear();
        buffer.extend_from_slice(&canvas);

        let total_time = start_total.elapsed();
        if total_time.as_millis() > 10 {
            let mut log_path = std::env::current_dir().unwrap_or_default();
            log_path.push(constants::DEBUG_LOG_FILE);

            if let Ok(mut file) = OpenOptions::new().append(true).open(log_path) {
                let _ = writeln!(
                    file,
                    "SIMD_FRAME: Total={}us | Decode={}us | Resize={}us | Letterbox={}us",
                    total_time.as_micros(),
                    decode_time.as_micros(),
                    resize_time.as_micros(),
                    letterbox_time.as_micros()
                );
            }
        }

        Ok(Some(target))
    }
}
