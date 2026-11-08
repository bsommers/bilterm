# Phase 59 Research Spike: GUI Web Browser Canvas (Chrome Engine) & Video Player Canvas (`WEB-01..02`, `VID-01..02`)

## 1. Executive Summary & Existing Rendering Seam Analysis

Bilterm's dual-surface architecture already provides a 1:1 pixel texture compositor in GUI mode and an adaptive sub-cell/protocol rasterizer in TUI mode:

1. **GUI Texture Pipeline (`src/ui/renderer.rs`, `src/ui/gui_font/{linux_x11,macos_ct,win32_gdi}.rs`)**:
   - `GuiWorkspaceRenderer::render_workspace_with_cell_size` populates `GuiFrameSurface { quads, glyphs, images }`.
   - Each `GuiImageTexture` (`id`, `x_px`, `y_px`, `width_px`, `height_px`, `pixel_width`, `pixel_height`, `rgba_bytes`) is blitted during **Pass 2** across all three native backends:
     - **Linux X11 (`X11GuiCanvas`)**: Blits `rgba_bytes` into `self.pixels: Vec<u32>` (`0xFFRRGGBB` ZPixmap) before `XPutImage` + `XftDrawStringUtf8`.
     - **macOS (`MacosGuiCanvas`)**: Blits `rgba_bytes` into `self.pixels: Vec<u32>` backing a 1:1 Retina `CGBitmapContext` -> `CGImageRef` before `CTFontDrawGlyphs`.
     - **Windows (`Win32GuiCanvas`)**: Blits `rgba_bytes` into `self.dib_pixels: *mut u32` (`CreateDIBSection`) before `ExtTextOutW` + `BitBlt`.
   - **Occlusion & Compositing**: Because Pass 2 (`surface.images`) executes *before* Pass 3 (geometric box-drawing) and Pass 4 (hardware font glyphs), and zeroes out alpha (`rgba[idx + 3] = 0`) under `pane_snap.prompt_rect` and top-level modals (`drawer_rect`, `mcp_rect`, `modal_rect`, `qt_rect`), any full-resolution RGBA frame pushed for a pane's `canvas_rect` automatically respects:
     - Borland Turbo Vision double-line borders and window chrome
     - The pane's `FloatingCommandPrompt` floating above the browser/video canvas
     - The bottom 1-row `OiaStatusBar` and custom OSD/URL bars rendered via Ratatui cells
     - Z-order window stacking (`ZStack`) and scaled panel masking
2. **TUI Fallback Pipeline (`src/pane/canvas_scene.rs`, `src/ui/pixel_canvas.rs`, `src/ui/canvas_raster.rs`)**:
   - `PaneCanvasScene` and `PixelCanvasBuffer` already support `InteractiveImageView { width, height, rgba, pan_x, pan_y, zoom }`, adaptive terminal graphics protocol detection (`CanvasRenderBackend::{Auto, Kitty, Sixel, ITerm2, HalfBlock, Octant, Braille, Sextant, Quadrant}`), and `wrap_tmux_dcs_passthrough`.

---

## 2. Part 1: Chrome Engine Web Browser Canvas (`WEB-01..02`)

### 2.1 Candidate Engine Architecture Comparison

| Dimension | Option A: Out-of-Process Chromium via CDP Screencast / BeginFrame (Recommended Default) | Option B: In-Process CEF (`libcef` Off-Screen Rendering, `--features cef-browser`) | Option C: OS WebView (`wry` / WebKitGTK / WebView2 / WKWebView) |
|-----------|-----------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|------------------------------------------------------------------|
| **Browser Engine** | True **Blink + V8 (Chrome Engine)** on all OSes (`google-chrome`, `chromium`, `brave`, `msedge`) | True **Blink + V8 (Chrome Engine)** via linked `libcef.so` / `libcef.dll` / `Chromium Embedded Framework.framework` | Heterogeneous (WebKitGTK on Linux, WKWebView on macOS, Edge WebView2 on Windows) — **violates Chrome Engine requirement on Linux/macOS** |
| **Build & Link Overhead** | **0 MB** Cargo dependency overhead; zero C++ SDK download required to run `cargo build` / `scripts/verify.sh` | **~220 MB–350 MB** prebuilt binary distribution per target OS/arch; complex `rpath` and bundle layout requirements | Requires `webkit2gtk-4.1-dev` system headers on Linux; breaks hermetic builds |
| **Off-Screen Pixel Capture** | CDP `Page.startScreencast` (JPEG/PNG) or `HeadlessExperimental.beginFrame` / shared-memory frame sink -> `GuiImageTexture` | `CefRenderHandler::OnPaint` callback delivers raw `BGRA_8888` dirty rects at `windowless_frame_rate = 60` | Native child `HWND`/`XWindow`/`NSView` airspace conflict: overlays *above* the parent window, breaking Bilterm's `FloatingCommandPrompt`, CRT scanlines, and Z-order stacking |
| **Process & Crash Isolation** | **100% isolated OS process tree**; a runaway tab or WebGL crash never corrupts Bilterm's terminal daemon or raw-mode state | Requires multi-process CEF zygote helper executable (`bilterm-cef-helper`) and careful signal-handler coexistence | In-process UI thread coupling on macOS/Linux |
| **Security & Sandboxing (`SEC-R01..04`)** | Dedicated ephemeral or named profile dir under `secure_fs::ensure_private_dir` (`0700`); communication over anonymous pipe (`--remote-debugging-pipe`) or `127.0.0.1:0` with `0600` state | Full Chromium sandbox requires root-owned `chrome-sandbox` SUID binary or user namespaces | Shares process memory space with terminal emulator |

