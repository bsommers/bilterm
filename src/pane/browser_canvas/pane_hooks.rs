//! `CompositePane` hooks for the browser canvas: opening and viewport sizing (`WEB-01`).
//!
//! Kept here so `src/pane/mod.rs` only carries one-line calls.

use crate::pane::CompositePane;

use super::geometry::viewport_px_for_cells;
use super::{BrowserCanvasState, browser_error_text};

impl CompositePane {
    /// Page size in pixels for the current canvas (`last_canvas_size` is `(rows, cols)`).
    fn browser_viewport_px(&self) -> (u16, u16) {
        let (rows, cols) = self.last_canvas_size;
        viewport_px_for_cells(cols, rows)
    }

    /// Opens the browser canvas on `url`. A URL refused by the policy opens nothing and is
    /// reported in the status line.
    pub(in crate::pane) fn open_browser_canvas(&mut self, url: &str) {
        let url = match BrowserCanvasState::check_url(url) {
            Ok(u) => u,
            Err(e) => {
                self.set_status_message(Some(format!("Browser: {}", browser_error_text(&e))));
                return;
            }
        };
        let (width_px, height_px) = self.browser_viewport_px();
        self.browser_canvas = Some(BrowserCanvasState::new_cdp_or_synthetic(&url, width_px, height_px, self.id));
    }

    /// Resizes the page to the pane's canvas. Called whenever the canvas cell size changes.
    pub(in crate::pane) fn sync_browser_viewport(&mut self) {
        let (width_px, height_px) = self.browser_viewport_px();
        if let Some(b) = self.browser_canvas.as_mut() {
            b.set_viewport_px(width_px, height_px);
        }
    }
}

impl CompositePane {
    /// `true` when this pane shows a browser or video canvas.
    pub fn has_media_canvas(&self) -> bool {
        self.browser_canvas.is_some() || self.video_canvas.is_some()
    }
}
