# Phase 58 Research Spike: Local AI Speech-to-Text (`STT`) Optional Module (`STT-01..03`)

**Date:** 2026-11-07  
**Status:** Complete  
**Requirements Covered:** `STT-01` (Pluggable Local AI STT Backends), `STT-02` (Zero-Bloat Cross-Platform Audio Capture), `STT-03` (Isolated Subsystem Seam, UX & Security Confinement)

---

## 1. Executive Summary

Bilterm's composite terminal pane architecture—featuring a dedicated multi-line `FloatingCommandPrompt` above a live PTY canvas—makes it uniquely suited for voice-driven terminal dictation and command synthesis. Unlike traditional terminal emulators where voice dictation blindly injects raw characters directly into `stdin` (risking accidental execution of destructive commands if an utterance contains a trailing newline or misrecognized token), Bilterm can stream or stage transcribed text directly inside the **focused pane's `FloatingCommandPrompt` buffer** for visual review, syntax highlighting, and editing prior to `CompositePane::submit_command()`.

To satisfy the core design constraint—**maximum modularity, local-first AI execution, and zero mandatory dependency bloat**—this research spike evaluates:
1. **Local AI STT Engines & Wire Protocols**: OpenAI-compatible `POST /v1/audio/transcriptions` (`whisper.cpp`, `speaches`/`faster-whisper-server`, `LocalAI`, `vLLM`, `LM Studio`, `mlx-audio`), Ollama local multimodal/audio endpoints, and local CLI subprocess runners (`whisper-cli`, `mlx_whisper`).
2. **Two-Tier Cross-Platform Audio Capture**:
   - **Tier 1 (Default Build, Zero Native C/ALSA/CoreAudio Link Dependencies)**: OS recorder subprocess pipe (`pw-record` / `parecord` / `arecord` on Linux, `rec` / `sox` / `ffmpeg -f avfoundation` on macOS, `ffmpeg -f dshow` / PowerShell `winmm.dll` helper on Windows) writing 16 kHz, 16-bit mono PCM/WAV into a `0600` private file/pipe via `src/secure_fs.rs`.
   - **Tier 2 (Opt-In Cargo Feature `--features stt-native-audio`)**: In-process `cpal` + `hound` ring-buffer capture eliminating subprocess spawn overhead (~15–30 ms).
3. **Strict Subsystem Isolation (`src/stt/`)**:
   - Communicating with `WindowManager` and the Tokio event loop exclusively through `Action::Voice(VoiceOp)` and an asynchronous `mpsc` event channel (`AppEvent::SttEvent(SttEvent)`).
   - Zero impact on startup time, binary size, or memory footprint when `[stt] enabled = false` (the default).

---

## 2. Local AI STT Backends & Protocol Evaluation

### 2.1 Protocol Matrix

| Backend / Engine | Primary Protocol / Endpoint | Audio Format | Streaming Support | Hardware Acceleration | Typical Latency (5s utterance) | VRAM / RAM Footprint |
|---|---|---|---|---|---|---|
| **`whisper.cpp` (`whisper-server`)** | `POST /v1/audio/transcriptions` or `POST /inference` (`multipart/form-data`) | 16kHz 16-bit mono WAV | Chunked VAD or batch WAV | CPU (AVX2/NEON), CUDA, Vulkan, Metal, CoreML | **120–350 ms** (`base.en` / `small.en` Q5) | **~180 MB – 600 MB** |
| **`speaches` (`faster-whisper-server`)** | `POST /v1/audio/transcriptions` + `WS /v1/audio/transcriptions` | WAV / raw PCM16 | **Yes** (WebSocket + SSE partial tokens) | CTranslate2 (CUDA, CPU INT8, ROCm) | **80–250 ms** (`distil-large-v3` / `small.en`) | **~300 MB – 1.5 GB** |
| **`mlx-audio` / `lightning-whisper-mlx`** | `POST /v1/audio/transcriptions` | WAV | Batch + chunked SSE | Apple Silicon Metal / AMX | **60–180 ms** (`distil-large-v3` 4-bit) | **~500 MB – 1.2 GB Unified** |
| **`LocalAI` / `vLLM` / `LM Studio`** | `POST /v1/audio/transcriptions` | WAV | Batch + SSE (`stream=true`) | CUDA, Metal, Vulkan, SYCL | **150–400 ms** | **~500 MB – 2 GB** |
| **Ollama (`/api/generate` / `/api/chat` or `v1`)** | `POST /api/chat` (base64 audio attachment for multimodal models) or OpenAI compat | WAV (base64 or multipart) | JSONL token stream | Ollama GPU/CPU runner | **350–900 ms** (multimodal LLM overhead) | **~2 GB – 8 GB** |
| **Local CLI (`whisper-cli` / `faster-whisper`)** | Subprocess `stdin`/`<temp.wav>` -> `stdout` JSON/text | 16kHz 16-bit mono WAV | Line-buffered stdout | Any local binary | **250–600 ms** (includes model load if non-daemon) | **~200 MB – 1 GB** |

