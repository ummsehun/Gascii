use super::frame_data::FrameData;
use anyhow::{anyhow, Result};
use crossbeam_channel::Sender;
use fast_image_resize as fr;
use fr::images::{Image, ImageRef};
use opencv::{core, imgproc, prelude::*, videoio};
use std::time::{Duration, Instant};

pub struct VideoDecoder {
    capture: videoio::VideoCapture,
    width: u32,
    height: u32,
    fps: f64,
    fill_mode: bool,
    allow_upscale: bool,
    decode_mat: Mat,
    rgb_mat: Mat,
    resizer: fr::Resizer,
    resized_buffer: Vec<u8>,
    perf_window_start: Instant,
    perf_frames: u64,
    perf_total_us: u128,
    perf_decode_us: u128,
    perf_resize_us: u128,
    perf_compose_us: u128,
}

impl VideoDecoder {
    pub fn new(
        path: &str,
        width: u32,
        height: u32,
        fill_mode: bool,
        allow_upscale: bool,
    ) -> Result<Self> {
        let mut capture = videoio::VideoCapture::default()?;

        // Open with HW-accel related parameters first, then fallback if backend rejects them.
        let mut params = core::Vector::<i32>::new();
        params.push(videoio::CAP_PROP_HW_ACCELERATION);
        params.push(videoio::VIDEO_ACCELERATION_ANY);

        let opened_with_params = capture
            .open_file_with_params(path, videoio::CAP_ANY, &params)
            .unwrap_or(false);
        if !opened_with_params {
            capture = videoio::VideoCapture::from_file(path, videoio::CAP_ANY)?;
        }

        if !capture.is_opened()? {
            return Err(anyhow!("Failed to open video file: {}", path));
        }

        let fps_raw = capture.get(videoio::CAP_PROP_FPS)?;
        let fps = if fps_raw.is_finite() && fps_raw >= 1.0 {
            fps_raw
        } else {
            30.0
        };
        let orig_width = capture.get(videoio::CAP_PROP_FRAME_WIDTH)? as u32;
        let orig_height = capture.get(videoio::CAP_PROP_FRAME_HEIGHT)? as u32;
        let backend = capture
            .get_backend_name()
            .unwrap_or_else(|_| "unknown".to_string());

        crate::utils::logger::info(&format!(
            "decoder init: video={} src={}x{} target={}x{} fps={:.3} backend={} hw_params={}",
            path, orig_width, orig_height, width, height, fps, backend, opened_with_params
        ));

        Ok(Self {
            capture,
            width,
            height,
            fps,
            fill_mode,
            allow_upscale,
            decode_mat: Mat::default(),
            rgb_mat: Mat::default(),
            resizer: fr::Resizer::new(),
            resized_buffer: Vec::new(),
            perf_window_start: Instant::now(),
            perf_frames: 0,
            perf_total_us: 0,
            perf_decode_us: 0,
            perf_resize_us: 0,
            perf_compose_us: 0,
        })
    }

    pub fn get_fps(&self) -> f64 {
        self.fps
    }

