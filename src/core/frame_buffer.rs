use crossbeam::queue::ArrayQueue;
use std::sync::Arc;

/// Lock-free ring buffer for video frames
/// Decouples FFmpeg I/O from rendering pipeline
pub struct FrameBuffer {
    queue: Arc<ArrayQueue<Vec<u8>>>,
    capacity: usize,
}

impl FrameBuffer {
    /// Create new frame buffer with specified capacity
    /// Capacity should be ~2-3 seconds of video (e.g., 60 frames at 24fps)
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
            capacity,
        }
    }

    /// Try to push a frame (non-blocking)
    /// Returns false if buffer is full (frame will be dropped)
    pub fn push(&self, frame: Vec<u8>) -> bool {
        self.queue.push(frame).is_ok()
    }

    /// Try to pop a frame (non-blocking)
    /// Returns None if buffer is empty
    pub fn pop(&self) -> Option<Vec<u8>> {
        self.queue.pop()
    }

    /// Get current buffer fill level (0.0 to 1.0)
    pub fn fill_level(&self) -> f32 {
        self.queue.len() as f32 / self.capacity as f32
    }

    #[allow(dead_code)]
    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get clone of the queue Arc for sharing across threads
    pub fn clone_queue(&self) -> Arc<ArrayQueue<Vec<u8>>> {
        Arc::clone(&self.queue)
    }
}
