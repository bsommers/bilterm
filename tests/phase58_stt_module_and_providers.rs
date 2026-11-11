//! Phase 58 Wave 1 Integration Tests: STT Subsystem & Local AI Providers (`STT-01`, `STT-02`, `STT-03`).

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::sleep;

use bilterm::config::BiltermConfig;
use bilterm::stt::{
    AudioRecorderKind, LocalCliSttProvider, OllamaSttProvider, OpenAiCompatSttProvider,
    ScrubbedTempWav, SttBackendKind, SttConfig, SttEvent, SttProvider, SttWorker,
    TranscriptionRequest, compute_rms_dbfs, encode_wav_s16le_mono, parse_wav_s16le_mono,
    probe_available_recorder,
};

#[test]
fn test_stt_config_defaults_and_toml_parsing() {
    let cfg = BiltermConfig::default();
    assert!(!cfg.stt.enabled, "STT must be disabled by default");
    assert_eq!(cfg.stt.provider, "openai_compatible");
    assert_eq!(cfg.stt.endpoint, "http://127.0.0.1:8080/v1/audio/transcriptions");
    assert_eq!(cfg.stt.model, "whisper-1");
    assert_eq!(cfg.stt.language.as_deref(), Some("en"));
    assert_eq!(cfg.stt.max_recording_secs, 60);
    assert_eq!(cfg.stt.auto_stop_silence_ms, 1500);
    assert!(!cfg.stt.allow_remote_stt, "allow_remote_stt must be false by default");
    assert!(!cfg.stt.auto_submit, "auto_submit must be false by default");

    // Test parsing custom TOML section
    let toml_src = r#"
        [stt]
        enabled = true
        provider = "ollama"
        endpoint = "http://localhost:11434/api/generate"
        model = "qwen2.5-coder:1.5b"
        max_recording_secs = 30
        auto_stop_silence_ms = 1200
        auto_submit = true
    "#;
    let parsed: BiltermConfig = toml::from_str(toml_src).expect("valid stt toml");
    assert!(parsed.stt.enabled);
    assert_eq!(parsed.stt.backend_kind(), SttBackendKind::Ollama);
    assert_eq!(parsed.stt.endpoint, "http://localhost:11434/api/generate");
    assert_eq!(parsed.stt.model, "qwen2.5-coder:1.5b");
    assert_eq!(parsed.stt.max_recording_secs, 30);
    assert_eq!(parsed.stt.auto_stop_silence_ms, 1200);
    assert!(parsed.stt.auto_submit);
}

#[test]
fn test_stt_endpoint_validation_security() {
    let mut config = SttConfig {
        endpoint: "http://127.0.0.1:8080/v1/audio/transcriptions".to_string(),
        ..Default::default()
    };
    assert!(config.validate_endpoint().is_ok());

    config.endpoint = "http://localhost:11434/api/generate".to_string();
    assert!(config.validate_endpoint().is_ok());

    config.endpoint = "http://[::1]:8080/v1/audio/transcriptions".to_string();
    assert!(config.validate_endpoint().is_ok());

    config.endpoint = "unix:///run/user/1000/whisper.sock".to_string();
    assert!(config.validate_endpoint().is_ok());

    // Non-loopback endpoints must be rejected by default (SEC-R04)
    config.endpoint = "http://api.openai.com/v1/audio/transcriptions".to_string();
    let err = config.validate_endpoint().unwrap_err();
    assert!(err.contains("non-loopback"));

    config.endpoint = "http://192.168.1.50:8080/v1/audio/transcriptions".to_string();
    assert!(config.validate_endpoint().is_err());

    // Opt-in allow_remote_stt must permit remote endpoints
    config.allow_remote_stt = true;
    assert!(config.validate_endpoint().is_ok());
}

#[test]
fn test_stt_api_key_loading_security() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let key_file = temp_dir.path().join("secret.token");

    // Create 0600 file
    let mut file = bilterm::secure_fs::create_private_file(&key_file).expect("private file");
    file.write_all(b"sk-test-local-token-1234567890\n").expect("write");
    drop(file);

    let config = SttConfig {
        api_key_file: Some(key_file.clone()),
        ..Default::default()
    };

    let loaded = config.load_api_key().expect("load ok");
    assert_eq!(loaded.as_deref(), Some("sk-test-local-token-1234567890"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Chmod to 0644 (world readable) must fail
        fs::set_permissions(&key_file, fs::Permissions::from_mode(0o644)).expect("chmod");
        let err = config.load_api_key().unwrap_err();
        assert!(err.contains("chmod 0600 required") || err.contains("unsafe permissions"));
    }
}

