//! Speech-to-Text configuration settings and security validation (`STT-01`, `SEC-R02`).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use super::SttBackendKind;

fn default_provider() -> String {
    "openai_compatible".to_string()
}

fn default_endpoint() -> String {
    "http://127.0.0.1:8080/v1/audio/transcriptions".to_string()
}

fn default_model() -> String {
    "whisper-1".to_string()
}

fn default_language() -> Option<String> {
    Some("en".to_string())
}

fn default_vocabulary_prompt() -> Option<String> {
    Some("Linux shell, bash, zsh, git, cargo, tmux, bilterm, kubectl, docker, flags, pipes".to_string())
}

fn default_audio_backend() -> String {
    "auto".to_string()
}

fn default_max_recording_secs() -> u64 {
    60
}

fn default_auto_stop_silence_ms() -> u64 {
    1500
}

fn default_vad_threshold_db() -> f32 {
    -38.0
}

fn default_cli_command() -> Vec<String> {
    vec![
        "whisper-cli".to_string(),
        "--no-timestamps".to_string(),
        "-nt".to_string(),
        "-f".to_string(),
        "{wav_path}".to_string(),
    ]
}

/// Declarative STT subsystem configuration section (`[stt]` in `config.toml`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SttConfig {
    /// Master switch. When false (default), no audio devices or sockets are opened.
    pub enabled: bool,
    /// STT provider kind: "openai_compatible" | "ollama" | "local_cli" | "mock".
    pub provider: String,
    /// HTTP endpoint URL or unix socket path for transcription.
    pub endpoint: String,
    /// Model name or identifier.
    pub model: String,
    /// Optional ISO-639-1 language code (e.g. "en") to skip detection latency.
    pub language: Option<String>,
    /// Vocabulary biasing prompt for terminal commands and flags.
    pub vocabulary_prompt: Option<String>,
    /// Path to a private 0600 file holding the authentication bearer token.
    pub api_key_file: Option<PathBuf>,
    /// When false (default), non-loopback endpoints are strictly rejected.
    pub allow_remote_stt: bool,
    /// Audio capture backend driver: "auto" | "pw-record" | "parecord" | "arecord" | "sox" | "ffmpeg" | "mock".
    pub audio_backend: String,
    /// Hard memory cap on recording duration in seconds (1..=300).
    pub max_recording_secs: u64,
    /// Voice Activity Detection silence auto-stop delay in milliseconds (0 = disabled).
    pub auto_stop_silence_ms: u64,
    /// Audio energy threshold in dBFS above which voice activity is detected.
    pub vad_threshold_db: f32,
    /// Whether to automatically submit the transcribed command into the shell.
    pub auto_submit: bool,
    /// Command template executed when `provider = "local_cli"`.
    pub cli_command: Vec<String>,
}

impl PartialEq for SttConfig {
    fn eq(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.provider == other.provider
            && self.endpoint == other.endpoint
            && self.model == other.model
            && self.language == other.language
            && self.vocabulary_prompt == other.vocabulary_prompt
            && self.api_key_file == other.api_key_file
            && self.allow_remote_stt == other.allow_remote_stt
            && self.audio_backend == other.audio_backend
            && self.max_recording_secs == other.max_recording_secs
            && self.auto_stop_silence_ms == other.auto_stop_silence_ms
            && self.vad_threshold_db.to_bits() == other.vad_threshold_db.to_bits()
            && self.auto_submit == other.auto_submit
            && self.cli_command == other.cli_command
    }
}

impl Eq for SttConfig {}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_provider(),
            endpoint: default_endpoint(),
            model: default_model(),
            language: default_language(),
            vocabulary_prompt: default_vocabulary_prompt(),
            api_key_file: None,
            allow_remote_stt: false,
            audio_backend: default_audio_backend(),
            max_recording_secs: default_max_recording_secs(),
            auto_stop_silence_ms: default_auto_stop_silence_ms(),
            vad_threshold_db: default_vad_threshold_db(),
            auto_submit: false,
            cli_command: default_cli_command(),
        }
    }
}

