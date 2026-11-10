//! `BrowserCanvasState`: the browser canvas owned by a pane (engine, focus, zoom, URL editor).

use std::sync::OnceLock;

use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::error::Result;
use crate::pane::PaneId;

use super::url_policy::{file_urls_enabled, validate_url};
use super::{
    BrowserEngine, EngineHealth, MediaMouseEvent, MediaRgbaFrame, SyntheticBrowserBackend, browser_error,
    browser_error_text, cdp, geometry,
};

/// In-progress URL bar edit: text buffer plus cursor (in chars).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlEditState {
    pub text: String,
    pub cursor: usize,
}

/// `true` inside a cargo test binary (unit or integration): those live in `target/<profile>/deps`.
/// `cfg!(test)` alone is false for integration tests, which link the library without it.
pub fn under_test_harness() -> bool {
    static HARNESS: OnceLock<bool> = OnceLock::new();
    cfg!(test)
        || *HARNESS.get_or_init(|| {
            std::env::var_os("NEXTEST").is_some()
                || std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().and_then(|d| d.file_name()).map(|n| n == "deps"))
                    .unwrap_or(false)
        })
}

/// Schemes served by the synthetic engine only; they never reach Chrome.
pub fn is_synthetic_url(url: &str) -> bool {
    url.starts_with("demo://") || url.starts_with("test://") || url.starts_with("about:")
}

/// Why the synthetic engine must be used, or `None` when real Chrome may be launched.
///
/// Under a test harness a real browser is launched only with `BILTERM_BROWSER_CDP=1`.
fn forced_synthetic(url: &str) -> bool {
    is_synthetic_url(url)
        || std::env::var_os("BILTERM_BROWSER_SYNTHETIC").is_some()
        || std::env::var_os("BILTERM_SYNTHETIC_MEDIA").is_some()
        || (under_test_harness() && std::env::var("BILTERM_BROWSER_CDP").as_deref() != Ok("1"))
}

/// Stateful browser canvas manager owned by [`crate::pane::CompositePane`].
pub struct BrowserCanvasState {
    engine: Box<dyn BrowserEngine>,
    pub viewport_focused: bool,
    pub zoom_pct: u16,
    cached_frame: Option<MediaRgbaFrame>,
    url_edit: Option<UrlEditState>,
    /// Why the synthetic engine runs instead of Chrome (`None`: by request, or CDP is running).
    fallback: Option<String>,
    /// Last refused or failed navigation, shown in the nav bar.
    notice: Option<String>,
    pane_id: Option<PaneId>,
    viewport_px: (u16, u16),
}

impl BrowserCanvasState {
    pub fn new(mut engine: Box<dyn BrowserEngine>) -> Self {
        let cached_frame = engine.latest_frame();
        let viewport_px = cached_frame.as_ref().map_or((800, 600), |f| (f.width_px, f.height_px));
        Self {
            engine,
            viewport_focused: false,
            zoom_pct: 100,
            cached_frame,
            url_edit: None,
            fallback: None,
            notice: None,
            pane_id: None,
            viewport_px,
        }
    }

    pub fn new_synthetic(initial_url: &str, width_px: u16, height_px: u16) -> Self {
        let mut state = Self::new(Box::new(SyntheticBrowserBackend::new(initial_url, width_px, height_px)));
        state.viewport_px = (width_px, height_px);
        state
    }

    /// Opens `initial_url` in real Chrome when possible, otherwise in the synthetic engine with
    /// the reason recorded (see [`Self::backend_name`]). Never blocks on Chrome: the CDP engine
    /// reports `Starting` until its handshake finishes.
    pub fn new_cdp_or_synthetic(initial_url: &str, width_px: u16, height_px: u16, pane_id: PaneId) -> Self {
        let mut state = if forced_synthetic(initial_url) {
            Self::new_synthetic(initial_url, width_px, height_px)
        } else {
            match cdp::CdpBrowserBackend::try_spawn(initial_url, width_px, height_px, pane_id) {
                Ok(backend) => Self::new(Box::new(backend)),
                Err(reason) => {
                    let mut s = Self::new_synthetic(initial_url, width_px, height_px);
                    s.fallback = Some(reason.to_string());
                    s
                }
            }
        };
        state.pane_id = Some(pane_id);
        state.viewport_px = (width_px, height_px);
        state
    }

    /// Applies the URL policy. Synthetic-only schemes pass when the synthetic engine is running.
    fn admit(&self, url: &str) -> Result<String> {
        if self.engine.backend_name() == "synthetic" && is_synthetic_url(url.trim()) {
            return Ok(url.trim().to_string());
        }
        validate_url(url, file_urls_enabled())
            .map_err(|e| browser_error(std::io::ErrorKind::InvalidInput, format!("blocked URL: {e}")))
    }

    /// Checks `url` against the URL policy without navigating.
    pub fn check_url(url: &str) -> Result<String> {
        if is_synthetic_url(url.trim()) {
            return Ok(url.trim().to_string());
        }
        validate_url(url, file_urls_enabled())
            .map_err(|e| browser_error(std::io::ErrorKind::InvalidInput, format!("blocked URL: {e}")))
    }

