//! Pluggable Local AI STT Providers (`STT-01`).
//!
//! Provides zero-external-dependency async HTTP/1.1 multipart client for OpenAI-compatible
//! servers (whisper.cpp, speaches, faster-whisper-server, LocalAI, LM Studio), Ollama JSON client,
//! local CLI subprocess runner (whisper-cli), and deterministic mock provider.

use std::time::Instant;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::config::SttConfig;
use crate::traffic::{Origin, Role, TrafficMeter, TrafficTap};

static TRAFFIC_METER: std::sync::Mutex<Option<TrafficMeter>> = std::sync::Mutex::new(None);

/// Installs the meter used to account STT request/response bytes (Phase 64, hook H33).
pub fn set_traffic_meter(meter: Option<TrafficMeter>) {
    if let Ok(mut g) = TRAFFIC_METER.lock() {
        *g = meter;
    }
}

/// Opens one `Origin::Stt{host}` channel for a request (`None` when no meter is installed).
fn stt_tap(host: &str) -> Option<TrafficTap> {
    let m = TRAFFIC_METER.lock().ok()?.clone()?;
    Some(m.open_channel(Origin::Stt { host: host.to_string() }, host, None, Role::Wire))
}
use super::wav::ScrubbedTempWav;
use super::{SttBackendKind, TranscriptionRequest, TranscriptionResponse};

/// Core abstraction for Speech-to-Text inference engines.
pub trait SttProvider: Send + Sync {
    /// Transcribes a 16 kHz mono WAV payload into text.
    fn transcribe(
        &self,
        req: &TranscriptionRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TranscriptionResponse, String>> + Send>>;
}

/// Helper to parse simple JSON string value for a given key, e.g. `"text": "hello"`.
pub fn extract_json_string_field(json: &str, field_name: &str) -> Option<String> {
    let key_pattern = format!("\"{}\"", field_name);
    let key_pos = json.find(&key_pattern)?;
    let after_key = &json[key_pos + key_pattern.len()..];

    // Find colon
    let colon_pos = after_key.find(':')?;
    let after_colon = after_key[colon_pos + 1..].trim_start();

    // Must start with quote
    if !after_colon.starts_with('"') {
        return None;
    }
    let value_str = &after_colon[1..];

    // Scan until unescaped quote
    let mut result = String::new();
    let mut chars = value_str.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            return Some(result);
        } else if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    '/' => result.push('/'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'b' => result.push('\x08'),
                    'f' => result.push('\x0c'),
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if let Some(h) = chars.next() {
                                hex.push(h);
                            }
                        }
                        if let Ok(cp) = u32::from_str_radix(&hex, 16)
                            && let Some(c) = char::from_u32(cp)
                        {
                            result.push(c);
                        }
                    }
                    other => {
                        result.push('\\');
                        result.push(other);
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    None
}

/// Helper to encode binary bytes into standard base64 without external crates.
pub fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Parses URL into `(host, port, path)`.
fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let without_proto = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let (host_port, path) = match without_proto.find('/') {
        Some(idx) => (&without_proto[..idx], &without_proto[idx..]),
        None => (without_proto, "/"),
    };

    let (host, port) = if host_port.starts_with('[') {
        if let Some(end) = host_port.find(']') {
            let h = &host_port[1..end];
            let rest = &host_port[end + 1..];
            let p = if let Some(p_str) = rest.strip_prefix(':') {
                p_str.parse::<u16>().map_err(|e| format!("Invalid port: {e}"))?
            } else {
                80
            };
            (h.to_string(), p)
        } else {
            return Err("Malformed IPv6 host".to_string());
        }
    } else if let Some((h, p_str)) = host_port.split_once(':') {
        let p = p_str.parse::<u16>().map_err(|e| format!("Invalid port: {e}"))?;
        (h.to_string(), p)
    } else {
        (host_port.to_string(), 80)
    };

    Ok((host, port, path.to_string()))
}

