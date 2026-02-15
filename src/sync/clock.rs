use std::time::{Duration, Instant};

/// Monotonic master clock for audio/video synchronization
/// 
/// This clock provides a single source of truth for elapsed time,
/// allowing audio and video to stay in sync.
pub struct MasterClock {
    start: Instant,
    paused: bool,
    pause_time: Option<Instant>,
    total_pause_duration: Duration,
}

impl MasterClock {
    /// Create a new master clock starting at time zero
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            paused: false,
            pause_time: None,
            total_pause_duration: Duration::ZERO,
        }
    }

    /// Get elapsed time since clock started (excluding paused time)
    pub fn elapsed(&self) -> Duration {
        if self.paused {
            if let Some(pause_time) = self.pause_time {
                pause_time.duration_since(self.start) - self.total_pause_duration
            } else {
                Duration::ZERO
            }
        } else {
            Instant::now().duration_since(self.start) - self.total_pause_duration
        }
    }

    /// Pause the clock
    pub fn pause(&mut self) {
        if !self.paused {
            self.paused = true;
            self.pause_time = Some(Instant::now());
        }
    }

    /// Resume the clock
    pub fn resume(&mut self) {
        if self.paused {
            if let Some(pause_time) = self.pause_time {
                self.total_pause_duration += Instant::now().duration_since(pause_time);
            }
            self.paused = false;
            self.pause_time = None;
        }
    }

    /// Check if clock is paused
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Reset the clock to zero
    pub fn reset(&mut self) {
        self.start = Instant::now();
        self.paused = false;
        self.pause_time = None;
        self.total_pause_duration = Duration::ZERO;
    }
}

impl Default for MasterClock {
    fn default() -> Self {
        Self::new()
    }
}