    fn track<T>(&mut self, result: Result<T>) -> Result<T> {
        self.notice = result.as_ref().err().map(browser_error_text);
        result
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        let result = self.admit(url).and_then(|u| self.engine.navigate(&u));
        self.cached_frame = self.engine.latest_frame().or(self.cached_frame.take());
        self.track(result)
    }

    pub fn go_back(&mut self) -> Result<()> {
        let result = self.engine.go_back();
        self.refresh_frame();
        self.track(result)
    }

    pub fn go_forward(&mut self) -> Result<()> {
        let result = self.engine.go_forward();
        self.refresh_frame();
        self.track(result)
    }

    /// Reloads the page. After a crash this relaunches the browser on the last URL.
    pub fn reload(&mut self) -> Result<()> {
        if matches!(self.engine.health(), EngineHealth::Crashed(_)) {
            let url = self.engine.current_url().to_string();
            let (focused, zoom) = (self.viewport_focused, self.zoom_pct);
            let (w, h) = self.viewport_px;
            *self = match self.pane_id {
                Some(id) => Self::new_cdp_or_synthetic(&url, w, h, id),
                None => Self::new_synthetic(&url, w, h),
            };
            self.viewport_focused = focused;
            self.zoom_pct = zoom;
            return self.apply_viewport();
        }
        let result = self.engine.reload();
        self.refresh_frame();
        self.track(result)
    }

    pub fn stop(&mut self) -> Result<()> {
        self.engine.stop()
    }

    fn refresh_frame(&mut self) {
        if let Some(f) = self.engine.latest_frame() {
            self.cached_frame = Some(f);
        }
    }

    fn apply_viewport(&mut self) -> Result<()> {
        let (w, h) = self.viewport_px;
        let result = self.engine.resize_viewport(w, h, f32::from(self.zoom_pct) / 100.0);
        self.refresh_frame();
        result
    }

    pub fn resize_viewport(&mut self, width_px: u16, height_px: u16, scale_factor: f32) -> Result<()> {
        self.viewport_px = (width_px, height_px);
        let result = self.engine.resize_viewport(width_px, height_px, scale_factor);
        self.refresh_frame();
        result
    }

    /// Sets the page size in frame pixels (zoom is applied on top). No-op when unchanged.
    pub fn set_viewport_px(&mut self, width_px: u16, height_px: u16) -> bool {
        if self.viewport_px == (width_px, height_px) {
            return false;
        }
        self.viewport_px = (width_px, height_px);
        let _ = self.apply_viewport();
        true
    }

    pub fn viewport_px(&self) -> (u16, u16) {
        self.viewport_px
    }

    pub fn set_zoom(&mut self, zoom_pct: u16) {
        let next = zoom_pct.clamp(25, 500);
        if next != self.zoom_pct {
            self.zoom_pct = next;
            let _ = self.apply_viewport();
        }
    }

    pub fn zoom_delta(&mut self, delta_pct: i16) {
        let next = (i32::from(self.zoom_pct) + i32::from(delta_pct)).clamp(25, 500) as u16;
        self.set_zoom(next);
    }

    pub fn toggle_focus(&mut self) {
        self.viewport_focused = !self.viewport_focused;
    }

    pub fn set_focus(&mut self, focused: bool) {
        self.viewport_focused = focused;
    }

    pub fn dispatch_mouse(&mut self, event: MediaMouseEvent) -> Result<()> {
        let result = self.engine.dispatch_mouse(event);
        self.refresh_frame();
        result
    }

    pub fn dispatch_key(&mut self, key: KeyEvent) -> Result<()> {
        let result = self.engine.dispatch_key(key);
        self.refresh_frame();
        result
    }

    /// Pastes `text` into the URL editor when it is open, otherwise into the page.
    pub fn paste_text(&mut self, text: &str) -> Result<()> {
        if let Some(edit) = self.url_edit.as_mut() {
            let clean: String = text.chars().filter(|c| !c.is_control()).collect();
            let at = edit.text.char_indices().nth(edit.cursor).map_or(edit.text.len(), |(b, _)| b);
            edit.text.insert_str(at, &clean);
            edit.cursor += clean.chars().count();
            return Ok(());
        }
        self.engine.insert_text(text)
    }

    /// Frame pixel under cell `(col, row)` of the viewport `body` rect, for mouse forwarding.
    pub fn frame_px_for_cell(&self, body: Rect, col: u16, row: u16) -> Option<(u16, u16)> {
        let (w, h) = self.cached_frame.as_ref().map_or(self.viewport_px, |f| (f.width_px, f.height_px));
        geometry::cell_to_frame_px(body, col, row, w, h)
    }

