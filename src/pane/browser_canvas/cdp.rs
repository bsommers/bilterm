//! Out-of-process Chromium engine bridge via Chrome DevTools Protocol (`WEB-01..02`).
//!
//! Layout: [`super::cdp_launch`] starts Chrome with `--remote-debugging-pipe`,
//! [`super::cdp_session`] runs the reader/writer threads and the protocol state machine,
//! [`super::cdp_proto`] and [`super::cdp_input`] build and parse messages. This file is the
//! [`BrowserEngine`] the pane talks to.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;

use crate::error::Result;
use crate::pane::PaneId;

use super::cdp_input::{key_commands, mouse_command};
use super::cdp_proto::{self as proto, Cmd};
use super::cdp_session::{Boot, PageState, Phase, PushError, SCREENCAST_QUALITY, Shared, Viewport, spawn_session};
use super::url_policy::{file_urls_enabled, validate_url};
use super::{BrowserEngine, EngineHealth, MediaMouseEvent, MediaMouseEventKind, MediaRgbaFrame, browser_error};

/// Probe order for finding an installed Chromium-family browser binary.
pub fn discover_chromium_binary() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("BILTERM_CHROME_PATH") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ];
        for path in &candidates {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
        ];
        for path in &candidates {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // Unix / Linux / fallback PATH lookup
    let progs = [
        "chromium",
        "chromium-browser",
        "google-chrome-stable",
        "google-chrome",
        "brave-browser",
        "microsoft-edge-stable",
    ];
    for name in &progs {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
    }

    None
}

/// Time allowed between launching Chrome and a finished DevTools handshake.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// How often the tick re-reads navigation history. Chrome sends no event when a script changes
/// `document.title`, so the committed history entry is polled for title and back/forward state.
const HISTORY_REFRESH: Duration = Duration::from_millis(400);
/// The refresh is skipped while this many commands are still waiting for the writer.
const REFRESH_MAX_BACKLOG: usize = 32;

/// Why real Chrome could not be started (shown as `synthetic (<reason>)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpawnError {
    NotFound,
    Launch(String),
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Chrome not found"),
            Self::Launch(why) => write!(f, "launch failed: {why}"),
        }
    }
}

/// Real browser engine: headless Chrome driven over the DevTools pipe.
///
/// Every method only enqueues a command or reads shared state, so the UI thread never waits for
/// Chrome. `backend_name` is `"cdp"`; [`BrowserEngine::health`] is `Starting` until the handshake.
pub struct CdpBrowserBackend {
    shared: Arc<Shared>,
    #[cfg(unix)]
    process: Option<super::cdp_launch::ChromeProcess>,
    page: PageState,
    viewport: Viewport,
    started: Instant,
    allow_file: bool,
    last_click: Option<(Instant, u16, u16, u8)>,
    last_history_refresh: Instant,
}

impl CdpBrowserBackend {
    /// Launches Chrome and starts the handshake in the background. Returns at once.
    pub fn try_spawn(
        initial_url: &str,
        width_px: u16,
        height_px: u16,
        pane_id: PaneId,
    ) -> std::result::Result<Self, SpawnError> {
        let allow_file = file_urls_enabled();
        let url = validate_url(initial_url, allow_file).map_err(|e| SpawnError::Launch(format!("blocked URL: {e}")))?;
        let bin = discover_chromium_binary().ok_or(SpawnError::NotFound)?;
        Self::spawn_process(&bin, &url, width_px, height_px, pane_id, allow_file)
    }

    #[cfg(unix)]
    fn spawn_process(
        bin: &std::path::Path,
        url: &str,
        width_px: u16,
        height_px: u16,
        pane_id: PaneId,
        allow_file: bool,
    ) -> std::result::Result<Self, SpawnError> {
        use super::cdp_launch::{launch, profiles_root, sweep_stale_profiles};
        let io_err = |what: &str, e: std::io::Error| SpawnError::Launch(format!("{what}: {e}"));
        let root = profiles_root().map_err(|e| io_err("private profile dir", e))?;
        let sweep_root = root.clone();
        let _ = std::thread::Builder::new().name("bilterm-chrome-sweep".into()).spawn(move || sweep_stale_profiles(&sweep_root));
        let viewport = Viewport::from_frame_px(width_px, height_px, 1.0);
        let (process, pipes) = launch(bin, &root, pane_id, viewport.css_width, viewport.css_height)
            .map_err(|e| io_err("could not start Chrome", e))?;
        let mut backend = Self::with_transport(pipes.events, pipes.commands, url, viewport, allow_file)
            .map_err(|e| io_err("could not start worker threads", e))?;
        backend.process = Some(process);
        Ok(backend)
    }