### 2.2 Detailed Protocol Analysis

#### A. OpenAI-Compatible Audio Transcriptions (`POST /v1/audio/transcriptions`) — **Primary Recommended Standard**
Almost every local STT server in the open-source AI ecosystem has converged on the OpenAI `/v1/audio/transcriptions` multipart form endpoint:
- **Request**: `multipart/form-data` containing:
  - `file`: 16 kHz, 16-bit, mono PCM WAV binary payload (`audio/wav`)
  - `model`: e.g. `"whisper-1"`, `"Systran/faster-whisper-small.en"`, `"ggml-base.en.bin"`
  - `language`: optional ISO-639-1 code (e.g. `"en"`) to skip 30-second language-detection overhead
  - `prompt`: optional context hint (e.g. `"Linux shell commands, git, cargo, tmux, kubectl, rustc, flags, pipes"` — dramatically improves recognition of CLI flags like `--release`, `kubectl`, `chmod`)
  - `response_format`: `"json"` (`{"text": "..."}`) or `"text"`
  - `temperature`: `0.0` (deterministic decoding)
- **Zero-Dependency HTTP Implementation**:
  - Because `/v1/audio/transcriptions` is a straightforward HTTP/1.1 `POST` with a `multipart/form-data` boundary, Bilterm can implement the HTTP/1.1 client over `tokio::net::TcpStream` (for `http://127.0.0.1:<port>`) or Unix domain sockets (`tokio::net::UnixStream`) **without pulling in `reqwest`, `hyper`, or OpenSSL/rustls into the default dependency tree**.
  - For optional remote HTTPS endpoints (`allow_remote_stt = true`), either `curl` subprocess transport or an optional Cargo feature (`stt-tls`) can be used.

#### B. Ollama Native & Multimodal Audio Bridge (`POST /api/chat` / `POST /api/generate`)
Ollama supports local model execution on port `11434`:
- **Path 1 (OpenAI-Compatible Endpoint)**: Recent Ollama builds and Ollama proxies expose `/v1/audio/transcriptions` or `/v1/chat/completions` with `input_audio: { data: "<base64>", format: "wav" }`.
- **Path 2 (Native Ollama `/api/generate` or `/api/chat`)**: Multimodal audio-capable models hosted in Ollama accept base64-encoded WAV buffers and a system prompt (`"Transcribe the following audio verbatim into text with no commentary."`), returning newline-delimited JSON (`{"message":{"content":"..."},"done":false}`).
- **Post-Processing Optional Hook ("Voice-to-Command" Mode)**: When paired with Ollama (`qwen2.5-coder:1.5b` or `llama3.2:1b`), the user can optionally toggle `translate_to_shell = true` to convert natural language speech (*"find all rust files modified in the last day"*) into executable shell syntax (`find . -name "*.rs" -mtime -1`) staged inside the floating command prompt.

#### C. Local Subprocess CLI Provider (`SttBackendKind::LocalCli`)
For users who have `whisper-cli` (`whisper.cpp` binary), `mlx_whisper`, or a custom wrapper script installed locally without running a persistent HTTP server:
- Spawns the configured command (e.g. `whisper-cli -m ~/.local/share/whisper/ggml-base.en.bin -f <wav_path> --no-timestamps --nt`) via `tokio::process::Command`.
- Reads `stdout`, strips ANSI/timestamp artifacts, and emits the transcript.

#### D. Push-to-Talk / Toggle Batch WAV vs. Chunked Streaming VAD
- **Push-to-Talk / Toggle-to-Talk Batch WAV (Wave 1 + Wave 2 Core)**:
  - User presses the voice chord (`Ctrl+B v` / `Alt+V`, `:voice toggle`, or clicks `[🎙 MIC]` in the OIA bar) to start recording, speaks a command (typically 1.5–6 seconds), and presses the key/button again (or pauses) to stop and transcribe.
  - At 16 kHz 16-bit mono, 5 seconds of audio is only **160 KB** (`32 KB/sec`). Sending 160 KB over loopback `127.0.0.1` takes `<0.2 ms`, and local `whisper.cpp` / `faster-whisper` transcribes it in **80–250 ms**, yielding instantaneous perceived responsiveness without complex WebSocket state drift.
