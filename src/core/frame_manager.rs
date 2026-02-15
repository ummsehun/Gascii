use anyhow::Result;
use crate::utils::file_utils;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::VecDeque;

pub struct FrameManager {
    // Metadata
    width: usize,
    height: usize,
    // Packed frames (1 bit per pixel) - stored as Arc for cheap clones
    packed_frames: Vec<Arc<Vec<u8>>>,
    // Optional cache for expanded RGB frames
    expanded_cache: Mutex<Vec<Option<Arc<Vec<u8>>>>>,
    // LRU tracking for cache evictions
    cache_order: Mutex<VecDeque<usize>>,
    cache_capacity: usize,
}

impl FrameManager {
    pub fn new() -> Self {
        Self { width: 0, height: 0, packed_frames: Vec::new(), expanded_cache: Mutex::new(Vec::new()), cache_order: Mutex::new(VecDeque::new()), cache_capacity: 64 }
    }

    pub fn load_frames(&mut self, dir: &str, _extension: &str) -> Result<usize> {
        let path = std::path::Path::new(dir).join("video.bin");
        println!("Loading video data from {:?}...", path);
        
        if !path.exists() {
            return Ok(0);
        }

        let data = file_utils::read_file(&path)?;
        
        if data.len() < 8 {
            return Ok(0);
        }

        // Header: Width(u16), Height(u16), FrameCount(u32)
        let width = u16::from_le_bytes([data[0], data[1]]) as usize;
        let height = u16::from_le_bytes([data[2], data[3]]) as usize;
        let frame_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        
        let compressed_body = &data[8..];
        
        println!("Decompressing data ({} frames, {}x{})...", frame_count, width, height);
        
        // Calculate expected unpacked size (1 bit per pixel)
        // Note: The extractor packed it as (width * height * 2 + 7) / 8 bytes per frame
        // We need to decompress to that size first
        let packed_frame_size = ((width * (height * 2)) + 7) / 8;
        let total_packed_size = packed_frame_size * frame_count;
        
        let decompressed_packed = lz4::block::decompress(compressed_body, Some(total_packed_size as i32))?;

        if decompressed_packed.len() < total_packed_size {
            anyhow::bail!("Decompressed data length {} shorter than expected {}", decompressed_packed.len(), total_packed_size);
        }
        
        println!("Storing packed frames...");
        self.packed_frames.reserve(frame_count);
        
        // Unpack each frame to RGB (or Grayscale) for the renderer
        // Renderer expects: [Width(u16)][Height(u16)][R,G,B, R,G,B...]
        // To save memory, let's just store [Width(u16)][Height(u16)][Gray, Gray...] (1 byte per pixel)
        // But DisplayManager expects RGB (3 bytes). Let's stick to RGB for compatibility for now, 
        // or update DisplayManager. Updating DisplayManager is better but risky.
        // Let's generate RGB frames to be safe and compatible with existing DisplayManager.
        // It uses more RAM but we solved the DISK size issue.
        
        let _pixels_per_frame = width * (height * 2);
        
        for i in 0..frame_count {
            let packed_start = i * packed_frame_size;
            let packed_frame = &decompressed_packed[packed_start..packed_start + packed_frame_size];
            // Store packed_frame as Arc to reduce memory usage
            self.packed_frames.push(Arc::new(packed_frame.to_vec()));
        }

        // Save width/height and initialize cache
        self.width = width;
        self.height = height;
        let mut cache = match self.expanded_cache.lock() {
            Ok(c) => c,
            Err(poisoned) => poisoned.into_inner(),
        };
        cache.clear();
        cache.resize(frame_count, None);
        // Initialize LRU structures
        let mut order = match self.cache_order.lock() {
            Ok(c) => c,
            Err(poisoned) => poisoned.into_inner(),
        };
        order.clear();

        println!("Stored {} packed frames.", self.packed_frames.len());
        Ok(self.packed_frames.len())
    }