    #[cfg(not(unix))]
    fn spawn_process(
        _bin: &std::path::Path,
        _url: &str,
        _width_px: u16,
        _height_px: u16,
        _pane_id: PaneId,
        _allow_file: bool,
    ) -> std::result::Result<Self, SpawnError> {
        Err(SpawnError::Launch("the DevTools pipe transport needs a Unix host".to_string()))
    }

    /// Runs the engine over an already connected transport (tests use in-memory pipes).
    pub fn with_transport<R, W>(reader: R, writer: W, url: &str, viewport: Viewport, allow_file: bool) -> std::io::Result<Self>
    where
        R: std::io::Read + Send + 'static,
        W: std::io::Write + Send + 'static,
    {
        let shared = Shared::new(Boot { url: url.to_string(), viewport, allow_file });
        spawn_session(reader, writer, Arc::clone(&shared))?;
        let page = shared.page();
        Ok(Self {
            shared,
            #[cfg(unix)]
            process: None,
            page,
            viewport,
            started: Instant::now(),
            allow_file,
            last_click: None,
            last_history_refresh: Instant::now(),
        })
    }

    /// Pixel size of the newest screencast frame (`(0, 0)` before the first one).
    pub fn frame_size_px(&self) -> (u16, u16) {
        self.page.frame_px
    }

    fn sync(&mut self) -> bool {
        match self.shared.page_if_changed(self.page.generation) {
            Some(page) => {
                self.page = page;
                true
            }
            None => false,
        }
    }

    fn send(&mut self, cmd: Cmd) -> Result<()> {
        match self.shared.queue.push(cmd) {
            Ok(()) => Ok(()),
            Err(PushError::Busy) => Err(browser_error(std::io::ErrorKind::WouldBlock, "browser is busy; input dropped")),
            Err(PushError::Closed) => Err(browser_error(std::io::ErrorKind::BrokenPipe, "browser is not running")),
        }
    }

    fn is_ready(&self) -> bool {
        self.page.phase == Phase::Ready
    }

    /// Frame pixels to CSS pixels, from the newest frame's metadata (falls back to the zoom scale).
    fn css_point(&self, x_px: u16, y_px: u16) -> (f64, f64) {
        let (fw, fh) = self.page.frame_px;
        let (dw, dh) = self.page.device_px;
        if fw > 0 && fh > 0 && dw > 0.0 && dh > 0.0 {
            return (f64::from(x_px) * dw / f64::from(fw), f64::from(y_px) * dh / f64::from(fh));
        }
        (f64::from(x_px) / self.viewport.scale, f64::from(y_px) / self.viewport.scale)
    }

    fn click_count(&mut self, button: u8, x: u16, y: u16) -> u8 {
        let now = Instant::now();
        let count = match self.last_click {
            Some((at, lx, ly, n)) if now.duration_since(at) <= DOUBLE_CLICK && lx.abs_diff(x) <= 8 && ly.abs_diff(y) <= 8 => {
                if n >= 3 { 1 } else { n + 1 }
            }
            _ => 1,
        };
        let _ = button;
        self.last_click = Some((now, x, y, count));
        count
    }
}

impl Drop for CdpBrowserBackend {
    fn drop(&mut self) {
        // Closing the queue stops the writer; killing Chrome (ChromeProcess::drop) ends the reader.
        self.shared.queue.close();
    }
}

impl BrowserEngine for CdpBrowserBackend {
    fn navigate(&mut self, url: &str) -> Result<()> {
        self.sync();
        let url = validate_url(url, self.allow_file)
            .map_err(|e| browser_error(std::io::ErrorKind::InvalidInput, format!("blocked URL: {e}")))?;
        match self.page.phase {
            Phase::Starting => self.shared.update_boot(|b| b.url = url.clone()),
            Phase::Ready => self.send(proto::navigate(&url))?,
            Phase::Crashed(_) | Phase::Failed(_) => {
                return Err(browser_error(std::io::ErrorKind::BrokenPipe, "browser is not running"));
            }
        }
        self.shared.update_page(|p| {
            p.url = url;
            p.loading = true;
            p.nav_error = None;
        });
        self.sync();
        Ok(())
    }

    fn go_back(&mut self) -> Result<()> {
        self.send(proto::get_history(-1))
    }

    fn go_forward(&mut self) -> Result<()> {
        self.send(proto::get_history(1))
    }

    fn reload(&mut self) -> Result<()> {
        self.send(proto::reload())
    }

