//! Background Tokio worker coordinating audio capture, VAD silence auto-stop, and STT inference (`STT-03`).

use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio::time::sleep;

use super::audio::{AudioCaptureBackend, AudioRecorderKind, AudioStreamHandle, MockAudioCapture, SubprocessAudioCapture, probe_available_recorder};
use super::config::SttConfig;
use super::provider::{MockSttProvider, SttProvider, create_stt_provider};
use super::wav::{compute_rms_dbfs, encode_pcm_bytes_to_wav};
use super::{SttBackendKind, SttEvent, TranscriptionRequest};

/// Commands sent from the WindowManager / UI loop into the background `SttWorker`.
#[derive(Debug)]
pub enum SttCommand {
    /// Starts microphone recording targeting `pane_id` (or focused pane).
    Start { pane_id: Option<usize> },
    /// Stops recording and dispatches captured audio to the active STT provider.
    Stop,
    /// Aborts active recording and discards audio without transcribing.
    Cancel,
    /// Directly injects a WAV buffer for deterministic headless testing or external audio pipelines.
    InjectWav { pane_id: Option<usize>, wav_bytes: Vec<u8> },
    /// Switches the active STT provider kind at runtime.
    SetProvider(SttBackendKind),
    /// Clean shutdown of the worker task.
    Shutdown,
}

/// Thread-safe handle for sending commands to the background `SttWorker`.
#[derive(Debug)]
pub struct SttHandle {
    cmd_tx: mpsc::UnboundedSender<SttCommand>,
    event_rx: Option<mpsc::UnboundedReceiver<SttEvent>>,
}

impl SttHandle {
    pub fn new(cmd_tx: mpsc::UnboundedSender<SttCommand>) -> Self {
        Self {
            cmd_tx,
            event_rx: None,
        }
    }

    pub fn with_receiver(
        cmd_tx: mpsc::UnboundedSender<SttCommand>,
        event_rx: mpsc::UnboundedReceiver<SttEvent>,
    ) -> Self {
        Self {
            cmd_tx,
            event_rx: Some(event_rx),
        }
    }

    pub fn set_event_rx(&mut self, event_rx: mpsc::UnboundedReceiver<SttEvent>) {
        self.event_rx = Some(event_rx);
    }

    pub fn try_recv_event(&mut self) -> Option<SttEvent> {
        self.event_rx.as_mut().and_then(|rx| rx.try_recv().ok())
    }

    pub fn start(&self, pane_id: Option<usize>) {
        let _ = self.cmd_tx.send(SttCommand::Start { pane_id });
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(SttCommand::Stop);
    }

    pub fn cancel(&self) {
        let _ = self.cmd_tx.send(SttCommand::Cancel);
    }

    pub fn inject_wav(&self, pane_id: Option<usize>, wav_bytes: Vec<u8>) {
        let _ = self.cmd_tx.send(SttCommand::InjectWav { pane_id, wav_bytes });
    }

    pub fn set_provider(&self, provider: SttBackendKind) {
        let _ = self.cmd_tx.send(SttCommand::SetProvider(provider));
    }

    pub fn shutdown(&self) {
        let _ = self.cmd_tx.send(SttCommand::Shutdown);
    }
}

/// Asynchronous background STT worker managing audio capture and inference.
pub struct SttWorker {
    config: SttConfig,
    event_tx: mpsc::UnboundedSender<SttEvent>,
    cmd_rx: mpsc::UnboundedReceiver<SttCommand>,
    active_provider: Box<dyn SttProvider>,
    active_provider_kind: SttBackendKind,
}