    /// Pulls engine state and the newest frame. `true` when something visible changed.
    pub fn poll_screencast_frame(&mut self) -> bool {
        let mut changed = self.engine.poll();
        match self.engine.health() {
            EngineHealth::Failed(reason) => {
                let url = self.engine.current_url().to_string();
                let (w, h) = self.viewport_px;
                self.engine = Box::new(SyntheticBrowserBackend::new(&url, w, h));
                self.fallback = Some(format!("launch failed: {reason}"));
                self.cached_frame = self.engine.latest_frame();
                return true;
            }
            EngineHealth::Crashed(_) => {
                // Never leave a frozen frame on screen after a crash.
                return self.cached_frame.take().is_some() || changed;
            }
            EngineHealth::Starting | EngineHealth::Ready => {}
        }
        if let Some(f) = self.engine.latest_frame()
            && self.cached_frame.as_ref().map(|c| c.seq) != Some(f.seq)
        {
            self.cached_frame = Some(f);
            changed = true;
        }
        changed
    }

    pub fn is_editing_url(&self) -> bool {
        self.url_edit.is_some()
    }

    pub fn url_edit(&self) -> Option<&UrlEditState> {
        self.url_edit.as_ref()
    }

    /// Enters URL edit mode prefilled with the current URL (cursor at end).
    pub fn begin_url_edit(&mut self) {
        let text = self.engine.current_url().to_string();
        let cursor = text.chars().count();
        self.url_edit = Some(UrlEditState { text, cursor });
    }

    pub fn cancel_url_edit(&mut self) {
        self.url_edit = None;
    }

    /// Commits the edit, navigating to the buffer (bare hosts get `https://`).
    pub fn commit_url_edit(&mut self) -> Result<()> {
        let Some(edit) = self.url_edit.take() else {
            return Ok(());
        };
        let t = edit.text.trim();
        if t.is_empty() {
            return Ok(());
        }
        self.navigate(t)
    }

    /// Routes a key to the URL editor. Returns `true` when consumed (always while editing).
    pub fn handle_url_edit_key(&mut self, key: KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        let Some(edit) = self.url_edit.as_mut() else {
            return false;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let len = edit.text.chars().count();
        let byte_at = |t: &str, i: usize| t.char_indices().nth(i).map_or(t.len(), |(b, _)| b);
        match key.code {
            KeyCode::Enter => {
                let _ = self.commit_url_edit();
            }
            KeyCode::Esc => self.url_edit = None,
            KeyCode::Char('u') if ctrl => {
                edit.text.clear();
                edit.cursor = 0;
            }
            KeyCode::Char(c) if !ctrl => {
                let b = byte_at(&edit.text, edit.cursor);
                edit.text.insert(b, c);
                edit.cursor += 1;
            }
            KeyCode::Backspace if edit.cursor > 0 => {
                let b = byte_at(&edit.text, edit.cursor - 1);
                edit.text.remove(b);
                edit.cursor -= 1;
            }
            KeyCode::Delete if edit.cursor < len => {
                let b = byte_at(&edit.text, edit.cursor);
                edit.text.remove(b);
            }
            KeyCode::Left => edit.cursor = edit.cursor.saturating_sub(1),
            KeyCode::Right => edit.cursor = (edit.cursor + 1).min(len),
            KeyCode::Home => edit.cursor = 0,
            KeyCode::End => edit.cursor = len,
            _ => {}
        }
        true
    }

    pub fn cached_frame(&self) -> Option<&MediaRgbaFrame> {
        self.cached_frame.as_ref()
    }

    pub fn current_url(&self) -> &str {
        self.engine.current_url()
    }

    pub fn page_title(&self) -> &str {
        self.engine.page_title()
    }

    pub fn is_loading(&self) -> bool {
        self.engine.is_loading()
    }

    pub fn can_go_back(&self) -> bool {
        self.engine.can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.engine.can_go_forward()
    }

    pub fn health(&self) -> EngineHealth {
        self.engine.health()
    }

    pub fn is_crashed(&self) -> bool {
        matches!(self.engine.health(), EngineHealth::Crashed(_))
    }

    /// What is really running: `cdp` only after the DevTools handshake succeeded.
    pub fn backend_name(&self) -> String {
        let name = self.engine.backend_name();
        match (self.engine.health(), &self.fallback) {
            (EngineHealth::Starting, _) => format!("{name} (starting)"),
            (EngineHealth::Crashed(_), _) => format!("{name} (crashed)"),
            (EngineHealth::Failed(_), _) => format!("{name} (failed)"),
            (EngineHealth::Ready, Some(why)) if name == "synthetic" => format!("synthetic ({why})"),
            (EngineHealth::Ready, _) => name.to_string(),
        }
    }

    /// Process group id and profile directory of the browser process, when one is running.
    pub fn process_info(&self) -> Option<(i32, std::path::PathBuf)> {
        self.engine.process_info()
    }

    /// Message that replaces the page while there is nothing truthful to show.
    pub fn overlay_message(&self) -> Option<String> {
        match self.engine.health() {
            EngineHealth::Starting if self.cached_frame.is_none() => Some("starting Chrome…".to_string()),
            EngineHealth::Crashed(reason) => Some(format!("{reason} — press r to reload")),
            _ => None,
        }
    }

    /// Last refused URL or navigation error (`net::ERR_…`), for the nav bar.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref().or_else(|| self.engine.nav_error())
    }
}
