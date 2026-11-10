//! Keyboard and paste routing for the browser canvas (`WEB-02`).
//!
//! - While the URL bar is being edited every key goes to the editor.
//! - `Ctrl+]` toggles viewport focus on a pane that shows a browser canvas. It is the escape
//!   hatch: it always hands the keyboard back to bilterm.
//! - With the viewport focused, keys go to the page (`Input.dispatchKeyEvent`). `Ctrl+L` edits the
//!   URL, and `r` reloads a crashed page. Leader chords, user bindings and reserved chords are
//!   resolved before this, so they keep working.
//! - Without focus nothing is taken: `Ctrl+L` still reaches the shell.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::input::AppAction;
use crate::wm::WindowManager;

/// Status-line hint shown when the page takes the keyboard.
pub const FOCUS_HINT: &str = "Browser focus: keys go to the page. Ctrl+] returns to the prompt";

/// `Ctrl+]`. Legacy terminals report it as `Ctrl+5` (byte 0x1D).
pub fn is_focus_toggle_key(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::CONTROL && matches!(key.code, KeyCode::Char(']') | KeyCode::Char('5'))
}

/// URL-bar editing and the focus toggle. Runs before modal and leader handling.
pub(super) fn handle_browser_chrome_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let pane = wm.active_pane_mut();
    let b = pane.browser_canvas_mut()?;
    if b.is_editing_url() {
        if key.kind != KeyEventKind::Release {
            b.handle_url_edit_key(key);
        }
        return Some(AppAction::Handled);
    }
    if is_focus_toggle_key(key) {
        if key.kind == KeyEventKind::Press {
            b.toggle_focus();
            let focused = b.viewport_focused;
            pane.set_status_message(Some(if focused { FOCUS_HINT.to_string() } else { "Browser focus: prompt".to_string() }));
        }
        return Some(AppAction::Handled);
    }
    None
}

/// Forwards a key to the page when the browser viewport has focus.
pub(super) fn forward_browser_viewport_key(wm: &mut WindowManager, key: KeyEvent) -> Option<AppAction> {
    let b = wm.active_pane_mut().browser_canvas_mut()?;
    if !b.viewport_focused {
        return None;
    }
    if key.kind == KeyEventKind::Release {
        return Some(AppAction::Handled);
    }
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('l') {
        b.begin_url_edit();
    } else if b.is_crashed() && key.modifiers.is_empty() && key.code == KeyCode::Char('r') {
        let _ = b.reload();
    } else {
        let _ = b.dispatch_key(key);
    }
    Some(AppAction::Handled)
}

