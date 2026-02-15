use anyhow::Result;
use std::path::Path;

/// Content type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    TwoDimensional,      // 2D animation (Bad Apple style)
    ThreeDimensional,    // 3D live action
}

/// Analyzes video content to determine optimal rendering strategy
pub struct ContentAnalyzer {
    sample_frames: usize,
    threshold: f32,
}

impl ContentAnalyzer {
    pub fn new() -> Self {
        Self {
            sample_frames: 100,  // Analyze first 100 frames
            threshold: 0.30,      // 30% pixel change threshold
        }
    }

    /// Analyze video and classify as 2D or 3D
    pub fn analyze_video(&self, video_path: &Path) -> Result<ContentType> {
        // Create a temporary decoder just for analysis
        let decoder = crate::decoder::VideoDecoder::new(
            video_path.to_str().unwrap(),
            640,  // Low res for fast analysis
            360,
            false // Don't fill
        )?;

        let (sender, receiver) = crossbeam_channel::bounded(10);
        let _handle = decoder.spawn_decoding_thread(sender);

        let mut prev_frame: Option<Vec<u8>> = None;
        let mut diff_sum = 0.0;
        let mut frame_count = 0;

        // Analyze first N frames
        for _ in 0..self.sample_frames {
            match receiver.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(frame) => {
                    if let Some(prev) = &prev_frame {
                        let diff = self.calculate_frame_difference(prev, &frame.buffer);
                        diff_sum += diff;
                        frame_count += 1;
                    }
                    prev_frame = Some(frame.buffer);
                }
                Err(_) => break, // End of video or timeout
            }
        }

        if frame_count == 0 {
            // Default to 3D if analysis fails
            return Ok(ContentType::ThreeDimensional);
        }

        let avg_diff = diff_sum / frame_count as f32;
        
        eprintln!("📊 Content Analysis: {:.1}% avg pixel change", avg_diff * 100.0);
        
        let content_type = if avg_diff < self.threshold {
            eprintln!("🎨 Classification: 2D Animation (ANSI Renderer)");
            ContentType::TwoDimensional
        } else {
            eprintln!("🖼️  Classification: 3D Live Action (Kitty Graphics Renderer)");
            ContentType::ThreeDimensional
        };

        Ok(content_type)
    }

    /// Calculate pixel difference between two frames
    fn calculate_frame_difference(&self, frame1: &[u8], frame2: &[u8]) -> f32 {
        if frame1.len() != frame2.len() {
            return 1.0; // Completely different
        }

        let total_pixels = frame1.len() / 3;
        let mut diff_count = 0;

        for i in 0..total_pixels {
            let idx = i * 3;
            let r_diff = (frame1[idx] as i32 - frame2[idx] as i32).abs();
            let g_diff = (frame1[idx + 1] as i32 - frame2[idx + 1] as i32).abs();
            let b_diff = (frame1[idx + 2] as i32 - frame2[idx + 2] as i32).abs();

            // Threshold: 30 per channel (out of 255)
            if r_diff > 30 || g_diff > 30 || b_diff > 30 {
                diff_count += 1;
            }
        }

        diff_count as f32 / total_pixels as f32
    }
}
