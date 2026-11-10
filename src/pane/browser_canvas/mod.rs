//! Chrome Engine Web Browser Canvas state, traits, and synthetic renderer (`WEB-01..02`).

use std::sync::Arc;
use crossterm::event::{KeyEvent, KeyModifiers, MouseButton};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::pane::PaneId;

pub mod cdp;
pub mod cdp_input;
#[cfg(unix)]
pub mod cdp_launch;
pub mod cdp_proto;
pub mod cdp_session;
pub mod chrome_ui;
pub mod geometry;
mod pane_hooks;
pub mod state;
pub mod url_policy;

pub use cdp::CdpBrowserBackend;
pub use chrome_ui::{BrowserNavHit, hit_test_browser_nav_bar, render_browser_nav_bar, render_browser_status_overlay};
pub use state::{BrowserCanvasState, UrlEditState};

/// Shared RGBA frame buffer with monotonic sequence counter for zero-copy GUI texture upload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaRgbaFrame {
    pub seq: u64,
    pub width_px: u16,
    pub height_px: u16,
    pub pts_ms: u64,
    pub rgba: Arc<Vec<u8>>,
}

impl MediaRgbaFrame {
    pub fn new(seq: u64, width_px: u16, height_px: u16, pts_ms: u64, rgba: Vec<u8>) -> Self {
        Self {
            seq,
            width_px,
            height_px,
            pts_ms,
            rgba: Arc::new(rgba),
        }
    }
}

/// Mouse event kind inside a pane's media viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaMouseEventKind {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    Move,
    Wheel { delta_x: i16, delta_y: i16 },
}

/// Mouse event in pixel-space coordinates inside a pane's media viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaMouseEvent {
    pub kind: MediaMouseEventKind,
    pub x_px: u16,
    pub y_px: u16,
    pub modifiers: KeyModifiers,
}

/// Canonical browser operations (`ARCH-R01`, `WEB-01..02`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserOp {
    Toggle,
    Open {
        url: String,
        pane_id: Option<PaneId>,
    },
    Back {
        pane_id: Option<PaneId>,
    },
    Forward {
        pane_id: Option<PaneId>,
    },
    Reload {
        pane_id: Option<PaneId>,
    },
    Stop {
        pane_id: Option<PaneId>,
    },
    FocusViewport {
        pane_id: Option<PaneId>,
        focused: Option<bool>,
    },
    Zoom {
        delta_pct: i16,
        pane_id: Option<PaneId>,
    },
    Click {
        x_px: u16,
        y_px: u16,
        button: u8,
        pane_id: Option<PaneId>,
    },
    Scroll {
        delta_x: i16,
        delta_y: i16,
        pane_id: Option<PaneId>,
    },
    Close {
        pane_id: Option<PaneId>,
    },
}

/// Pluggable Chrome Engine backend trait (`CdpBrowserBackend`, `SyntheticBrowserBackend`).
pub trait BrowserEngine: Send {
    fn navigate(&mut self, url: &str) -> Result<()>;
    fn go_back(&mut self) -> Result<()>;
    fn go_forward(&mut self) -> Result<()>;
    fn reload(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn resize_viewport(&mut self, width_px: u16, height_px: u16, scale_factor: f32) -> Result<()>;
    fn dispatch_mouse(&mut self, event: MediaMouseEvent) -> Result<()>;
    fn dispatch_key(&mut self, key: KeyEvent) -> Result<()>;
    fn latest_frame(&mut self) -> Option<MediaRgbaFrame>;
    fn current_url(&self) -> &str;
    fn page_title(&self) -> &str;
    fn is_loading(&self) -> bool;
    /// `"cdp"` or `"synthetic"`: the engine that is really running.
    fn backend_name(&self) -> &'static str;
    /// Pulls state published by a background engine. `true` when url/title/loading/health changed.
    fn poll(&mut self) -> bool {
        false
    }
    fn health(&self) -> EngineHealth {
        EngineHealth::Ready
    }
    /// Inserts pasted text at the page's focused element.
    fn insert_text(&mut self, _text: &str) -> Result<()> {
        Ok(())
    }
    fn can_go_back(&self) -> bool {
        false
    }
    fn can_go_forward(&self) -> bool {
        false
    }
    /// Process group id and profile directory of an out-of-process engine.
    fn process_info(&self) -> Option<(i32, std::path::PathBuf)> {
        None
    }
    /// Last navigation error reported by the engine (for example `net::ERR_NAME_NOT_RESOLVED`).
    fn nav_error(&self) -> Option<&str> {
        None
    }
}

/// Health of a browser engine, as seen from the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineHealth {
    /// Launched, handshake not finished yet.
    Starting,
    Ready,
    /// Was running, then the page or the browser process died.
    Crashed(String),
    /// Never came up; the canvas falls back to the synthetic engine.
    Failed(String),
}

/// Builds the error used for browser failures (shown in the status line without an I/O prefix).
pub fn browser_error(kind: std::io::ErrorKind, msg: impl Into<String>) -> crate::error::BiltermError {
    crate::error::BiltermError::Io(std::io::Error::new(kind, msg.into()))
}

/// Text of a browser error for status lines.
pub fn browser_error_text(err: &crate::error::BiltermError) -> String {
    match err {
        crate::error::BiltermError::Io(e) => e.to_string(),
        other => other.to_string(),
    }
}