impl SttConfig {
    /// Resolves the provider kind from configuration string.
    pub fn backend_kind(&self) -> SttBackendKind {
        match self.provider.trim().to_ascii_lowercase().as_str() {
            "openai" | "openai_compatible" | "whisper" | "faster-whisper" | "speaches" => {
                SttBackendKind::OpenAiCompatible
            }
            "ollama" => SttBackendKind::Ollama,
            "cli" | "local_cli" | "whisper-cli" => SttBackendKind::LocalCli,
            "mock" => SttBackendKind::Mock,
            _ => SttBackendKind::OpenAiCompatible,
        }
    }

    /// Validates that the endpoint URL is strictly confined to loopback unless `allow_remote_stt` is true (`SEC-R04`).
    pub fn validate_endpoint(&self) -> Result<(), String> {
        if self.allow_remote_stt {
            return Ok(());
        }

        let trimmed = self.endpoint.trim();
        if trimmed.starts_with("unix://") || trimmed.starts_with("unix:") {
            return Ok(());
        }

        let without_proto = if let Some(stripped) = trimmed.strip_prefix("http://") {
            stripped
        } else if let Some(stripped) = trimmed.strip_prefix("https://") {
            stripped
        } else {
            return Err(format!(
                "STT endpoint '{}' must start with http://, https://, or unix://",
                self.endpoint
            ));
        };

        // Extract host part before port or path
        let host_port = without_proto.split('/').next().unwrap_or(without_proto);
        let host = if host_port.starts_with('[') {
            // IPv6 literal e.g. [::1]:8080
            if let Some(end_bracket) = host_port.find(']') {
                &host_port[1..end_bracket]
            } else {
                host_port
            }
        } else {
            host_port.split(':').next().unwrap_or(host_port)
        };

        let is_loopback = host.eq_ignore_ascii_case("localhost")
            || host == "127.0.0.1"
            || host == "::1"
            || host.starts_with("127.");

        if !is_loopback {
            return Err(format!(
                "STT endpoint '{}' host '{}' is non-loopback; set allow_remote_stt = true in config.toml to permit remote STT endpoints",
                self.endpoint, host
            ));
        }

        Ok(())
    }

    /// Reads and validates the bearer token from `api_key_file` with strict 0600 permission checks (`SEC-R02`).
    pub fn load_api_key(&self) -> Result<Option<String>, String> {
        let Some(ref path) = self.api_key_file else {
            return Ok(None);
        };
        if path.as_os_str().is_empty() {
            return Ok(None);
        }

        let md = fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to access api_key_file {:?}: {e}", path))?;

        if md.file_type().is_symlink() {
            return Err(format!(
                "api_key_file {:?} is a symlink, which is prohibited for secret tokens",
                path
            ));
        }
        if !md.is_file() {
            return Err(format!("api_key_file {:?} is not a regular file", path));
        }

        #[cfg(unix)]
        {
            if md.uid() != crate::secure_fs::current_euid() {
                return Err(format!(
                    "api_key_file {:?} is owned by UID {}, expected current UID {}",
                    path,
                    md.uid(),
                    crate::secure_fs::current_euid()
                ));
            }
            if md.mode() & 0o077 != 0 {
                return Err(format!(
                    "api_key_file {:?} has unsafe permissions {:o}; must be chmod 0600",
                    path,
                    md.mode() & 0o777
                ));
            }
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read api_key_file {:?}: {e}", path))?;
        let trimmed = content.trim().to_string();
        if trimmed.is_empty() {
            return Ok(None);
        }
        Ok(Some(trimmed))
    }

    /// Returns bounded recording duration in seconds (clamped to 1..=300).
    pub fn effective_max_recording_secs(&self) -> u64 {
        self.max_recording_secs.clamp(1, 300)
    }
}