### 2.2 Deep Dive: Option A — Out-of-Process Chromium via CDP (`CdpBrowserEngine`)

1. **Executable Discovery**:
   - Deterministic probe order across platforms (configurable via `config.browser.executable_path` or `BILTERM_CHROME_PATH`):
     - **Linux**: `chromium`, `chromium-browser`, `google-chrome-stable`, `google-chrome`, `brave-browser`, `microsoft-edge-stable`
     - **macOS**: `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`, `/Applications/Chromium.app/Contents/MacOS/Chromium`, `/Applications/Brave Browser.app/Contents/MacOS/Brave Browser`, `/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge`
     - **Windows**: `%ProgramFiles%\Google\Chrome\Application\chrome.exe`, `%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe`, `%LocalAppData%\Google\Chrome\Application\chrome.exe`
2. **Launch Flags & Confinement (`SEC-R01..04`)**:
   - Profile directory created via `secure_fs::ensure_private_dir(&runtime_dir.join("browser-profiles").join(safe_pane_id))` (`0700`).
   - Spawned with:
     ```text
     --headless=new
     --user-data-dir=<0700_private_dir>
     --window-size=<canvas_width_px>,<canvas_height_px>
     --hide-scrollbars
     --mute-audio (toggleable)
     --no-first-run
     --no-default-browser-check
     --disable-background-networking
     --remote-debugging-pipe (or --remote-debugging-port=0 on 127.0.0.1)
     ```
   - Never passes tokens or URLs with embedded credentials on CLI `argv` (`SEC-R03`): launches `about:blank` and navigates via CDP `Page.navigate` over the private debugging channel.
3. **Frame Capture Pipeline**:
   - **Viewport Synchronization**: When a pane resizes or `font_scale_pct` changes, compute `width_px = u32::from(canvas_rect.width) * u32::from(eff_cw)` and `height_px = u32::from(canvas_rect.height.saturating_sub(1)) * u32::from(eff_ch)` (reserving row 0 of `canvas_rect` for the interactive URL/Navigation Bar chrome) and send CDP `Emulation.setDeviceMetricsOverride { width, height, deviceScaleFactor: 1.0, mobile: false }`.
   - **Streaming Frames**:
     - Primary: `Page.startScreencast { format: "jpeg", quality: 85, maxWidth: width_px, maxHeight: height_px, everyNthFrame: 1 }`.
     - Upon receiving `Page.screencastFrame { data, metadata, sessionId }`, a dedicated worker thread decodes the frame into a recycled `Arc<Vec<u8>>` RGBA buffer, updates `BrowserFrameSlot` (lock-free `ArcSwap` / `Mutex<Option<BrowserFrame>>` with monotonic `frame_seq: u64`), and immediately sends `Page.screencastFrameAck { sessionId }` to pace delivery to Bilterm's render rate.
     - Deterministic Headless/Test Fallback: Built-in `SyntheticDomRenderer` renders a crisp offline HTML/DOM preview + status card into RGBA when no external Chromium binary is installed (ensuring 100% deterministic CI tests in `scripts/verify.sh` without requiring Chrome in minimal containers).