- **Chunked Rolling Window / VAD Auto-Stop (Wave 2 Enhancement)**:
  - While recording, the audio capture loop computes RMS energy (`sqrt(sum(sample^2) / N)`) on each 100 ms PCM frame (3,200 bytes).
  - Provides two immediate benefits with zero external VAD library:
    1. **Live VU Meter in OIA Bar**: Renders signal activity (`[🎙 REC ▂▄▆█ 02.4s]`) so the user immediately knows their microphone is live.
    2. **Silence Auto-Stop (VAD)**: When `auto_stop_silence_ms = 1200` is configured, once speech energy exceeds `vad_threshold_db` and then drops below threshold for `1200 ms`, recording automatically stops and triggers transcription hands-free.
    3. **Chunked Interim Streaming**: For utterances longer than `chunk_interval_ms` (e.g., `2500 ms`), completed speech segments between silence gaps can be dispatched asynchronously as `is_final: false` partial transcripts to preview text in real time.

---

## 3. Cross-Platform Audio Capture Without Mandatory Native Bloat

A major pitfall of adding audio capture to Rust CLI/TUI applications is unconditionally adding `cpal` or `alsa-sys` to `Cargo.toml`. On Linux, `alsa-sys` requires `libasound2-dev` and `pkg-config` at compile time, breaking hermetic `cargo build` in minimal containers or CI environments lacking ALSA headers.

### 3.1 Two-Tier Audio Capture Strategy

#### Tier 1: Zero-Link-Dependency Subprocess Recorder Pipe (Default Build)
Works out of the box with `default = []` features in `Cargo.toml` by auto-probing standard platform audio tools and streaming raw `S16_LE` 16 kHz mono PCM over `stdout` (or into a `0600` temp file):

| Platform | Probe Priority Order | Command Line Invoked (16 kHz, 16-bit signed LE, 1 channel raw PCM to stdout) |
|---|---|---|
| **Linux** | 1. `pw-record` (PipeWire)<br>2. `parecord` (PulseAudio)<br>3. `arecord` (ALSA)<br>4. `ffmpeg` | 1. `pw-record --format=s16 --rate=16000 --channels=1 -`<br>2. `parecord --format=s16le --rate=16000 --channels=1 --raw`<br>3. `arecord -q -f S16_LE -r 16000 -c 1 -t raw -`<br>4. `ffmpeg -loglevel quiet -f pulse -i default -ar 16000 -ac 1 -f s16le -` |
| **macOS** | 1. `rec` / `sox`<br>2. `ffmpeg` (AVFoundation) | 1. `rec -q -t raw -b 16 -e signed-integer -r 16000 -c 1 -`<br>2. `ffmpeg -loglevel quiet -f avfoundation -i ":default" -ar 16000 -ac 1 -f s16le -` |
| **Windows** | 1. `ffmpeg` (DirectShow / WASAPI)<br>2. `sox` | 1. `ffmpeg -loglevel quiet -f dshow -i audio="default" -ar 16000 -ac 1 -f s16le -`<br>2. `sox -d -t raw -b 16 -e signed-integer -r 16000 -c 1 -` |

**Why raw `s16le` PCM over `stdout` pipe is superior to letting the child write a WAV file directly:**
1. **In-Memory Ring Buffer + Zero-Touch WAV Framing**: Bilterm reads 3,200-byte chunks (100 ms of 16 kHz 16-bit mono PCM) directly from `child.stdout` in Tokio, computes real-time RMS energy for the OIA `[🎙 REC]` VU meter and silence-based VAD auto-stop, enforces a hard memory cap (`max_recording_secs`, default `60s` = `1.92 MB`, satisfying `bounding-untrusted-resource-use`), and prepends a deterministic 44-byte RIFF/WAVE header in memory before sending to HTTP `/v1/audio/transcriptions`.
2. **Clean Termination**: Stopping recording simply drops/kills the recorder child process and immediately flushes the already-captured PCM buffer—no risk of a truncated/corrupted WAV header written by a `SIGKILL`ed subprocess.
3. **Zero Disk Touch for HTTP Providers**: For HTTP providers (`OpenAiCompatible`, `Ollama`), the 44-byte WAV header + PCM buffer stays entirely in process memory (`Vec<u8>`) and is zeroized on drop. A `0600` file under `secure_fs` is only materialized if `SttBackendKind::LocalCli` requires a file path argument, and is unlinked immediately in a `Drop` guard.