    fn stop(&mut self) -> Result<()> {
        self.send(proto::stop_loading())
    }

    fn resize_viewport(&mut self, width_px: u16, height_px: u16, scale_factor: f32) -> Result<()> {
        let viewport = Viewport::from_frame_px(width_px, height_px, scale_factor);
        if viewport == self.viewport {
            return Ok(());
        }
        self.viewport = viewport;
        self.sync();
        if !self.is_ready() {
            self.shared.update_boot(|b| b.viewport = viewport);
            return Ok(());
        }
        let (w, h) = viewport.frame_px();
        self.send(proto::set_device_metrics(viewport.css_width, viewport.css_height, viewport.scale))?;
        self.send(proto::stop_screencast())?;
        self.send(proto::start_screencast(w, h, SCREENCAST_QUALITY))
    }

    fn dispatch_mouse(&mut self, event: MediaMouseEvent) -> Result<()> {
        if !self.is_ready() {
            return Ok(());
        }
        let (x, y) = self.css_point(event.x_px, event.y_px);
        let clicks = match event.kind {
            MediaMouseEventKind::Down(b) => self.click_count(b as u8, event.x_px, event.y_px),
            MediaMouseEventKind::Up(_) => self.last_click.map_or(1, |c| c.3),
            _ => 0,
        };
        self.send(mouse_command(&event, x, y, clicks))
    }

    fn dispatch_key(&mut self, key: KeyEvent) -> Result<()> {
        if !self.is_ready() {
            return Ok(());
        }
        for cmd in key_commands(key) {
            self.send(cmd)?;
        }
        Ok(())
    }

    fn insert_text(&mut self, text: &str) -> Result<()> {
        if !self.is_ready() || text.is_empty() {
            return Ok(());
        }
        self.send(proto::insert_text(text))
    }

    fn latest_frame(&mut self) -> Option<MediaRgbaFrame> {
        self.shared.latest_frame()
    }

    fn current_url(&self) -> &str {
        &self.page.url
    }

    fn page_title(&self) -> &str {
        &self.page.title
    }

    fn is_loading(&self) -> bool {
        self.page.loading
    }

    fn backend_name(&self) -> &'static str {
        "cdp"
    }

    fn poll(&mut self) -> bool {
        if self.page.phase == Phase::Starting && self.started.elapsed() > HANDSHAKE_TIMEOUT {
            #[cfg(unix)]
            let detail = self.process.as_ref().and_then(|p| {
                p.kill_group();
                p.stderr_tail()
            });
            #[cfg(not(unix))]
            let detail: Option<String> = None;
            let secs = HANDSHAKE_TIMEOUT.as_secs();
            self.shared.mark_dead(&match detail {
                Some(d) => format!("no DevTools handshake within {secs} s ({d})"),
                None => format!("no DevTools handshake within {secs} s"),
            });
        }
        if self.is_ready()
            && self.last_history_refresh.elapsed() >= HISTORY_REFRESH
            && self.shared.queue.len() < REFRESH_MAX_BACKLOG
        {
            self.last_history_refresh = Instant::now();
            let _ = self.shared.queue.push(proto::get_history(0));
        }
        self.sync()
    }

    fn health(&self) -> EngineHealth {
        match &self.page.phase {
            Phase::Starting => EngineHealth::Starting,
            Phase::Ready => EngineHealth::Ready,
            Phase::Crashed(why) => EngineHealth::Crashed(why.clone()),
            Phase::Failed(why) => EngineHealth::Failed(why.clone()),
        }
    }

    fn can_go_back(&self) -> bool {
        self.page.history_idx > 0
    }

    fn can_go_forward(&self) -> bool {
        self.page.history_idx + 1 < self.page.history.len()
    }

    fn nav_error(&self) -> Option<&str> {
        self.page.nav_error.as_deref()
    }

    fn process_info(&self) -> Option<(i32, PathBuf)> {
        #[cfg(unix)]
        return self.process.as_ref().map(|p| (p.pgid(), p.profile_dir().to_path_buf()));
        #[cfg(not(unix))]
        None
    }
}

// Minimal PATH lookup helper without adding external `which` crate if not in Cargo.toml
mod which {
    use std::path::PathBuf;

    pub fn which(binary_name: &str) -> Result<PathBuf, ()> {
        let path_var = std::env::var_os("PATH").ok_or(())?;
        for dir in std::env::split_paths(&path_var) {
            let full = dir.join(binary_name);
            if full.is_file() {
                return Ok(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(format!("{binary_name}.exe"));
                if full_exe.is_file() {
                    return Ok(full_exe);
                }
            }
        }
        Err(())
    }
}
