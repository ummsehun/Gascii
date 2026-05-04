use anyhow::{anyhow, Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct AudioManager {
    rodio: Option<RodioAudio>,
    #[cfg(windows)]
    native: Option<windows_mci::WindowsMciAudio>,
    active_backend: Option<AudioBackendKind>,
}

struct RodioAudio {
    _stream: OutputStream,
    _stream_handle: rodio::OutputStreamHandle,
    sink: Arc<Mutex<Sink>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioBackendKind {
    Rodio,
    #[cfg(windows)]
    Native,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        match RodioAudio::new() {
            Ok(rodio) => Ok(Self {
                rodio: Some(rodio),
                #[cfg(windows)]
                native: None,
                active_backend: None,
            }),
            Err(error) => {
                #[cfg(windows)]
                {
                    crate::utils::logger::error(&format!(
                        "Rodio audio output unavailable; falling back to Windows MCI: {}",
                        error
                    ));
                    Ok(Self {
                        rodio: None,
                        native: Some(windows_mci::WindowsMciAudio::new()),
                        active_backend: None,
                    })
                }

                #[cfg(not(windows))]
                {
                    Err(error)
                }
            }
        }
    }

    pub fn play(&mut self, path: &str) -> Result<Instant> {
        if let Some(rodio) = &self.rodio {
            match rodio.play(path) {
                Ok(clock_start) => {
                    self.active_backend = Some(AudioBackendKind::Rodio);
                    return Ok(clock_start);
                }
                Err(error) => {
                    #[cfg(windows)]
                    {
                        crate::utils::logger::error(&format!(
                            "Rodio failed to play {}; falling back to Windows MCI: {}",
                            path, error
                        ));
                    }

                    #[cfg(not(windows))]
                    {
                        return Err(error);
                    }
                }
            }
        }

        #[cfg(windows)]
        {
            let native = self
                .native
                .get_or_insert_with(windows_mci::WindowsMciAudio::new);
            let clock_start = native.play(path)?;
            self.active_backend = Some(AudioBackendKind::Native);
            return Ok(clock_start);
        }

        #[cfg(not(windows))]
        {
            Err(anyhow!("No audio backend available"))
        }
    }

    pub fn stop(&self) -> Result<()> {
        match self.active_backend {
            Some(AudioBackendKind::Rodio) => {
                if let Some(rodio) = &self.rodio {
                    rodio.stop()?;
                }
            }
            #[cfg(windows)]
            Some(AudioBackendKind::Native) => {
                if let Some(native) = &self.native {
                    native.stop()?;
                }
            }
            None => {}
        }
        Ok(())
    }

    pub fn is_finished(&self) -> Result<bool> {
        match self.active_backend {
            Some(AudioBackendKind::Rodio) => self
                .rodio
                .as_ref()
                .ok_or_else(|| anyhow!("Rodio audio backend missing"))?
                .is_finished(),
            #[cfg(windows)]
            Some(AudioBackendKind::Native) => self
                .native
                .as_ref()
                .ok_or_else(|| anyhow!("Windows MCI audio backend missing"))?
                .is_finished(),
            None => Ok(true),
        }
    }
}

impl RodioAudio {
    fn new() -> Result<Self> {
        let (_stream, stream_handle) =
            OutputStream::try_default().context("No audio output device found")?;
        let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

        Ok(Self {
            _stream,
            _stream_handle: stream_handle,
            sink: Arc::new(Mutex::new(sink)),
        })
    }

    fn play(&self, path: &str) -> Result<Instant> {
        let file =
            File::open(path).with_context(|| format!("Failed to open audio file: {}", path))?;
        let source = Decoder::new(BufReader::new(file)).context("Failed to decode audio")?;

        let sink = self
            .sink
            .lock()
            .map_err(|_| anyhow!("Audio sink mutex poisoned"))?;
        if !sink.empty() {
            sink.stop();
        }
        sink.append(source);
        sink.play();
        Ok(Instant::now())
    }

    fn stop(&self) -> Result<()> {
        let sink = self
            .sink
            .lock()
            .map_err(|_| anyhow!("Audio sink mutex poisoned"))?;
        sink.stop();
        Ok(())
    }

    fn is_finished(&self) -> Result<bool> {
        let sink = self
            .sink
            .lock()
            .map_err(|_| anyhow!("Audio sink mutex poisoned"))?;
        Ok(sink.empty())
    }
}

#[cfg(windows)]
mod windows_mci {
    use anyhow::{anyhow, Result};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::time::Instant;

    #[link(name = "winmm")]
    extern "system" {
        fn mciSendStringW(
            command: *const u16,
            return_string: *mut u16,
            return_length: u32,
            callback: usize,
        ) -> u32;
    }

    pub struct WindowsMciAudio {
        alias: String,
    }

    impl WindowsMciAudio {
        pub fn new() -> Self {
            Self {
                alias: format!("gascii_audio_{}", std::process::id()),
            }
        }

        pub fn play(&self, path: &str) -> Result<Instant> {
            let _ = self.close();
            mci_send(&format!(
                "open \"{}\" type {} alias {}",
                escape_mci_path(path),
                mci_device_type(path),
                self.alias
            ))?;
            mci_send(&format!("play {} from 0", self.alias))?;
            Ok(Instant::now())
        }

        pub fn stop(&self) -> Result<()> {
            let _ = mci_send(&format!("stop {}", self.alias));
            self.close()
        }

        pub fn is_finished(&self) -> Result<bool> {
            let status = mci_query(&format!("status {} mode", self.alias))?;
            Ok(matches!(status.as_str(), "stopped" | "not ready"))
        }

        fn close(&self) -> Result<()> {
            let _ = mci_send(&format!("close {}", self.alias));
            Ok(())
        }
    }

    impl Drop for WindowsMciAudio {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }

    fn mci_send(command: &str) -> Result<()> {
        let command = wide(command);
        let status = unsafe { mciSendStringW(command.as_ptr(), std::ptr::null_mut(), 0, 0) };
        if status == 0 {
            Ok(())
        } else {
            Err(anyhow!("Windows MCI command failed with code {}", status))
        }
    }

    fn mci_query(command: &str) -> Result<String> {
        let command = wide(command);
        let mut output = vec![0u16; 128];
        let status = unsafe {
            mciSendStringW(
                command.as_ptr(),
                output.as_mut_ptr(),
                output.len() as u32,
                0,
            )
        };
        if status != 0 {
            return Err(anyhow!("Windows MCI query failed with code {}", status));
        }
        let len = output
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(output.len());
        Ok(String::from_utf16_lossy(&output[..len]))
    }

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }

    fn escape_mci_path(path: &str) -> String {
        path.replace('"', "")
    }

    fn mci_device_type(path: &str) -> &'static str {
        match path
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("wav" | "wave") => "waveaudio",
            _ => "mpegvideo",
        }
    }
}
