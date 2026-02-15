use anyhow::Result;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Audio player with lifecycle management
/// 
/// Wraps ffplay for audio playback with proper cleanup
pub struct AudioPlayer {
    process: Option<Child>,
    running: Arc<AtomicBool>,
}

impl AudioPlayer {
    /// Create a new audio player for the given file
    pub fn new(audio_path: &str) -> Result<Self> {
        let running = Arc::new(AtomicBool::new(false));
        
        let child = Command::new("ffplay")
            .arg("-nodisp")          // No video display
            .arg("-autoexit")        // Exit when done
            .arg("-hide_banner")     // Clean output
            .arg("-loglevel").arg("error")
            .arg(audio_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        
        running.store(true, Ordering::Relaxed);
        
        Ok(Self {
            process: Some(child),
            running,
        })
    }

    /// Check if audio is currently playing
    pub fn is_playing(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop audio playback
    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
            self.running.store(false, Ordering::Relaxed);
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
