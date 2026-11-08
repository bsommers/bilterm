//! Cross-platform audio capture pipeline (`STT-02`).
//!
//! Provides zero-native-dependency subprocess streaming over piped stdout
//! and a deterministic mock audio generator for headless CI testing.

use std::io::Read;
use std::process::{Child, Command, Stdio};

use super::wav::STT_BYTES_PER_SAMPLE;

/// Supported system audio recording binaries and mock backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioRecorderKind {
    PwRecord,
    PaRecord,
    ARecord,
    SoxRec,
    Ffmpeg,
    Mock,
}

impl AudioRecorderKind {
    /// Command line executable name for this recorder.
    pub fn executable_name(&self) -> &'static str {
        match self {
            Self::PwRecord => "pw-record",
            Self::PaRecord => "parecord",
            Self::ARecord => "arecord",
            Self::SoxRec => "rec",
            Self::Ffmpeg => "ffmpeg",
            Self::Mock => "mock",
        }
    }
}

/// Helper checking if an executable is found on system `$PATH`.
fn is_executable_on_path(exe: &str) -> bool {
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path_var) {
        let p = dir.join(exe);
        if p.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = p.metadata()
                    && meta.permissions().mode() & 0o111 != 0
                {
                    return true;
                }
            }
            #[cfg(not(unix))]
            {
                return true;
            }
        }
    }
    false
}

/// Probes the system for an available audio recording binary according to OS preference.
pub fn probe_available_recorder(preferred: &str) -> Result<AudioRecorderKind, String> {
    let pref = preferred.trim().to_ascii_lowercase();
    match pref.as_str() {
        "mock" => return Ok(AudioRecorderKind::Mock),
        "pw-record" | "pipewire" => {
            if is_executable_on_path("pw-record") {
                return Ok(AudioRecorderKind::PwRecord);
            }
            return Err("Configured audio recorder 'pw-record' was not found on PATH".to_string());
        }
        "parecord" | "pulseaudio" => {
            if is_executable_on_path("parecord") {
                return Ok(AudioRecorderKind::PaRecord);
            }
            return Err("Configured audio recorder 'parecord' was not found on PATH".to_string());
        }
        "arecord" | "alsa" => {
            if is_executable_on_path("arecord") {
                return Ok(AudioRecorderKind::ARecord);
            }
            return Err("Configured audio recorder 'arecord' was not found on PATH".to_string());
        }
        "sox" | "rec" => {
            if is_executable_on_path("rec") || is_executable_on_path("sox") {
                return Ok(AudioRecorderKind::SoxRec);
            }
            return Err("Configured audio recorder 'rec'/'sox' was not found on PATH".to_string());
        }
        "ffmpeg" => {
            if is_executable_on_path("ffmpeg") {
                return Ok(AudioRecorderKind::Ffmpeg);
            }
            return Err("Configured audio recorder 'ffmpeg' was not found on PATH".to_string());
        }
        "auto" | "" => {}
        other => return Err(format!("Unknown audio recorder preference '{other}'")),
    }

    // Auto-probe platform priority order
    #[cfg(target_os = "linux")]
    {
        if is_executable_on_path("pw-record") {
            return Ok(AudioRecorderKind::PwRecord);
        }
        if is_executable_on_path("parecord") {
            return Ok(AudioRecorderKind::PaRecord);
        }
        if is_executable_on_path("arecord") {
            return Ok(AudioRecorderKind::ARecord);
        }
        if is_executable_on_path("ffmpeg") {
            return Ok(AudioRecorderKind::Ffmpeg);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if is_executable_on_path("rec") || is_executable_on_path("sox") {
            return Ok(AudioRecorderKind::SoxRec);
        }
        if is_executable_on_path("ffmpeg") {
            return Ok(AudioRecorderKind::Ffmpeg);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if is_executable_on_path("ffmpeg.exe") || is_executable_on_path("ffmpeg") {
            return Ok(AudioRecorderKind::Ffmpeg);
        }
        if is_executable_on_path("sox.exe") || is_executable_on_path("sox") {
            return Ok(AudioRecorderKind::SoxRec);
        }
    }

    // Generic fallback for any other OS
    if is_executable_on_path("ffmpeg") {
        return Ok(AudioRecorderKind::Ffmpeg);
    }
    if is_executable_on_path("rec") {
        return Ok(AudioRecorderKind::SoxRec);
    }

    Err("No supported audio recorder found on PATH (install pipewire, pulseaudio, alsa-utils, sox, or ffmpeg)".to_string())
}

/// Handle to an active recording audio stream.
pub trait AudioStreamHandle: Send {
    /// Reads the next PCM chunk (e.g. 100 ms = 3200 bytes).
    ///
    /// Returns `Ok(Some(bytes))` on incoming data, `Ok(None)` if stream ended.
    fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String>;

    /// Stops audio capture and reaps child process if any.
    fn stop(&mut self) -> Result<(), String>;
}

/// Abstract audio capture backend.
pub trait AudioCaptureBackend: Send + Sync {
    /// Starts a recording stream bounded by `max_secs`.
    fn start_recording(&self, max_secs: u64) -> Result<Box<dyn AudioStreamHandle>, String>;
}

/// Subprocess-based audio capture streaming 16 kHz 16-bit signed LE mono PCM to stdout.
pub struct SubprocessAudioCapture {
    pub kind: AudioRecorderKind,
}

impl SubprocessAudioCapture {
    pub fn new(kind: AudioRecorderKind) -> Self {
        Self { kind }
    }
}

pub struct SubprocessAudioStream {
    child: Child,
    stdout: Box<dyn Read + Send>,
    max_bytes: usize,
    bytes_read: usize,
}

impl AudioStreamHandle for SubprocessAudioStream {
    fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.bytes_read >= self.max_bytes {
            return Ok(None);
        }

        // 100 ms of 16 kHz 16-bit mono = 1600 samples * 2 bytes = 3200 bytes
        let chunk_size = 3200usize;
        let mut buf = vec![0u8; chunk_size];
        let mut n_read = 0;

        while n_read < chunk_size && self.bytes_read + n_read < self.max_bytes {
            match self.stdout.read(&mut buf[n_read..chunk_size]) {
                Ok(0) => break,
                Ok(n) => n_read += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("Audio capture read error: {e}")),
            }
        }

        if n_read == 0 {
            return Ok(None);
        }

        buf.truncate(n_read);
        self.bytes_read += n_read;
        Ok(Some(buf))
    }

    fn stop(&mut self) -> Result<(), String> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for SubprocessAudioStream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl AudioCaptureBackend for SubprocessAudioCapture {
    fn start_recording(&self, max_secs: u64) -> Result<Box<dyn AudioStreamHandle>, String> {
        let max_bytes = (max_secs as usize) * 16_000 * STT_BYTES_PER_SAMPLE;
        let mut cmd = match self.kind {
            AudioRecorderKind::PwRecord => {
                let mut c = Command::new("pw-record");
                c.args(["--format=s16", "--rate=16000", "--channels=1", "-"]);
                c
            }
            AudioRecorderKind::PaRecord => {
                let mut c = Command::new("parecord");
                c.args(["--format=s16le", "--rate=16000", "--channels=1", "--raw"]);
                c
            }
            AudioRecorderKind::ARecord => {
                let mut c = Command::new("arecord");
                c.args(["-q", "-f", "S16_LE", "-r", "16000", "-c", "1", "-t", "raw", "-"]);
                c
            }
            AudioRecorderKind::SoxRec => {
                let exe = if is_executable_on_path("rec") { "rec" } else { "sox" };
                let mut c = Command::new(exe);
                if exe == "sox" {
                    c.args(["-d"]);
                }
                c.args(["-q", "-t", "raw", "-b", "16", "-e", "signed-integer", "-r", "16000", "-c", "1", "-"]);
                c
            }
            AudioRecorderKind::Ffmpeg => {
                let mut c = Command::new("ffmpeg");
                c.args(["-loglevel", "quiet"]);
                #[cfg(target_os = "linux")]
                c.args(["-f", "pulse", "-i", "default"]);
                #[cfg(target_os = "macos")]
                c.args(["-f", "avfoundation", "-i", ":default"]);
                #[cfg(target_os = "windows")]
                c.args(["-f", "dshow", "-i", "audio=default"]);
                #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
                c.args(["-f", "pulse", "-i", "default"]);
                c.args(["-ar", "16000", "-ac", "1", "-f", "s16le", "-"]);
                c
            }
            AudioRecorderKind::Mock => {
                return Err("Mock recorder should use MockAudioCapture".to_string());
            }
        };

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn audio recorder '{}': {e}", self.kind.executable_name()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture audio recorder stdout pipe".to_string())?;

        Ok(Box::new(SubprocessAudioStream {
            child,
            stdout: Box::new(stdout),
            max_bytes,
            bytes_read: 0,
        }))
    }
}

