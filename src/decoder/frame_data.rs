use std::time::Duration;

/// Frame data structure for video frames
#[derive(Clone)]
pub struct FrameData {
    pub buffer: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp: Duration,
}

impl FrameData {
    pub fn new(buffer: Vec<u8>, width: u32, height: u32, timestamp: Duration) -> Self {
        Self { buffer, width, height, timestamp }
    }
}