/// OpenAI-compatible STT Provider (`POST /v1/audio/transcriptions`).
pub struct OpenAiCompatSttProvider {
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl OpenAiCompatSttProvider {
    pub fn new(endpoint: String, model: String, api_key: Option<String>) -> Self {
        Self {
            endpoint,
            model,
            api_key,
        }
    }
}

impl SttProvider for OpenAiCompatSttProvider {
    fn transcribe(
        &self,
        req: &TranscriptionRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TranscriptionResponse, String>> + Send>> {
        let endpoint = self.endpoint.clone();
        let default_model = self.model.clone();
        let api_key = self.api_key.clone();
        let req = req.clone();
        Box::pin(async move {
            let start = Instant::now();
            let (host, port, path) = parse_http_url(&endpoint)?;

            let boundary = format!("------------------------bilterm{}", crate::secure_fs::unique_suffix());

            let mut body: Vec<u8> = Vec::new();

            // Part 1: model
            let model_name = if !req.model.is_empty() {
                &req.model
            } else {
                &default_model
            };
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
            body.extend_from_slice(model_name.as_bytes());
            body.extend_from_slice(b"\r\n");

            // Part 2: response_format = json
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
            body.extend_from_slice(b"json\r\n");

            // Part 3: language (optional)
            if let Some(ref lang) = req.language {
                body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                body.extend_from_slice(b"Content-Disposition: form-data; name=\"language\"\r\n\r\n");
                body.extend_from_slice(lang.as_bytes());
                body.extend_from_slice(b"\r\n");
            }

            // Part 4: prompt / vocabulary hint (optional)
            if let Some(ref prompt) = req.vocabulary_prompt {
                body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                body.extend_from_slice(b"Content-Disposition: form-data; name=\"prompt\"\r\n\r\n");
                body.extend_from_slice(prompt.as_bytes());
                body.extend_from_slice(b"\r\n");
            }

            // Part 5: file (audio.wav)
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                b"Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n",
            );
            body.extend_from_slice(&req.wav_bytes);
            body.extend_from_slice(b"\r\n");

            // Closing boundary
            body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

            // Build HTTP headers
            let mut request = Vec::new();
            request.extend_from_slice(format!("POST {path} HTTP/1.1\r\n").as_bytes());
            request.extend_from_slice(format!("Host: {host}:{port}\r\n").as_bytes());
            request.extend_from_slice(
                format!("Content-Type: multipart/form-data; boundary={boundary}\r\n").as_bytes(),
            );
            request.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
            request.extend_from_slice(b"Connection: close\r\n");
            request.extend_from_slice(b"Accept: application/json, text/plain\r\n");

            if let Some(ref key) = api_key {
                request.extend_from_slice(format!("Authorization: Bearer {key}\r\n").as_bytes());
            }
            request.extend_from_slice(b"\r\n");
            request.extend_from_slice(&body);

            // Connect & send
            let mut stream = TcpStream::connect((host.as_str(), port))
                .await
                .map_err(|e| format!("Failed to connect to STT server at {host}:{port}: {e}"))?;

            let tap = stt_tap(&host);
            stream
                .write_all(&request)
                .await
                .map_err(|e| format!("Failed to write HTTP request: {e}"))?;
            if let Some(t) = &tap {
                t.tx(request.len());
            }

            // Read response (bounded 64 KiB, SEC-R01)
            let mut response_bytes = Vec::new();
            let mut reader = stream.take(65536);
            reader
                .read_to_end(&mut response_bytes)
                .await
                .map_err(|e| format!("Failed to read HTTP response: {e}"))?;
            if let Some(t) = &tap {
                t.rx(response_bytes.len());
            }

            let response_str = String::from_utf8_lossy(&response_bytes);

            // Check HTTP status code
            let first_line = response_str.lines().next().unwrap_or("");
            if !first_line.contains("200") {
                return Err(format!("STT server returned error status: {first_line}"));
            }

            // Split body from headers
            let body_str = if let Some(idx) = response_str.find("\r\n\r\n") {
                &response_str[idx + 4..]
            } else if let Some(idx) = response_str.find("\n\n") {
                &response_str[idx + 2..]
            } else {
                &response_str
            };

            // Extract transcript text
            let text = if let Some(extracted) = extract_json_string_field(body_str, "text") {
                extracted
            } else {
                body_str.trim().to_string()
            };

            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(TranscriptionResponse {
                text,
                provider: SttBackendKind::OpenAiCompatible,
                duration_ms,
            })
        })
    }
}

/// Ollama STT / multimodal audio provider (`POST /api/chat` or `POST /api/generate`).
pub struct OllamaSttProvider {
    pub endpoint: String,
    pub model: String,
}

impl OllamaSttProvider {
    pub fn new(endpoint: String, model: String) -> Self {
        Self { endpoint, model }
    }
}

impl SttProvider for OllamaSttProvider {
    fn transcribe(
        &self,
        req: &TranscriptionRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TranscriptionResponse, String>> + Send>> {
        let endpoint = self.endpoint.clone();
        let model = self.model.clone();
        let req = req.clone();
        Box::pin(async move {
            let start = Instant::now();
            let (host, port, path) = parse_http_url(&endpoint)?;
            let b64_audio = base64_encode(&req.wav_bytes);

            let prompt_text = req
                .vocabulary_prompt
                .as_deref()
                .unwrap_or("Transcribe the speech in this audio exactly into text without commentary.");

            // Format JSON payload for Ollama /api/generate
            let json_body = format!(
                r#"{{"model":"{}","prompt":"{}","images":["{}"],"stream":false}}"#,
                model, prompt_text, b64_audio
            );

            let mut request = Vec::new();
            request.extend_from_slice(format!("POST {path} HTTP/1.1\r\n").as_bytes());
            request.extend_from_slice(format!("Host: {host}:{port}\r\n").as_bytes());
            request.extend_from_slice(b"Content-Type: application/json\r\n");
            request.extend_from_slice(format!("Content-Length: {}\r\n", json_body.len()).as_bytes());
            request.extend_from_slice(b"Connection: close\r\n\r\n");
            request.extend_from_slice(json_body.as_bytes());

            let mut stream = TcpStream::connect((host.as_str(), port))
                .await
                .map_err(|e| format!("Failed to connect to Ollama server at {host}:{port}: {e}"))?;

            let tap = stt_tap(&host);
            stream
                .write_all(&request)
                .await
                .map_err(|e| format!("Failed to send request to Ollama: {e}"))?;
            if let Some(t) = &tap {
                t.tx(request.len());
            }

            let mut response_bytes = Vec::new();
            let mut reader = stream.take(65536);
            reader
                .read_to_end(&mut response_bytes)
                .await
                .map_err(|e| format!("Failed to read Ollama response: {e}"))?;
            if let Some(t) = &tap {
                t.rx(response_bytes.len());
            }

            let response_str = String::from_utf8_lossy(&response_bytes);
            let first_line = response_str.lines().next().unwrap_or("");
            if !first_line.contains("200") {
                return Err(format!("Ollama server returned error: {first_line}"));
            }

            let body_str = if let Some(idx) = response_str.find("\r\n\r\n") {
                &response_str[idx + 4..]
            } else {
                &response_str
            };

            let text = extract_json_string_field(body_str, "response")
                .or_else(|| extract_json_string_field(body_str, "content"))
                .unwrap_or_else(|| body_str.trim().to_string());

            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(TranscriptionResponse {
                text,
                provider: SttBackendKind::Ollama,
                duration_ms,
            })
        })
    }
}

/// Local CLI binary STT provider (invokes `whisper-cli` or custom shell command with `{wav_path}`).
pub struct LocalCliSttProvider {
    pub command_template: Vec<String>,
}

impl LocalCliSttProvider {
    pub fn new(command_template: Vec<String>) -> Self {
        Self { command_template }
    }
}

impl SttProvider for LocalCliSttProvider {
    fn transcribe(
        &self,
        req: &TranscriptionRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TranscriptionResponse, String>> + Send>> {
        let command_template = self.command_template.clone();
        let req = req.clone();
        Box::pin(async move {
            let start = Instant::now();
            let temp_wav = ScrubbedTempWav::create(&req.wav_bytes)?;

            if command_template.is_empty() {
                return Err("LocalCli command template is empty".to_string());
            }

            let wav_str = temp_wav.path().to_string_lossy().into_owned();
            let program = &command_template[0];
            let mut args = Vec::new();
            for arg in &command_template[1..] {
                if arg.contains("{wav_path}") {
                    args.push(arg.replace("{wav_path}", &wav_str));
                } else {
                    args.push(arg.clone());
                }
            }

            let output = tokio::process::Command::new(program)
                .args(&args)
                .output()
                .await
                .map_err(|e| format!("Failed to execute local STT CLI command '{program}': {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Local STT CLI failed with exit code {:?}: {stderr}", output.status.code()));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let cleaned = stdout.trim().to_string();

            // temp_wav automatically unlinks and zeroes out on drop here
            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(TranscriptionResponse {
                text: cleaned,
                provider: SttBackendKind::LocalCli,
                duration_ms,
            })
        })
    }
}

/// Deterministic mock STT provider for unit and integration testing.
pub struct MockSttProvider {
    pub response_text: String,
}

impl MockSttProvider {
    pub fn new(response_text: String) -> Self {
        Self { response_text }
    }
}

impl SttProvider for MockSttProvider {
    fn transcribe(
        &self,
        _req: &TranscriptionRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<TranscriptionResponse, String>> + Send>> {
        let text = self.response_text.clone();
        Box::pin(async move {
            Ok(TranscriptionResponse {
                text,
                provider: SttBackendKind::Mock,
                duration_ms: 10,
            })
        })
    }
}

/// Builds an active `SttProvider` instance from `SttConfig`.
pub fn create_stt_provider(config: &SttConfig) -> Result<Box<dyn SttProvider>, String> {
    config.validate_endpoint()?;
    let api_key = config.load_api_key()?;

    match config.backend_kind() {
        SttBackendKind::OpenAiCompatible => Ok(Box::new(OpenAiCompatSttProvider::new(
            config.endpoint.clone(),
            config.model.clone(),
            api_key,
        ))),
        SttBackendKind::Ollama => Ok(Box::new(OllamaSttProvider::new(
            config.endpoint.clone(),
            config.model.clone(),
        ))),
        SttBackendKind::LocalCli => Ok(Box::new(LocalCliSttProvider::new(config.cli_command.clone()))),
        SttBackendKind::Mock => Ok(Box::new(MockSttProvider::new("cargo test -q".to_string()))),
    }
}
