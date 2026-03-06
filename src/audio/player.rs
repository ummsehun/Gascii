use anyhow::{anyhow, Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::process::{Child, Command, Stdio};

enum AudioBackend {
    Rodio {
        _stream: OutputStream,
        _handle: OutputStreamHandle,
        sink: Sink,
    },
    Ffplay {
        process: Child,
    },
}

/// Audio player with lifecycle management.
///
/// Primary backend: rodio (lower startup jitter).
/// Fallback backend: ffplay (works when CoreAudio device init fails).
pub struct AudioPlayer {
    backend: AudioBackend,
}

impl AudioPlayer {
    /// Create a new audio player for the given file.
    pub fn new(audio_path: &str) -> Result<Self> {
        match Self::new_rodio(audio_path) {
            Ok(player) => Ok(player),
            Err(rodio_err) => {
                crate::utils::logger::info(&format!(
                    "rodio init failed, falling back to ffplay: {}",
                    rodio_err
                ));
                Self::new_ffplay(audio_path).with_context(|| {
                    format!(
                        "failed to start audio via rodio and ffplay (rodio: {})",
                        rodio_err
                    )
                })
            }
        }
    }

    fn new_rodio(audio_path: &str) -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("failed to initialize audio output stream")?;
        let sink = Sink::try_new(&handle).context("failed to create audio sink")?;

        let file = File::open(audio_path)
            .with_context(|| format!("failed to open audio file: {}", audio_path))?;
        let source = Decoder::new(BufReader::new(file)).context("failed to decode audio stream")?;

        sink.append(source);
        sink.play();

        Ok(Self {
            backend: AudioBackend::Rodio {
                _stream: stream,
                _handle: handle,
                sink,
            },
        })
    }

    fn new_ffplay(audio_path: &str) -> Result<Self> {
        let child = Command::new("ffplay")
            .arg("-nodisp")
            .arg("-autoexit")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg(audio_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("failed to spawn ffplay: {}", e))?;

        Ok(Self {
            backend: AudioBackend::Ffplay { process: child },
        })
    }

    /// Check if audio is currently playing.
    pub fn is_playing(&mut self) -> bool {
        match &mut self.backend {
            AudioBackend::Rodio { sink, .. } => !sink.empty() && !sink.is_paused(),
            AudioBackend::Ffplay { process } => match process.try_wait() {
                Ok(status) => status.is_none(),
                Err(_) => false,
            },
        }
    }

    /// Stop audio playback.
    pub fn stop(&mut self) {
        match &mut self.backend {
            AudioBackend::Rodio { sink, .. } => sink.stop(),
            AudioBackend::Ffplay { process } => {
                let _ = process.kill();
                let _ = process.wait();
            }
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
