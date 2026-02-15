use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};

pub struct AudioManager {
    _stream: OutputStream,
    _stream_handle: rodio::OutputStreamHandle,
    sink: Arc<Mutex<Sink>>,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        let (_stream, stream_handle) = OutputStream::try_default().context("No audio output device found")?;
        let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;
        
        Ok(Self {
            _stream,
            _stream_handle: stream_handle,
            sink: Arc::new(Mutex::new(sink)),
        })
    }

    pub fn play(&self, path: &str) -> Result<()> {
        let file = File::open(path).with_context(|| format!("Failed to open audio file: {}", path))?;
        let source = Decoder::new(BufReader::new(file)).context("Failed to decode audio")?;
        
        let sink = self.sink.lock().map_err(|_| anyhow::anyhow!("Audio sink mutex poisoned"))?;
        if !sink.empty() {
            sink.stop();
        }
        sink.append(source);
        sink.play();
        Ok(())
    }

}
