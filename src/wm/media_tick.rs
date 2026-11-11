//! Window-manager tick for browser and video canvases (`WEB-01..02`, `VID-01..02`).
//!
//! Every pane with a media canvas is polled so its URL, title and health stay current, but a
//! redraw is requested only when a pane that can be on screen got a new frame or state.

use super::WindowManager;

impl WindowManager {
    /// Polls media canvases. `true` when a visible pane changed and the screen must be redrawn.
    pub(crate) fn tick_media_canvases(&mut self, now_ms: u64) -> bool {
        let zoomed = self.zoomed_pane;
        let mut redraw = false;
        for (&id, pane) in self.panes.iter_mut() {
            if !pane.has_media_canvas() {
                continue;
            }
            let changed = pane.tick_media_canvases(now_ms);
            let visible = zoomed.is_none_or(|z| z == id);
            redraw |= changed && visible;
        }
        let quick = &mut self.quick_terminal;
        if quick.pane.has_media_canvas() {
            let changed = quick.pane.tick_media_canvases(now_ms);
            redraw |= changed && quick.visible;
        }
        redraw
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crossterm::event::KeyEvent;

    use crate::error::Result;
    use crate::pane::browser_canvas::{BrowserCanvasState, BrowserEngine, MediaMouseEvent, MediaRgbaFrame};
    use crate::wm::WindowManager;

    /// Engine whose frame sequence is driven by the test.
    struct ScriptedEngine(Arc<AtomicU64>);

    impl BrowserEngine for ScriptedEngine {
        fn navigate(&mut self, _url: &str) -> Result<()> { Ok(()) }
        fn go_back(&mut self) -> Result<()> { Ok(()) }
        fn go_forward(&mut self) -> Result<()> { Ok(()) }
        fn reload(&mut self) -> Result<()> { Ok(()) }
        fn stop(&mut self) -> Result<()> { Ok(()) }
        fn resize_viewport(&mut self, _w: u16, _h: u16, _s: f32) -> Result<()> { Ok(()) }
        fn dispatch_mouse(&mut self, _e: MediaMouseEvent) -> Result<()> { Ok(()) }
        fn dispatch_key(&mut self, _k: KeyEvent) -> Result<()> { Ok(()) }
        fn latest_frame(&mut self) -> Option<MediaRgbaFrame> {
            Some(MediaRgbaFrame::new(self.0.load(Ordering::SeqCst), 2, 2, 0, vec![0; 16]))
        }
        fn current_url(&self) -> &str { "test://scripted" }
        fn page_title(&self) -> &str { "" }
        fn is_loading(&self) -> bool { false }
        fn backend_name(&self) -> &'static str { "synthetic" }
    }

    fn wm_with_scripted_browser() -> (WindowManager, Arc<AtomicU64>) {
        let mut wm = WindowManager::new_headless(24, 80);
        let seq = Arc::new(AtomicU64::new(1));
        let id = wm.active_pane_id();
        wm.pane_mut(id).unwrap().browser_canvas =
            Some(BrowserCanvasState::new(Box::new(ScriptedEngine(Arc::clone(&seq)))));
        (wm, seq)
    }

    #[test]
    fn tick_requests_a_redraw_only_when_a_frame_changed() {
        let mut plain = WindowManager::new_headless(24, 80);
        assert!(!plain.tick_media_canvases(0), "no media canvas, nothing to redraw");

        let (mut wm, seq) = wm_with_scripted_browser();
        assert!(!wm.tick_media_canvases(1), "the frame cached at open is not new");
        seq.store(2, Ordering::SeqCst);
        assert!(wm.tick_media_canvases(2), "a new frame requests a redraw");
        assert!(!wm.tick_media_canvases(3), "the same frame does not");
    }

    #[test]
    fn hidden_panes_are_polled_but_do_not_request_a_redraw() {
        let (mut wm, seq) = wm_with_scripted_browser();
        let id = wm.active_pane_id();
        wm.zoomed_pane = Some(id + 1000);
        seq.store(7, Ordering::SeqCst);
        assert!(!wm.tick_media_canvases(1), "pane hidden behind a zoomed pane");
        let cached = wm.pane(id).unwrap().browser_canvas().unwrap().cached_frame().map(|f| f.seq);
        assert_eq!(cached, Some(7), "state is still pulled while hidden");
    }

    #[test]
    fn poll_all_ptys_runs_the_media_tick() {
        let (mut wm, seq) = wm_with_scripted_browser();
        wm.poll_all_ptys();
        seq.store(5, Ordering::SeqCst);
        wm.poll_all_ptys();
        let id = wm.active_pane_id();
        assert_eq!(wm.pane(id).unwrap().browser_canvas().unwrap().cached_frame().map(|f| f.seq), Some(5));
    }
}
