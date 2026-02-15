use std::time::{Duration, Instant};
use super::clock::MasterClock;

/// Adaptive VSync manager for smooth frame pacing
/// 
/// Handles frame timing with compensation for rendering overhead
/// and supports frame dropping when running behind schedule.
pub struct VSync {
    target_fps: f64,
    frame_duration: Duration,
    next_frame_time: Instant,
    frames_rendered: u64,
    frames_dropped: u64,
}

impl VSync {
    /// Create a new VSync manager with target FPS
    pub fn new(fps: f64) -> Self {
        let frame_duration = Duration::from_secs_f64(1.0 / fps);
        Self {
            target_fps: fps,
            frame_duration,
            next_frame_time: Instant::now() + frame_duration,
            frames_rendered: 0,
            frames_dropped: 0,
        }
    }

    /// Wait until it's time to render the next frame
    /// 
    /// This accounts for rendering time and sleeps only if needed.
    /// If we're already past the next frame time, returns immediately.
    pub fn wait_for_next_frame(&mut self) {
        let now = Instant::now();
        
        // If we're more than one frame duration behind, resync
        // This prevents infinite drift where next_frame_time keeps getting further behind
        if now > self.next_frame_time + self.frame_duration * 3 {
            // Reset to current time to prevent runaway frame skipping
            self.next_frame_time = now + self.frame_duration;
            self.frames_rendered += 1;
            return;
        }
        
        if now < self.next_frame_time {
            std::thread::sleep(self.next_frame_time - now);
        }
        
        // Advance to next frame
        self.next_frame_time += self.frame_duration;
        self.frames_rendered += 1;
    }

    /// Check if we should drop the current frame to catch up
    /// 
    /// Returns true if the clock is significantly ahead of where we should be
    pub fn should_drop_frame(&self, clock: &MasterClock) -> bool {
        let elapsed = clock.elapsed();
        let expected_frame = (elapsed.as_secs_f64() * self.target_fps) as u64;
        
        // Only drop if we're more than 5 frames behind (very aggressive lag)
        // Reduced from 2 to prevent premature frame dropping
        let behind_by = expected_frame.saturating_sub(self.frames_rendered);
        behind_by > 5
    }

    /// Mark a frame as dropped
    pub fn drop_frame(&mut self) {
        self.frames_dropped += 1;
        self.next_frame_time += self.frame_duration;
    }

    /// Get rendering statistics
    pub fn stats(&self) -> VSyncStats {
        VSyncStats {
            frames_rendered: self.frames_rendered,
            frames_dropped: self.frames_dropped,
            target_fps: self.target_fps,
        }
    }

    /// Reset frame counters
    pub fn reset(&mut self) {
        self.next_frame_time = Instant::now() + self.frame_duration;
        self.frames_rendered = 0;
        self.frames_dropped = 0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VSyncStats {
    pub frames_rendered: u64,
    pub frames_dropped: u64,
    pub target_fps: f64,
}

impl VSyncStats {
    pub fn effective_fps(&self, elapsed: Duration) -> f64 {
        if elapsed.as_secs_f64() > 0.0 {
            self.frames_rendered as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        }
    }
}