#[test]
fn test_wav_encoding_and_parsing_round_trip() {
    // Generate 1600 samples (100 ms at 16 kHz) of 440 Hz sine wave
    let mut original_samples = Vec::with_capacity(1600);
    for i in 0..1600 {
        let val = (16000.0 * (2.0 * std::f64::consts::PI * 440.0 * (i as f64) / 16000.0).sin()) as i16;
        original_samples.push(val);
    }

    let wav_bytes = encode_wav_s16le_mono(&original_samples);
    assert_eq!(wav_bytes.len(), 44 + 1600 * 2);
    assert_eq!(&wav_bytes[0..4], b"RIFF");
    assert_eq!(&wav_bytes[8..12], b"WAVE");

    let parsed_samples = parse_wav_s16le_mono(&wav_bytes).expect("parse ok");
    assert_eq!(parsed_samples.len(), original_samples.len());
    assert_eq!(parsed_samples, original_samples);
}

#[test]
fn test_compute_rms_dbfs() {
    // Silence must yield -96 dBFS floor
    let silence = vec![0i16; 1600];
    assert_eq!(compute_rms_dbfs(&silence), -96.0);

    // Full-scale square wave must yield ~ 0 dBFS
    let square_wave = vec![32767i16; 1600];
    let rms_full = compute_rms_dbfs(&square_wave);
    assert!((-0.1..=0.0).contains(&rms_full), "rms_full = {rms_full}");

    // Moderate amplitude sine wave (~16000 peak) gives ~ -6 dBFS
    let mut sine_samples = Vec::with_capacity(1600);
    for i in 0..1600 {
        let val = (16000.0 * (2.0 * std::f64::consts::PI * 440.0 * (i as f64) / 16000.0).sin()) as i16;
        sine_samples.push(val);
    }
    let rms_sine = compute_rms_dbfs(&sine_samples);
    assert!((-10.0..=-5.0).contains(&rms_sine), "rms_sine = {rms_sine}");
}

#[test]
fn test_scrubbed_temp_wav_lifecycle() {
    let samples = vec![1000i16, 2000, 3000];
    let wav_bytes = encode_wav_s16le_mono(&samples);

    let file_path: PathBuf;
    {
        let temp_wav = ScrubbedTempWav::create(&wav_bytes).expect("create temp wav");
        file_path = temp_wav.path().to_path_buf();
        assert!(file_path.exists(), "temp WAV file must exist while guard is active");

        let read_bytes = fs::read(&file_path).expect("read");
        assert_eq!(read_bytes, wav_bytes);
    }

    // After drop, file must be zeroed and unlinked
    assert!(!file_path.exists(), "temp WAV file must be unlinked on drop");
}

#[test]
fn test_audio_recorder_probing() {
    // Mock backend always resolves
    let mock = probe_available_recorder("mock").expect("mock probe");
    assert_eq!(mock, AudioRecorderKind::Mock);

    // Non-existent backend must return clear error
    let bad = probe_available_recorder("non-existent-rec-tool-999");
    assert!(bad.is_err());
}

#[tokio::test]
async fn test_openai_compat_multipart_transcription() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock tcp");
    let port = listener.local_addr().expect("addr").port();
    let endpoint = format!("http://127.0.0.1:{port}/v1/audio/transcriptions");

    // Spawn mock server task
    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut req_buf = vec![0u8; 8192];
            let n = stream.read(&mut req_buf).await.expect("read");
            let req_str = String::from_utf8_lossy(&req_buf[..n]);

            // Verify multipart request structure
            assert!(req_str.starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
            assert!(req_str.contains("Content-Type: multipart/form-data; boundary="));
            assert!(req_str.contains("Authorization: Bearer test-secret-token"));
            assert!(req_str.contains("name=\"model\""));
            assert!(req_str.contains("whisper-1"));
            assert!(req_str.contains("filename=\"audio.wav\""));

            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"text\": \"cargo test -q\"}";
            stream.write_all(resp.as_bytes()).await.expect("write");
        }
    });

    let provider = OpenAiCompatSttProvider::new(
        endpoint,
        "whisper-1".to_string(),
        Some("test-secret-token".to_string()),
    );

    let samples = vec![500i16; 1600];
    let req = TranscriptionRequest {
        wav_bytes: encode_wav_s16le_mono(&samples),
        model: "whisper-1".to_string(),
        language: Some("en".to_string()),
        vocabulary_prompt: Some("cargo, rustc".to_string()),
    };

    let resp = provider.transcribe(&req).await.expect("transcribe ok");
    assert_eq!(resp.text, "cargo test -q");
    assert_eq!(resp.provider, SttBackendKind::OpenAiCompatible);
}