impl SttWorker {
    /// Spawns the background worker task and returns an `SttHandle`.
    pub fn spawn(config: SttConfig, event_tx: mpsc::UnboundedSender<SttEvent>) -> SttHandle {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let provider_kind = config.backend_kind();
        let provider: Box<dyn SttProvider> = match create_stt_provider(&config) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to initialize configured STT provider: {e}; falling back to Mock");
                Box::new(MockSttProvider::new("cargo test -q".to_string()))
            }
        };

        let mut worker = Self {
            config,
            event_tx,
            cmd_rx,
            active_provider: provider,
            active_provider_kind: provider_kind,
        };

        let runner = async move {
            worker.run().await;
        };
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(runner);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(runner);
                }
            });
        }

        SttHandle::new(cmd_tx)
    }

    /// Spawns the background worker task and bundles the event receiver into `SttHandle`.
    pub fn spawn_autonomous(config: SttConfig) -> SttHandle {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let provider_kind = config.backend_kind();
        let provider: Box<dyn SttProvider> = match create_stt_provider(&config) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to initialize configured STT provider: {e}; falling back to Mock");
                Box::new(MockSttProvider::new("cargo test -q".to_string()))
            }
        };

        let mut worker = Self {
            config,
            event_tx,
            cmd_rx,
            active_provider: provider,
            active_provider_kind: provider_kind,
        };

        let runner = async move {
            worker.run().await;
        };
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(runner);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(runner);
                }
            });
        }

        SttHandle::with_receiver(cmd_tx, event_rx)
    }

    async fn run(&mut self) {
        let mut recording = false;
        let mut target_pane_id: Option<usize> = None;
        let mut active_stream: Option<Box<dyn AudioStreamHandle>> = None;
        let mut pcm_buffer: Vec<u8> = Vec::new();
        let mut record_start: Option<Instant> = None;
        let mut last_speech_time: Option<Instant> = None;
        let mut had_speech = false;

        loop {
            // If recording, poll stream with 100 ms timeout so we can process chunks and VAD
            if recording {
                tokio::select! {
                    cmd_opt = self.cmd_rx.recv() => {
                        let Some(cmd) = cmd_opt else { break; };
                        match cmd {
                            SttCommand::Stop => {
                                if let Some(mut stream) = active_stream.take() {
                                    let _ = stream.stop();
                                }
                                recording = false;
                                let elapsed = record_start.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);
                                self.emit_telemetry(false, true, -96, elapsed, None);
                                self.transcribe_buffer(target_pane_id, std::mem::take(&mut pcm_buffer)).await;
                                self.emit_telemetry(false, false, -96, 0, None);
                            }
                            SttCommand::Cancel => {
                                if let Some(mut stream) = active_stream.take() {
                                    let _ = stream.stop();
                                }
                                recording = false;
                                pcm_buffer.clear();
                                record_start = None;
                                last_speech_time = None;
                                had_speech = false;
                                self.emit_telemetry(false, false, -96, 0, None);
                            }
                            SttCommand::Start { pane_id } => {
                                // Already recording; just update target pane if specified
                                if pane_id.is_some() {
                                    target_pane_id = pane_id;
                                }
                            }
                            SttCommand::InjectWav { pane_id, wav_bytes } => {
                                self.emit_telemetry(false, true, -96, 0, None);
                                self.transcribe_wav(pane_id, wav_bytes).await;
                                self.emit_telemetry(false, false, -96, 0, None);
                            }
                            SttCommand::SetProvider(kind) => {
                                self.update_provider(kind);
                            }
                            SttCommand::Shutdown => {
                                if let Some(mut stream) = active_stream.take() {
                                    let _ = stream.stop();
                                }
                                break;
                            }
                        }
                    }
                    _ = sleep(Duration::from_millis(50)) => {
                        if let Some(ref mut stream) = active_stream {
                            match stream.next_chunk() {
                                Ok(Some(chunk)) => {
                                    let mut samples = Vec::with_capacity(chunk.len() / 2);
                                    for c in chunk.chunks_exact(2) {
                                        samples.push(i16::from_le_bytes([c[0], c[1]]));
                                    }
                                    let rms_db = compute_rms_dbfs(&samples);
                                    pcm_buffer.extend_from_slice(&chunk);

                                    let elapsed = record_start.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);

                                    // VAD logic
                                    if rms_db >= self.config.vad_threshold_db {
                                        had_speech = true;
                                        last_speech_time = Some(Instant::now());
                                    } else if had_speech
                                        && self.config.auto_stop_silence_ms > 0
                                        && let Some(last_speech) = last_speech_time
                                        && last_speech.elapsed().as_millis()
                                            >= self.config.auto_stop_silence_ms as u128
                                    {
                                        // VAD silence auto-stop triggered!
                                        let _ = stream.stop();
                                        active_stream = None;
                                        recording = false;
                                        self.emit_telemetry(false, true, -96, elapsed, None);
                                        self.transcribe_buffer(target_pane_id, std::mem::take(&mut pcm_buffer)).await;
                                        self.emit_telemetry(false, false, -96, 0, None);
                                        continue;
                                    }

                                    // Max recording duration cap
                                    let max_ms = self.config.effective_max_recording_secs() * 1000;
                                    if elapsed >= max_ms {
                                        let _ = stream.stop();
                                        active_stream = None;
                                        recording = false;
                                        self.emit_telemetry(false, true, -96, elapsed, None);
                                        self.transcribe_buffer(target_pane_id, std::mem::take(&mut pcm_buffer)).await;
                                        self.emit_telemetry(false, false, -96, 0, None);
                                        continue;
                                    }

                                    self.emit_telemetry(true, false, rms_db as i32, elapsed, None);
                                }
                                Ok(None) => {
                                    // Audio stream closed by OS or EOF
                                    let _ = stream.stop();
                                    active_stream = None;
                                    recording = false;
                                    let elapsed = record_start.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);
                                    self.emit_telemetry(false, true, -96, elapsed, None);
                                    self.transcribe_buffer(target_pane_id, std::mem::take(&mut pcm_buffer)).await;
                                    self.emit_telemetry(false, false, -96, 0, None);
                                }
                                Err(e) => {
                                    let _ = stream.stop();
                                    active_stream = None;
                                    recording = false;
                                    self.emit_telemetry(false, false, -96, 0, Some(e.clone()));
                                    let _ = self.event_tx.send(SttEvent::Error(e));
                                }
                            }
                        }
                    }
                }
            } else {
                // Idle / transcribing: wait for commands
                let Some(cmd) = self.cmd_rx.recv().await else {
                    break;
                };
                match cmd {
                    SttCommand::Start { pane_id } => {
                        target_pane_id = pane_id;
                        pcm_buffer.clear();
                        record_start = Some(Instant::now());
                        last_speech_time = None;
                        had_speech = false;

                        // Start audio capture backend
                        let stream_res = if self.config.audio_backend == "mock" || self.active_provider_kind == SttBackendKind::Mock {
                            MockAudioCapture::new(20, 20).start_recording(self.config.effective_max_recording_secs())
                        } else {
                            match probe_available_recorder(&self.config.audio_backend) {
                                Ok(AudioRecorderKind::Mock) => {
                                    MockAudioCapture::new(20, 20).start_recording(self.config.effective_max_recording_secs())
                                }
                                Ok(kind) => {
                                    SubprocessAudioCapture::new(kind).start_recording(self.config.effective_max_recording_secs())
                                }
                                Err(e) => Err(e),
                            }
                        };

                        match stream_res {
                            Ok(stream) => {
                                active_stream = Some(stream);
                                recording = true;
                                self.emit_telemetry(true, false, -96, 0, None);
                            }
                            Err(e) => {
                                self.emit_telemetry(false, false, -96, 0, Some(e.clone()));
                                let _ = self.event_tx.send(SttEvent::Error(e));
                            }
                        }
                    }
                    SttCommand::Stop | SttCommand::Cancel => {
                        // Already stopped
                        self.emit_telemetry(false, false, -96, 0, None);
                    }
                    SttCommand::InjectWav { pane_id, wav_bytes } => {
                        self.emit_telemetry(false, true, -96, 0, None);
                        self.transcribe_wav(pane_id, wav_bytes).await;
                        self.emit_telemetry(false, false, -96, 0, None);
                    }
                    SttCommand::SetProvider(kind) => {
                        self.update_provider(kind);
                    }
                    SttCommand::Shutdown => break,
                }
            }
        }
    }

    fn emit_telemetry(&self, recording: bool, transcribing: bool, rms_db: i32, elapsed_ms: u64, error: Option<String>) {
        let _ = self.event_tx.send(SttEvent::Telemetry {
            recording,
            transcribing,
            rms_db,
            elapsed_ms,
            error,
        });
    }

    fn update_provider(&mut self, kind: SttBackendKind) {
        self.active_provider_kind = kind;
        self.config.provider = match kind {
            SttBackendKind::OpenAiCompatible => "openai_compatible".to_string(),
            SttBackendKind::Ollama => "ollama".to_string(),
            SttBackendKind::LocalCli => "local_cli".to_string(),
            SttBackendKind::Mock => "mock".to_string(),
        };
        if let Ok(p) = create_stt_provider(&self.config) {
            self.active_provider = p;
        }
    }

    async fn transcribe_buffer(&self, pane_id: Option<usize>, pcm: Vec<u8>) {
        if pcm.is_empty() {
            return;
        }
        let wav_bytes = encode_pcm_bytes_to_wav(&pcm);
        self.transcribe_wav(pane_id, wav_bytes).await;
    }

    async fn transcribe_wav(&self, pane_id: Option<usize>, wav_bytes: Vec<u8>) {
        let req = TranscriptionRequest {
            wav_bytes,
            model: self.config.model.clone(),
            language: self.config.language.clone(),
            vocabulary_prompt: self.config.vocabulary_prompt.clone(),
        };

        match self.active_provider.transcribe(&req).await {
            Ok(resp) => {
                let _ = self.event_tx.send(SttEvent::Transcript {
                    pane_id,
                    text: resp.text,
                    is_final: true,
                });
            }
            Err(e) => {
                let _ = self.event_tx.send(SttEvent::Error(e));
            }
        }
    }
}