#### Tier 2: Optional Native In-Process Audio (`--features stt-native-audio`)
For users building desktop/GUI releases who want native CoreAudio / WASAPI / ALSA / PipeWire device enumeration without external CLI helpers:
- Optional Cargo feature `stt-native-audio = ["dep:cpal"]`.
- Implements the exact same `AudioCaptureBackend` trait (`start_stream() -> Result<AudioStreamHandle>`), downmixing and resampling device frames to 16 kHz mono `i16` chunks fed into the same VAD/RMS/WAV pipeline.

---

## 4. Security, Privacy & Resource Bounding (`SEC-R01..04`)

Microphone access and speech transcription handle highly sensitive environmental data. The STT module enforces four strict invariants:

1. **Opt-In Disabled by Default**:
   - `[stt] enabled = false` by default in `Config`. No microphone probe, subprocess spawn, or network socket is opened unless explicitly enabled by the user or toggled via an explicit user action when enabled.
2. **Loopback-Only Endpoint Confinement by Default**:
   - `SttConfig::validate_endpoint()` parses the configured host and rejects any non-loopback address (`127.0.0.1`, `::1`, `localhost`, or `unix:///...`) unless `allow_remote_stt = true` is explicitly set in `config.toml`. This prevents accidental exfiltration of voice audio to external cloud endpoints.
3. **Secret Hygiene (`SEC-R02`)**:
   - No API key is ever passed on CLI argv or logged in `tracing`. If authentication is needed (e.g., secured local gateway or remote OpenAI/Groq when `allow_remote_stt = true`), credentials are read exclusively from `api_key_file: Option<PathBuf>`, verified to have `0600` permissions on Unix via `src/secure_fs.rs`.
4. **Bounded Buffers & Ephemeral File Scrubbing (`SEC-R01`)**:
   - Recording duration is bounded by `max_recording_secs` (default `60`, max `300`, i.e., `<= 9.6 MB` PCM cap).
   - HTTP responses from the STT server are capped at `64 KiB` (`tokio::io::AsyncReadExt::take(65536)`).
   - Any temporary WAV file created for `LocalCli` backends is written inside `secure_fs::ensure_private_dir(&runtime_dir.join("stt"))` (`0700`) with mode `0600`, overwritten with zeros, and unlinked immediately via an RAII `ScrubbedTempWav` guard when transcription finishes or aborts.

---

## 5. UX & Surface Uniformity (`UNI-R01`)

To respect Bilterm's IBM 3270 + Borland Turbo Vision aesthetic and `UNI-R01` surface uniformity:

1. **Operator Information Area (OIA) Status Bar Indicator**:
   - Idle (when `[stt] enabled = true`): `[🎙 MIC]` clickable hotspot in the OIA bar.
   - Recording: `[🎙 REC ▂▄▆ 03.2s]` in high-contrast 3270 status alert style (red/amber/phosphor highlight).
   - Transcribing: `[🎙 STT ...]` while awaiting local model inference.
   - Error / Disabled: `[🎙 ERR]` (with status message in OIA/toast) or hidden when disabled.
2. **Safe Staging in Floating Command Prompt**:
   - By default (`auto_submit = false`), transcribed text is inserted at the cursor position inside the focused pane's `FloatingCommandPrompt` (`Focus::FloatingPrompt` is automatically focused if not in `DECSET 1049` alternate screen).
   - This lets the user inspect the command with live `tree-sitter-bash` syntax highlighting before pressing `Enter`.
   - When the pane is in Raw Passthrough / Alternate Screen (`vim`, `claude`, `ssh`) or if `:voice send` is used, text can optionally be routed to the PTY only when explicitly requested.
3. **All 4 Surfaces Covered (`UNI-R01`)**:
   - **Mouse**: Click `[🎙 MIC]` / `[🎙 REC]` badge in the OIA bottom status bar (or Top Menu `Tools -> Voice Dictation Toggle`) to start/stop recording.
   - **Keyboard**: Leader chord `Ctrl+B v` (and `Alt+V` global/pane shortcut) toggles voice recording; `Esc` while recording cancels recording without transcribing (`:voice cancel`).
   - **Colon Command / Palette / Help**: `:voice toggle`, `:voice start`, `:voice stop`, `:voice cancel`, `:voice status`, `:voice provider <openai|ollama|cli>`.
   - **RPC / HTTP / MCP / `bilterm ctl`**: `"voice.toggle"`, `"voice.start"`, `"voice.stop"`, `"voice.cancel"`, `"voice.status"`, and `"voice.inject_wav"` (for deterministic headless testing and external audio pipelines).