/// Offline, deterministic synthetic browser backend for tests and CI environments.
pub struct SyntheticBrowserBackend {
    url: String,
    title: String,
    width_px: u16,
    height_px: u16,
    loading: bool,
    frame_seq: u64,
    last_click: Option<(u16, u16)>,
    latest_frame: Option<MediaRgbaFrame>,
    history: Vec<String>,
    history_idx: usize,
}

impl SyntheticBrowserBackend {
    pub fn new(initial_url: &str, width_px: u16, height_px: u16) -> Self {
        let mut backend = Self {
            url: initial_url.to_string(),
            title: format!("Synthetic - {initial_url}"),
            width_px: width_px.clamp(64, 1920),
            height_px: height_px.clamp(64, 1080),
            loading: false,
            frame_seq: 0,
            last_click: None,
            latest_frame: None,
            history: vec![initial_url.to_string()],
            history_idx: 0,
        };
        backend.render_synthetic_frame();
        backend
    }

    fn show_history_entry(&mut self, idx: usize) {
        if let Some(url) = self.history.get(idx).cloned() {
            self.history_idx = idx;
            self.title = format!("Synthetic - {url}");
            self.url = url;
        }
        self.render_synthetic_frame();
    }

    fn render_synthetic_frame(&mut self) {
        self.frame_seq += 1;
        let w = self.width_px as usize;
        let h = self.height_px as usize;
        let mut rgba = vec![0u8; w * h * 4];

        // Background color: slate navy
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                rgba[idx] = 24;
                rgba[idx + 1] = 31;
                rgba[idx + 2] = 46;
                rgba[idx + 3] = 255;
            }
        }

        // Header card
        let header_h = 32.min(h);
        for y in 0..header_h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                rgba[idx] = 38;
                rgba[idx + 1] = 48;
                rgba[idx + 2] = 71;
                rgba[idx + 3] = 255;
            }
        }

        // Accent line under header
        if header_h < h {
            for x in 0..w {
                let idx = (header_h * w + x) * 4;
                rgba[idx] = 66;
                rgba[idx + 1] = 135;
                rgba[idx + 2] = 245;
                rgba[idx + 3] = 255;
            }
        }

        // Cursor / click indicator
        if let Some((cx, cy)) = self.last_click {
            let cx = (cx as usize).min(w.saturating_sub(1));
            let cy = (cy as usize).min(h.saturating_sub(1));
            let rad = 6;
            for dy in cy.saturating_sub(rad)..=(cy + rad).min(h.saturating_sub(1)) {
                for dx in cx.saturating_sub(rad)..=(cx + rad).min(w.saturating_sub(1)) {
                    let dist = (dx as i32 - cx as i32).pow(2) + (dy as i32 - cy as i32).pow(2);
                    if dist <= (rad as i32).pow(2) {
                        let idx = (dy * w + dx) * 4;
                        rgba[idx] = 255;
                        rgba[idx + 1] = 215;
                        rgba[idx + 2] = 0;
                        rgba[idx + 3] = 255;
                    }
                }
            }
        }

        self.latest_frame = Some(MediaRgbaFrame::new(
            self.frame_seq,
            self.width_px,
            self.height_px,
            self.frame_seq * 16,
            rgba,
        ));
    }
}

impl BrowserEngine for SyntheticBrowserBackend {
    fn navigate(&mut self, url: &str) -> Result<()> {
        self.url = url.to_string();
        self.title = format!("Synthetic - {url}");
        self.history.truncate(self.history_idx + 1);
        self.history.push(url.to_string());
        self.history_idx = self.history.len() - 1;
        self.render_synthetic_frame();
        Ok(())
    }

    fn go_back(&mut self) -> Result<()> {
        self.show_history_entry(self.history_idx.saturating_sub(1));
        Ok(())
    }

    fn go_forward(&mut self) -> Result<()> {
        self.show_history_entry((self.history_idx + 1).min(self.history.len().saturating_sub(1)));
        Ok(())
    }

    fn can_go_back(&self) -> bool {
        self.history_idx > 0
    }

    fn can_go_forward(&self) -> bool {
        self.history_idx + 1 < self.history.len()
    }

    fn reload(&mut self) -> Result<()> {
        self.render_synthetic_frame();
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.loading = false;
        Ok(())
    }

    fn resize_viewport(&mut self, width_px: u16, height_px: u16, _scale_factor: f32) -> Result<()> {
        self.width_px = width_px.clamp(64, 1920);
        self.height_px = height_px.clamp(64, 1080);
        self.render_synthetic_frame();
        Ok(())
    }

    fn dispatch_mouse(&mut self, event: MediaMouseEvent) -> Result<()> {
        self.last_click = Some((event.x_px, event.y_px));
        self.render_synthetic_frame();
        Ok(())
    }

    fn dispatch_key(&mut self, _key: KeyEvent) -> Result<()> {
        self.render_synthetic_frame();
        Ok(())
    }

    fn latest_frame(&mut self) -> Option<MediaRgbaFrame> {
        self.latest_frame.clone()
    }

    fn current_url(&self) -> &str {
        &self.url
    }

    fn page_title(&self) -> &str {
        &self.title
    }

    fn is_loading(&self) -> bool {
        self.loading
    }

    fn backend_name(&self) -> &'static str {
        "synthetic"
    }
}