    pub fn get_frame(&self, index: usize) -> Option<Arc<Vec<u8>>> {
        if index >= self.packed_frames.len() { return None; }

        // Check cache
        {
            let cache = match self.expanded_cache.lock() {
                Ok(c) => c,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(ref cached) = cache[index] {
                return Some(Arc::clone(cached));
            }
        }

        // Expand packed frame to RGB + header
        let packed = Arc::clone(&self.packed_frames[index]);
        let pixels_per_frame = self.width * (self.height * 2);
        let mut frame_data = Vec::with_capacity(4 + pixels_per_frame * 3);
        frame_data.extend_from_slice(&(self.width as u16).to_le_bytes());
        frame_data.extend_from_slice(&(self.height as u16).to_le_bytes());

        let mut bit_idx = 0usize;
        for _ in 0..pixels_per_frame {
            let byte_pos = bit_idx / 8;
            let bit_pos = 7 - (bit_idx % 8);
            let is_white = (packed[byte_pos] >> bit_pos) & 1 == 1;
            let val = if is_white { 255 } else { 0 };
            frame_data.push(val);
            frame_data.push(val);
            frame_data.push(val);
            bit_idx += 1;
        }

        let arc = Arc::new(frame_data);
        let mut cache = match self.expanded_cache.lock() {
            Ok(c) => c,
            Err(poisoned) => poisoned.into_inner(),
        };
        cache[index] = Some(Arc::clone(&arc));
        // LRU update
        let mut order = match self.cache_order.lock() {
            Ok(c) => c,
            Err(poisoned) => poisoned.into_inner(),
        };

        order.push_back(index);
        // Evict if capacity exceeded
        while order.len() > self.cache_capacity {
            if let Some(evicted) = order.pop_front() {
                cache[evicted] = None;
            }
        }
        Some(arc)
    }
    pub fn frame_count(&self) -> usize {
        self.packed_frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, File};
    use std::io::Write;

    #[test]
    fn test_load_frames_and_unpack() {
        // Create temp dir
        let tmp_dir = std::env::temp_dir().join("bad_apple_test_frames");
        create_dir_all(&tmp_dir).unwrap();
        let mut video_path = tmp_dir.clone();
        video_path.push("video.bin");

        // Use width=2, height=2, frame_count=2 => pixels_per_frame = 2 * (2*2) = 8
        let width: u16 = 2;
        let height: u16 = 2;
        let frame_count: u32 = 2;

        // Prepare packed data (1 byte per frame since 8 bits)
        let frame1: u8 = 0b10101010; // alternating pixels
        let frame2: u8 = 0b01010101;
        let packed = vec![frame1, frame2];

        // Compress using lz4
        let compressed = lz4::block::compress(&packed, None, false).unwrap();

        let mut file = File::create(&video_path).unwrap();
        file.write_all(&width.to_le_bytes()).unwrap();
        file.write_all(&height.to_le_bytes()).unwrap();
        file.write_all(&frame_count.to_le_bytes()).unwrap();
        file.write_all(&compressed).unwrap();
        file.flush().unwrap();

        let mut fm = FrameManager::new();
        let loaded = fm.load_frames(tmp_dir.to_str().unwrap(), "bin").unwrap();
        assert_eq!(loaded, 2);

        let f0 = fm.get_frame(0).unwrap();
        let f1 = fm.get_frame(1).unwrap();

        // Expanded frame length should be pixels_per_frame * 3 + 4 header
        let pixels_per_frame = (width as usize) * ((height as usize) * 2);
        assert_eq!(f0.len(), 4 + pixels_per_frame * 3);
        assert_eq!(f1.len(), 4 + pixels_per_frame * 3);

        // Check a couple of pixel values: first pixel
        // FrameManager packs/expands white -> 255, black -> 0
        // For frame1 (0b10101010), first bit (MSB) = 1 -> white
        assert_eq!(f0[4], 255); // R
        assert_eq!(f0[5], 255); // G
        assert_eq!(f0[6], 255); // B

        // For frame2 (0b01010101), first bit = 0 -> black
        assert_eq!(f1[4], 0);
        assert_eq!(f1[5], 0);
        assert_eq!(f1[6], 0);
    }
}