    pub fn spawn_decoding_thread(
        mut self,
        sender: Sender<FrameData>,
    ) -> std::thread::JoinHandle<Result<()>> {
        std::thread::spawn(move || {
            crate::utils::logger::debug("Decoder thread started");
            let mut frame_counter: u64 = 0;
            let mut decode_buffer = Vec::new();

            loop {
                match self.read_frame_into(&mut decode_buffer) {
                    Ok(true) => {
                        let timestamp = Duration::from_secs_f64(frame_counter as f64 / self.fps);
                        frame_counter += 1;

                        let frame = FrameData {
                            buffer: std::mem::take(&mut decode_buffer),
                            width: self.width,
                            height: self.height,
                            timestamp,
                        };

                        match sender.send(frame) {
                            Ok(()) => {}
                            Err(err) => {
                                decode_buffer = err.0.buffer;
                                crate::utils::logger::debug(
                                    "Decoder sender disconnected (receiver dropped)",
                                );
                                break;
                            }
                        }
                    }
                    Ok(false) => {
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

    pub fn read_frame(&mut self) -> Result<Option<Vec<u8>>> {
        let mut buffer = Vec::new();
        if self.read_frame_into(&mut buffer)? {
            Ok(Some(buffer))
        } else {
            Ok(None)
        }
    }

    pub fn read_frame_into(&mut self, buffer: &mut Vec<u8>) -> Result<bool> {
        let start_total = Instant::now();

        let start_decode = Instant::now();
        if !self.capture.read(&mut self.decode_mat)? {
            return Ok(false);
        }
        let decode_time = start_decode.elapsed();

        if self.decode_mat.empty() {
            return Ok(false);
        }

        let start_resize = Instant::now();
        let orig_w = self.decode_mat.cols() as u32;
        let orig_h = self.decode_mat.rows() as u32;

        let scale_w = self.width as f64 / orig_w as f64;
        let scale_h = self.height as f64 / orig_h as f64;
        let mut scale = if self.fill_mode {
            scale_w.max(scale_h)
        } else {
            scale_w.min(scale_h)
        };
        if scale > 1.0 {
            if self.allow_upscale {
                scale = scale.min(crate::shared::constants::MAX_UPSCALE_FACTOR);
            } else {
                scale = 1.0;
            }
        }

        let resize_alg = if scale < 1.0 {
            // Hamming gives near-bicubic downscale quality at bilinear-class speed.
            fr::ResizeAlg::Convolution(fr::FilterType::Hamming)
        } else if scale <= 1.2 {
            fr::ResizeAlg::Convolution(fr::FilterType::Bilinear)
        } else {
            fr::ResizeAlg::Convolution(fr::FilterType::CatmullRom)
        };

        #[cfg(target_os = "macos")]
        imgproc::cvt_color(
            &self.decode_mat,
            &mut self.rgb_mat,
            imgproc::COLOR_BGR2RGB,
            0,
            core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;
        #[cfg(not(target_os = "macos"))]
        imgproc::cvt_color(
            &self.decode_mat,
            &mut self.rgb_mat,
            imgproc::COLOR_BGR2RGB,
            0,
        )?;

        if !self.rgb_mat.is_continuous() {
            return Err(anyhow!("Frame is not continuous"));
        }

        let src_bytes = self.rgb_mat.data_bytes()?;
        let src_image = ImageRef::new(orig_w, orig_h, src_bytes, fr::PixelType::U8x3)?;

        // Fill mode can skip intermediate oversized buffers by cropping source ratio directly
        // into destination dimensions and resizing in one pass.
        let can_fill_without_forced_upscale =
            self.allow_upscale || (self.width <= orig_w && self.height <= orig_h);
        if self.fill_mode && can_fill_without_forced_upscale {
            let out_len = (self.width * self.height * 3) as usize;
            if buffer.len() != out_len {
                buffer.resize(out_len, 0);
            }

            let mut dst_image = Image::from_slice_u8(
                self.width,
                self.height,
                buffer.as_mut_slice(),
                fr::PixelType::U8x3,
            )?;
            let options = fr::ResizeOptions::new()
                .resize_alg(resize_alg)
                .fit_into_destination(Some((0.5, 0.5)));
            self.resizer
                .resize(&src_image, &mut dst_image, Some(&options))?;

            let resize_time = start_resize.elapsed();
            self.record_perf(
                start_total.elapsed(),
                decode_time,
                resize_time,
                Duration::ZERO,
            );
            return Ok(true);
        }

        let new_w = ((orig_w as f64 * scale).round() as u32).max(1);
        let new_h = ((orig_h as f64 * scale).round() as u32).max(1);
        let resized_len = (new_w * new_h * 3) as usize;
        if self.resized_buffer.len() != resized_len {
            self.resized_buffer.resize(resized_len, 0);
        }
        {
            let mut dst_image = Image::from_slice_u8(
                new_w,
                new_h,
                self.resized_buffer.as_mut_slice(),
                fr::PixelType::U8x3,
            )?;
            let options = fr::ResizeOptions::new().resize_alg(resize_alg);
            self.resizer
                .resize(&src_image, &mut dst_image, Some(&options))?;
        }
        let resize_time = start_resize.elapsed();

        let start_compose = Instant::now();
        let out_len = (self.width * self.height * 3) as usize;
        if buffer.len() != out_len {
            buffer.resize(out_len, 0);
        }

        if new_w > self.width || new_h > self.height {
            // Crop center directly into output buffer (full-frame write, no clear needed).
            let crop_x = ((new_w - self.width) / 2) as usize;
            let crop_y = ((new_h - self.height) / 2) as usize;
            let src_stride = new_w as usize * 3;
            let dst_stride = self.width as usize * 3;

            for y in 0..self.height as usize {
                let src_offset = (crop_y + y) * src_stride + crop_x * 3;
                let dst_offset = y * dst_stride;
                buffer[dst_offset..dst_offset + dst_stride]
                    .copy_from_slice(&self.resized_buffer[src_offset..src_offset + dst_stride]);
            }
        } else {
            // Letterbox center with black bars.
            buffer.fill(0);
            let x_off = ((self.width - new_w) / 2) as usize;
            let y_off = ((self.height - new_h) / 2) as usize;
            let src_stride = new_w as usize * 3;
            let dst_stride = self.width as usize * 3;

            for y in 0..new_h as usize {
                let src_offset = y * src_stride;
                let dst_offset = (y_off + y) * dst_stride + x_off * 3;
                buffer[dst_offset..dst_offset + src_stride]
                    .copy_from_slice(&self.resized_buffer[src_offset..src_offset + src_stride]);
            }
        }
        let compose_time = start_compose.elapsed();

        self.record_perf(
            start_total.elapsed(),
            decode_time,
            resize_time,
            compose_time,
        );

        Ok(true)
    }

    fn record_perf(
        &mut self,
        total_time: Duration,
        decode_time: Duration,
        resize_time: Duration,
        compose_time: Duration,
    ) {
        self.perf_frames += 1;
        self.perf_total_us += total_time.as_micros();
        self.perf_decode_us += decode_time.as_micros();
        self.perf_resize_us += resize_time.as_micros();
        self.perf_compose_us += compose_time.as_micros();

        if self.perf_frames % crate::shared::constants::PERF_LOG_EVERY_FRAMES != 0 {
            return;
        }

        let avg_total_ms = self.perf_total_us as f64 / self.perf_frames as f64 / 1000.0;
        let avg_decode_ms = self.perf_decode_us as f64 / self.perf_frames as f64 / 1000.0;
        let avg_resize_ms = self.perf_resize_us as f64 / self.perf_frames as f64 / 1000.0;
        let avg_compose_ms = self.perf_compose_us as f64 / self.perf_frames as f64 / 1000.0;
        let fps =
            self.perf_frames as f64 / self.perf_window_start.elapsed().as_secs_f64().max(0.001);

        crate::utils::logger::debug(&format!(
            "decoder perf: fps={:.1} avg_total={:.2}ms decode={:.2}ms resize={:.2}ms compose={:.2}ms",
            fps, avg_total_ms, avg_decode_ms, avg_resize_ms, avg_compose_ms
        ));

        self.perf_window_start = Instant::now();
        self.perf_frames = 0;
        self.perf_total_us = 0;
        self.perf_decode_us = 0;
        self.perf_resize_us = 0;
        self.perf_compose_us = 0;
    }
}