/// Bracketed paste: into the URL editor when it is open, into the page when it has focus.
pub(super) fn handle_browser_paste(wm: &mut WindowManager, text: &str) -> Option<AppAction> {
    let b = wm.active_pane_mut().browser_canvas_mut()?;
    if !b.is_editing_url() && !b.viewport_focused {
        return None;
    }
    let _ = b.paste_text(text);
    Some(AppAction::Handled)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::error::Result;
    use crate::pane::browser_canvas::{BrowserCanvasState, BrowserEngine, EngineHealth, MediaMouseEvent, MediaRgbaFrame};

    #[derive(Default)]
    struct Log {
        keys: Vec<KeyEvent>,
        pasted: Vec<String>,
        crashed: bool,
    }

    struct RecordingEngine(Arc<Mutex<Log>>);

    impl BrowserEngine for RecordingEngine {
        fn navigate(&mut self, _url: &str) -> Result<()> { Ok(()) }
        fn go_back(&mut self) -> Result<()> { Ok(()) }
        fn go_forward(&mut self) -> Result<()> { Ok(()) }
        fn reload(&mut self) -> Result<()> { Ok(()) }
        fn stop(&mut self) -> Result<()> { Ok(()) }
        fn resize_viewport(&mut self, _w: u16, _h: u16, _s: f32) -> Result<()> { Ok(()) }
        fn dispatch_mouse(&mut self, _e: MediaMouseEvent) -> Result<()> { Ok(()) }
        fn dispatch_key(&mut self, k: KeyEvent) -> Result<()> {
            self.0.lock().unwrap().keys.push(k);
            Ok(())
        }
        fn insert_text(&mut self, text: &str) -> Result<()> {
            self.0.lock().unwrap().pasted.push(text.to_string());
            Ok(())
        }
        fn latest_frame(&mut self) -> Option<MediaRgbaFrame> { None }
        fn current_url(&self) -> &str { "https://example.com/" }
        fn page_title(&self) -> &str { "" }
        fn is_loading(&self) -> bool { false }
        fn backend_name(&self) -> &'static str { "cdp" }
        fn health(&self) -> EngineHealth {
            if self.0.lock().unwrap().crashed { EngineHealth::Crashed("page crashed".into()) } else { EngineHealth::Ready }
        }
    }

    fn wm_with_engine() -> (WindowManager, Arc<Mutex<Log>>) {
        let mut wm = WindowManager::new_headless(24, 80);
        let log = Arc::new(Mutex::new(Log::default()));
        wm.active_pane_mut().browser_canvas = Some(BrowserCanvasState::new(Box::new(RecordingEngine(Arc::clone(&log)))));
        (wm, log)
    }

    fn press(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
        crate::input::handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
    }

    fn browser(wm: &mut WindowManager) -> &mut BrowserCanvasState {
        wm.active_pane_mut().browser_canvas_mut().unwrap()
    }

    #[test]
    fn keys_reach_the_engine_only_while_the_viewport_is_focused() {
        let (mut wm, log) = wm_with_engine();
        press(&mut wm, KeyCode::Char('a'), KeyModifiers::NONE);
        press(&mut wm, KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert!(log.lock().unwrap().keys.is_empty());
        assert!(!browser(&mut wm).is_editing_url(), "Ctrl+L is left to the shell");

        press(&mut wm, KeyCode::Char(']'), KeyModifiers::CONTROL);
        assert!(browser(&mut wm).viewport_focused);
        for (code, mods) in [
            (KeyCode::Char('a'), KeyModifiers::NONE),
            (KeyCode::Char('A'), KeyModifiers::SHIFT),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::F(5), KeyModifiers::NONE),
            (KeyCode::Char('a'), KeyModifiers::CONTROL),
        ] {
            press(&mut wm, code, mods);
        }
        let codes: Vec<KeyCode> = log.lock().unwrap().keys.iter().map(|k| k.code).collect();
        assert_eq!(
            codes,
            [KeyCode::Char('a'), KeyCode::Char('A'), KeyCode::Tab, KeyCode::Esc, KeyCode::F(5), KeyCode::Char('a')]
        );

        // Key releases (kitty protocol) are swallowed, not sent twice.
        let mut release = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        crate::input::handle_wm_key_event(&mut wm, release).unwrap();
        assert_eq!(log.lock().unwrap().keys.len(), 6);

        press(&mut wm, KeyCode::Char(']'), KeyModifiers::CONTROL);
        assert!(!browser(&mut wm).viewport_focused);
        press(&mut wm, KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(log.lock().unwrap().keys.len(), 6, "escape hatch returned the keyboard");
    }

    #[test]
    fn ctrl_l_edits_the_url_only_when_focused_and_paste_follows_focus() {
        let (mut wm, log) = wm_with_engine();
        assert!(handle_browser_paste(&mut wm, "nope").is_none(), "unfocused paste goes to the prompt");
        browser(&mut wm).set_focus(true);
        crate::input::handle_wm_paste_event(&mut wm, "hello\nworld").unwrap();
        assert_eq!(log.lock().unwrap().pasted, ["hello\nworld"]);

        press(&mut wm, KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert!(browser(&mut wm).is_editing_url());
        press(&mut wm, KeyCode::Char('u'), KeyModifiers::CONTROL);
        crate::input::handle_wm_paste_event(&mut wm, "exa\nmple.org").unwrap();
        assert_eq!(browser(&mut wm).url_edit().unwrap().text, "example.org", "control characters are stripped");
        assert_eq!(log.lock().unwrap().pasted.len(), 1, "paste went to the URL editor, not the page");
        assert!(log.lock().unwrap().keys.is_empty(), "Ctrl+L and Ctrl+U were not sent to the page");
    }

    #[test]
    fn r_reloads_a_crashed_page_and_is_not_typed_into_it() {
        let (mut wm, log) = wm_with_engine();
        browser(&mut wm).set_focus(true);
        log.lock().unwrap().crashed = true;
        assert_eq!(browser(&mut wm).overlay_message().as_deref(), Some("page crashed — press r to reload"));
        assert_eq!(browser(&mut wm).backend_name(), "cdp (crashed)");
        press(&mut wm, KeyCode::Char('r'), KeyModifiers::NONE);
        assert!(log.lock().unwrap().keys.is_empty());
        assert!(!browser(&mut wm).is_crashed(), "reload replaced the dead engine");
        assert!(browser(&mut wm).viewport_focused, "focus survives the relaunch");
    }
}
