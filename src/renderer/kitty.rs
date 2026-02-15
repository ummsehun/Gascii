use anyhow::Result;
use std::io::{BufWriter, Write};

/// Kitty Graphics Protocol Renderer
/// Uses PNG encoding + base64 + Kitty escape sequences
pub struct KittyRenderer {
    stdout: BufWriter<std::io::Stdout>,
    term_width: u16,
    term_height: u16,
}

impl KittyRenderer {
    pub fn new() -> Result<Self> {
        let (term_width, term_height) = crossterm::terminal::size()?;
        
        // Initialize Kitty graphics mode
        let stdout = BufWriter::with_capacity(1024 * 1024, std::io::stdout()); // 1MB buffer
        
        eprintln!("🖼️  Kitty Graphics Renderer initialized ({}x{} cells)", term_width, term_height);
        
        Ok(Self {
            stdout,
            term_width,
            term_height,
        })
    }
    
    // Render a frame using Kitty Graphics Protocol
    pub fn render_frame(&mut self, pixel_data: &[u8], width: u32, height: u32) -> Result<()> {
        // 1. Encode frame as PNG
        let png_data = self.encode_png(pixel_data, width, height)?;
        
        // 2. Base64 encode
        let b64_data = base64::encode(&png_data);
        
        // 3. Calculate display dimensions
        // Kitty protocol uses character cells for positioning
        let cols = self.term_width.min((width / 10) as u16); // Approximate
        let rows = self.term_height.min((height / 20) as u16);
        
        // 4. Generate Kitty escape sequence
        // Format: \x1b_Ga=T,f=100,s=<size>,c=<cols>,r=<rows>;<base64_data>\x1b\\
        let escape_seq = format!(
            "\x1b_Ga=T,f=100,s={},c={},r={};{}\x1b\\",
            png_data.len(),
            cols,
            rows,
            b64_data
        );
        
        // 5. Write to terminal
        self.stdout.write_all(escape_seq.as_bytes())?;
        self.stdout.flush()?;
        
        Ok(())
    }

    /// Encode RGB pixel data as PNG
    fn encode_png(&self, pixel_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        use image::{ImageBuffer, Rgb};
        
        // Create image from raw pixel data
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, pixel_data.to_vec())
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        
        // Encode as PNG
        let mut png_buffer = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_buffer), image::ImageOutputFormat::Png)?;
        
        Ok(png_buffer)
    }

    /// Clear the screen
    pub fn clear(&mut self) -> Result<()> {
        // Use Kitty delete command to clear all graphics
        self.stdout.write_all(b"\x1b_Ga=d\x1b\\")?;
        self.stdout.flush()?;
        Ok(())
    }
}

impl Drop for KittyRenderer {
    fn drop(&mut self) {
        // Cleanup: delete all graphics on exit
        let _ = self.clear();
    }
}