4. **Interactive Input Coordinate Mapping (`GuiWorkspaceRenderer` & TUI SGR Mouse)**:
   - In GUI mode (`src/ui/gui_window.rs`, `src/ui/gui_font/linux_x11.rs`, `src/ui/gui_macos.rs`), mouse events currently convert `(px, py)` -> `(col, row)` via `pixel_to_cell_coords`.
   - For sub-cell pixel accuracy in the browser canvas:
     - Store the latest raw cursor pixel coordinates `(last_mouse_px_x, last_mouse_px_y)` alongside `(col, row)` in GUI window runners, or derive `(rel_x_px, rel_y_px)` from `(col - canvas_rect.x) * eff_cw + eff_cw / 2`, `(row - (canvas_rect.y + 1)) * eff_ch + eff_ch / 2`.
     - Forward clicks/drags via CDP `Input.dispatchMouseEvent { type: "mousePressed" | "mouseReleased" | "mouseMoved" | "mouseWheel", x: rel_x_px, y: rel_y_px, button: "left" | "right" | "middle", clickCount: 1, deltaX, deltaY }`.
     - When the pane is in `BrowserInteractive` focus mode (clicked inside the browser viewport or toggled via `Enter` / `:web focus`), keystrokes forward via CDP `Input.dispatchKeyEvent { type: "keyDown" | "char" | "keyUp", text, key, windowsVirtualKeyCode, modifiers }`, while `Esc` or `Ctrl+B` / `Ctrl+X` leader returns focus to Bilterm's `FloatingCommandPrompt`!

### 2.3 Deep Dive: Option B — In-Process CEF (`CefBrowserEngine`, `--features cef-browser`)

- Behind Cargo feature `cef-browser`, implement the same `BrowserEngine` trait using CEF Windowless (Off-Screen) Rendering:
  - `CefWindowInfo::windowless_rendering_enabled = 1`.
  - `CefRenderHandler::GetViewRect` returns `(width_px, height_px)`.
  - `CefRenderHandler::OnPaint` receives `buffer: *const c_void` (`BGRA_8888`, `width * height * 4`), swizzles `BGRA -> RGBA` (or passes BGRA directly to `GuiImageTexture`), and increments `frame_seq`.
  - Input forwarding calls `CefBrowserHost::SendMouseClickEvent`, `SendMouseMoveEvent`, `SendMouseWheelEvent`, and `SendKeyEvent`.

---

## 3. Part 2: Video Player Canvas (`VID-01..02`)

### 3.1 Video Decoding & Audio Pipeline Comparison

| Dimension | Option 1: Out-of-Process `ffmpeg` Raw RGBA Pipe + Companion Audio (`ffplay -nodisp`) (Recommended Default) | Option 2: Linked `libmpv` Render API (`--features mpv-video`) | Option 3: Linked `gstreamer-app` (`--features gst-video`) |
|-----------|------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------|-----------------------------------------------------------|
| **Format & Codec Coverage** | `.mp4` (H.264/H.265/AV1), `.webm` (VP8/VP9/AV1), `.mkv`, `.gif`, `.mov`, HTTP/RTSP streams | Identical (FFmpeg-backed) | Requires installing `gst-plugins-{base,good,bad,ugly}` |
| **Build & Link Overhead** | **0 MB** Cargo build impact; shells out to `ffmpeg` / `ffprobe` when present + native Rust GIF/test-stream decoder built-in | Requires `libmpv-dev` system C headers | Requires 10+ GStreamer pkg-config dev packages |
| **Frame Pacing & Sync** | `-re` real-time pacing or PTS-driven ring buffer (`VideoFrameRing`, 3-slot triple buffer) synced to wall-clock `Instant` + audio start timestamp | `mpv_render_context_render` with `MPV_RENDER_PARAM_SW_POINTER` | `AppSink` sample pull callback |
| **Fault Isolation** | Corrupt media container cannot segfault Bilterm process | In-process C codec vulnerabilities share address space | In-process C plugin vulnerabilities share address space |

### 3.2 Video Player Architecture (`VideoPlayerEngine`)

1. **Dual-Tier Decoder Backend**:
   - **Tier 1 (`FfmpegPipeDecoder`)**:
     - Probes media metadata (`duration_secs`, `fps`, `width`, `height`, `has_audio`, `codec_name`) via `ffprobe -v quiet -print_format json -show_format -show_streams <path>`.
     - Spawns `ffmpeg -nostdin -loglevel error -ss <seek_secs> -re -i <path> -vf scale=<dst_w>:<dst_h>:flags=fast_bilinear -pix_fmt rgba -f rawvideo -` on a background `std::thread`.
     - Reads exact `dst_w * dst_h * 4` byte chunks into a pre-allocated 3-slot `VideoFrameRing` (zero per-frame heap allocation after resize) and updates `pts_ms` + `frame_seq`.
     - Audio synchronization: When `muted == false` and audio stream exists, spawns companion `ffplay -nodisp -vn -autoexit -loglevel quiet -ss <seek_secs> <path>` tied to the same play/pause/seek lifecycle (killed automatically on `Drop`, pause, seek, or pane close via `controlling-processes-safely`).
   - **Tier 2 (`BuiltInSyntheticAndAnimationDecoder`)**:
     - Pure-Rust procedural/animated frame generator and raw `.ppm`/animated-frame loader so `:video open demo://smpte` or `:video open <file>` works deterministically in CI and on systems without `ffmpeg`.