/// Deterministic mock audio capture generator for unit and integration testing.
pub struct MockAudioCapture {
    speech_chunks: usize,
    silence_chunks: usize,
}

impl MockAudioCapture {
    /// Creates a mock capture emitting `speech_chunks` active audio chunks (RMS ~ -20 dBFS)
    /// followed by `silence_chunks` silent audio chunks (RMS == -96 dBFS).
    pub fn new(speech_chunks: usize, silence_chunks: usize) -> Self {
        Self {
            speech_chunks,
            silence_chunks,
        }
    }
}

pub struct MockAudioStream {
    speech_remaining: usize,
    silence_remaining: usize,
}

impl AudioStreamHandle for MockAudioStream {
    fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.speech_remaining > 0 {
            self.speech_remaining -= 1;
            // Generate 1600 samples of 440 Hz sine wave (~ -20 dBFS)
            let mut chunk = Vec::with_capacity(3200);
            for i in 0..1600 {
                let sample = (3276.0 * (2.0 * std::f64::consts::PI * 440.0 * (i as f64) / 16000.0).sin()) as i16;
                chunk.extend_from_slice(&sample.to_le_bytes());
            }
            return Ok(Some(chunk));
        }

        if self.silence_remaining > 0 {
            self.silence_remaining -= 1;
            // Generate 1600 samples of pure silence (0s)
            return Ok(Some(vec![0u8; 3200]));
        }

        Ok(None)
    }

    fn stop(&mut self) -> Result<(), String> {
        self.speech_remaining = 0;
        self.silence_remaining = 0;
        Ok(())
    }
}

impl AudioCaptureBackend for MockAudioCapture {
    fn start_recording(&self, _max_secs: u64) -> Result<Box<dyn AudioStreamHandle>, String> {
        Ok(Box::new(MockAudioStream {
            speech_remaining: self.speech_chunks,
            silence_remaining: self.silence_chunks,
        }))
    }
}