#[tokio::test]
async fn test_ollama_stt_transcription() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock ollama");
    let port = listener.local_addr().expect("addr").port();
    let endpoint = format!("http://127.0.0.1:{port}/api/generate");

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut req_buf = vec![0u8; 16384];
            let n = stream.read(&mut req_buf).await.expect("read");
            let req_str = String::from_utf8_lossy(&req_buf[..n]);

            assert!(req_str.starts_with("POST /api/generate HTTP/1.1"));
            assert!(req_str.contains("\"model\":\"qwen2.5-coder\""));
            assert!(req_str.contains("\"images\":[\""));

            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"response\": \"git status -sb\"}";
            stream.write_all(resp.as_bytes()).await.expect("write");
        }
    });

    let provider = OllamaSttProvider::new(endpoint, "qwen2.5-coder".to_string());
    let req = TranscriptionRequest {
        wav_bytes: encode_wav_s16le_mono(&[100i16; 800]),
        model: "qwen2.5-coder".to_string(),
        language: None,
        vocabulary_prompt: None,
    };

    let resp = provider.transcribe(&req).await.expect("transcribe ok");
    assert_eq!(resp.text, "git status -sb");
    assert_eq!(resp.provider, SttBackendKind::Ollama);
}

#[tokio::test]
#[cfg_attr(windows, ignore = "drives a POSIX /bin/sh child; the Windows PTY path is covered by tests/smoke_binary_per_os.rs")]
async fn test_local_cli_stt_provider() {
    // Invoke a shell echo command simulating a whisper CLI output
    let cli = LocalCliSttProvider::new(vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "echo 'ls -la {wav_path}'".to_string(),
    ]);

    let req = TranscriptionRequest {
        wav_bytes: encode_wav_s16le_mono(&[50i16; 800]),
        model: "cli".to_string(),
        language: None,
        vocabulary_prompt: None,
    };

    let resp = cli.transcribe(&req).await.expect("transcribe ok");
    assert!(resp.text.starts_with("ls -la"));
    assert!(resp.text.contains("bilterm-stt-"));
    assert_eq!(resp.provider, SttBackendKind::LocalCli);
}

#[tokio::test]
async fn test_stt_worker_with_mock_audio_and_vad_auto_stop() {
    let config = SttConfig {
        enabled: true,
        provider: "mock".to_string(),
        audio_backend: "mock".to_string(),
        auto_stop_silence_ms: 200, // Fast auto-stop for testing
        vad_threshold_db: -38.0,
        ..Default::default()
    };

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let handle = SttWorker::spawn(config, event_tx);

    // Start recording targeting pane 42
    handle.start(Some(42));

    let mut saw_telemetry_recording = false;
    let mut saw_transcript = false;
    let mut transcript_text = String::new();

    let timeout = sleep(Duration::from_secs(3));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                break;
            }
            Some(event) = event_rx.recv() => {
                match event {
                    SttEvent::Telemetry { recording, .. } => {
                        if recording {
                            saw_telemetry_recording = true;
                        }
                    }
                    SttEvent::Transcript { pane_id, text, is_final } => {
                        assert_eq!(pane_id, Some(42));
                        assert!(is_final);
                        saw_transcript = true;
                        transcript_text = text;
                        break;
                    }
                    SttEvent::Error(e) => {
                        panic!("Unexpected STT worker error: {e}");
                    }
                }
            }
        }
    }

    assert!(saw_telemetry_recording, "Expected recording telemetry");
    assert!(saw_transcript, "Expected transcript after VAD auto-stop");
    assert_eq!(transcript_text, "cargo test -q");

    handle.shutdown();
}