2. **Memory & FPS Budget**:
   - At `640×360` (16:9 inside a typical split pane): `640 * 360 * 4 = 921.6 KB` per RGBA frame (`2.76 MB` for a 3-slot ring buffer).
   - At `1280×720` (720p zoomed pane): `1280 * 720 * 4 = 3.68 MB` per RGBA frame (`11.05 MB` for a 3-slot ring buffer).
   - Hard cap (`bounding-untrusted-resource-use`): Decoder clamps output raster dimensions to `MAX_VIDEO_CANVAS_PX = (1920, 1080)` (`8.29 MB` max frame size) regardless of source 4K/8K media resolution.
   - At 60 FPS (`16.67 ms` budget), blitting `640×360` into `GuiFrameSurface` / `Win32GuiCanvas` / `X11GuiCanvas` / `MacosGuiCanvas` copies `230,400` `u32` words (`< 0.35 ms` in release builds).
3. **Interactive OSD Transport Controls**:
   - Rendered along the bottom row of `canvas_rect` (just above `OiaStatusBar`) in both GUI and TUI modes:
     ```text
     [▶ PLAY / ⏸ PAUSE] [⏮ -5s] [⏭ +5s] [━━━●──────── 01:24 / 03:50] [🔊 VOL 100%] [1.0x] [🔁 LOOP]
     ```
   - **Mouse**:
     - Click `[▶/⏸]` toggles play/pause.
     - Click on the progress scrub bar `[━━━●────]` seeks to `(click_col - bar_start_col) / bar_width * duration_secs`.
     - Click `[🔊]` toggles mute; mouse wheel over volume or scrub bar adjusts volume/seek.
   - **Keyboard (when video canvas is active)**:
     - `Space`: Toggle Play/Pause
     - `Left` / `Right`: Seek `-5s` / `+5s`
     - `Up` / `Down`: Volume `+5%` / `-5%`
     - `m`: Toggle Mute
     - `l`: Toggle Loop
     - `[` / `]`: Playback speed `0.5x` .. `2.0x`

---

## 4. Security, Resource Bounding & Surface Uniformity (`SEC-R01..04`, `UNI-R01`)

1. **Filesystem & Argument Security (`SEC-R01..04`)**:
   - Browser user-data directories and snapshot downloads are confined inside `secure_fs::ensure_private_dir` (`0700`).
   - Local `file://` URLs and video file paths are canonicalized and checked against path traversal when invoked from untrusted RPC/MCP sessions.
   - Subprocesses (`chromium`, `ffmpeg`, `ffplay`) are spawned with `Command::new(program).args(...)` (never `sh -c`), `stdin(Stdio::null())` (or dedicated pipe), and explicit `kill_on_drop` / PID lifecycle tracking so no orphan browser or audio processes survive pane closure or Bilterm exit.
2. **Bounded Buffers (`bounding-untrusted-resource-use`)**:
   - Browser and video frame buffers are capped at `1920 × 1080 × 4` bytes (`8,294,400` bytes).
   - CDP JSON message reader enforces a `16 MiB` maximum frame payload cap.
3. **Surface Uniformity (`UNI-R01`)**:
   - Every browser and video operation lowers to `Action::Browser(BrowserOp)` and `Action::Video(VideoOp)` executed via `WindowManager::apply`:
     - **Mouse**: URL bar click, Back/Forward/Reload buttons, viewport click/drag/wheel forwarding; Video OSD play/pause, scrub bar click-to-seek, mute toggle.
     - **Keys**: Leader chords + intra-canvas interactive bindings (`Space`, arrows, `m`, `Esc`) in `src/input/keymap.rs`.
     - **Colon Commands & Palette**: `:web` / `:browser` (`open`, `back`, `forward`, `reload`, `stop`, `zoom`, `close`) and `:video` (`open`, `play`, `pause`, `toggle`, `seek`, `volume`, `mute`, `loop`, `speed`, `close`).
     - **RPC / HTTP / MCP / CLI**: `browser.open`, `browser.navigate`, `browser.input`, `browser.state`, `video.open`, `video.control`, `video.state`.
