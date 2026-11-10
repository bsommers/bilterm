//! OIA surface-mode badge (`[TUI]` / `[GUI]`) and `:voice` pre-flight diagnostics.
//!
//! (a) `render_oia_status_bar_with_surface` shows `[TUI]` / `[GUI]` per `wm.is_gui_surface()`;
//!     the legacy `render_oia_status_bar*` entry points stay byte-identical (Classic goldens).
//! (b) `:voice status` reports recorder availability + provider/endpoint; `:voice start|toggle`
//!     surfaces actionable hints when no recorder exists or the whisper endpoint is unreachable.

use bilterm::action::{Action, VoiceOp};
use bilterm::stt::{SttBackendKind, VoicePreflight};
use bilterm::ui::glyphs::GlyphSet;
use bilterm::ui::oia::{
    oia_surface_badge, render_oia_status_bar_themed, render_oia_status_bar_with_surface,
};
use bilterm::ui::theme::ThemePalette;
use bilterm::wm::WindowManager;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

const UNREACHABLE: &str = "http://127.0.0.1:1/v1/audio/transcriptions";

fn row_text(buf: &Buffer, w: u16) -> String {
    (0..w).map(|x| buf[(x, 0)].symbol().to_string()).collect()
}

fn render_with_surface(wm: &WindowManager, w: u16) -> String {
    let rect = Rect::new(0, 0, w, 1);
    let mut buf = Buffer::empty(rect);
    render_oia_status_bar_with_surface(
        wm.active_pane(),
        rect,
        &ThemePalette::default(),
        &GlyphSet::default(),
        wm.is_gui_surface(),
        &mut buf,
    );
    row_text(&buf, w)
}

fn status_msg(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or_default().to_string()
}

// ---------- (a) OIA surface badge ----------

#[test]
fn oia_badge_reads_tui_on_terminal_surface() {
    let wm = WindowManager::new_headless(24, 120);
    assert!(!wm.is_gui_surface());
    assert_eq!(oia_surface_badge(false), "[TUI]");
    let row = render_with_surface(&wm, 120);
    assert!(row.contains("[TUI]"), "got: {row}");
    assert!(!row.contains("[GUI]"), "got: {row}");
}

#[test]
fn oia_badge_reads_gui_on_gui_surface() {
    let mut wm = WindowManager::new_headless(24, 120);
    wm.set_gui_surface(true);
    assert!(wm.is_gui_surface());
    assert_eq!(oia_surface_badge(true), "[GUI]");
    let row = render_with_surface(&wm, 120);
    assert!(row.contains("[GUI]"), "got: {row}");
    assert!(!row.contains("[TUI]"), "got: {row}");
}

#[test]
fn legacy_oia_entry_point_has_no_surface_badge() {
    let wm = WindowManager::new_headless(24, 120);
    let rect = Rect::new(0, 0, 120, 1);
    let mut buf = Buffer::empty(rect);
    render_oia_status_bar_themed(
        wm.active_pane(),
        rect,
        &ThemePalette::default(),
        &GlyphSet::default(),
        &mut buf,
    );
    let row = row_text(&buf, 120);
    assert!(!row.contains("[TUI]") && !row.contains("[GUI]"), "got: {row}");
}

#[test]
fn surface_badge_is_dropped_on_narrow_rows() {
    let wm = WindowManager::new_headless(24, 20);
    let row = render_with_surface(&wm, 20);
    assert!(!row.contains("[TUI]"), "narrow row must not clobber the head: {row}");
}

// ---------- (b) voice pre-flight & status ----------

#[test]
fn voice_status_reports_recorder_provider_and_endpoint() {
    let mut wm = WindowManager::new_headless(24, 120);
    wm.config_mut().stt.endpoint = UNREACHABLE.to_string();
    wm.apply(Action::Voice(VoiceOp::Status)).expect("status");
    let msg = status_msg(&wm);
    assert!(msg.contains("provider=openai_compatible"), "got: {msg}");
    assert!(msg.contains(&format!("endpoint={UNREACHABLE}")), "got: {msg}");
    assert!(msg.contains("recorder="), "got: {msg}");
    let data = wm.take_command_data().expect("status json");
    assert!(data.contains("\"recorder_available\""), "{data}");
    assert!(data.contains("\"endpoint\""), "{data}");
    assert!(data.contains("\"endpoint_reachable\":false"), "{data}");
}

#[test]
fn voice_status_reports_missing_recorder() {
    let mut wm = WindowManager::new_headless(24, 120);
    wm.config_mut().stt.audio_backend = "no-such-recorder".to_string();
    wm.apply(Action::Voice(VoiceOp::Status)).expect("status");
    let msg = status_msg(&wm);
    assert!(msg.contains("recorder=missing"), "got: {msg}");
    let data = wm.take_command_data().expect("status json");
    assert!(data.contains("\"recorder_available\":false"), "{data}");
}

#[test]
fn voice_start_without_recorder_surfaces_install_hint() {
    let mut wm = WindowManager::new_headless(24, 120);
    wm.config_mut().stt.audio_backend = "no-such-recorder".to_string();
    wm.apply(Action::Voice(VoiceOp::Start { pane_id: None })).expect("start");
    let msg = status_msg(&wm);
    assert!(msg.contains("Hint:"), "got: {msg}");
    assert!(msg.contains("audio_backend"), "got: {msg}");
    assert!(wm.stt_status().last_error.is_some());
}

#[test]
fn voice_toggle_with_unreachable_endpoint_surfaces_whisper_hint() {
    let mut wm = WindowManager::new_headless(24, 120);
    wm.config_mut().stt.audio_backend = "mock".to_string();
    wm.config_mut().stt.endpoint = UNREACHABLE.to_string();
    wm.apply(Action::Voice(VoiceOp::Toggle)).expect("toggle");
    let msg = status_msg(&wm);
    assert!(msg.contains("unreachable"), "got: {msg}");
    assert!(msg.contains("Hint:"), "got: {msg}");
    assert!(msg.contains(":voice provider"), "got: {msg}");
}

#[test]
fn voice_start_with_healthy_setup_has_no_hint() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let mut wm = WindowManager::new_headless(24, 120);
    wm.config_mut().stt.audio_backend = "mock".to_string();
    wm.config_mut().stt.endpoint = format!("http://127.0.0.1:{port}/v1/audio/transcriptions");
    wm.apply(Action::Voice(VoiceOp::Start { pane_id: None })).expect("start");
    let msg = status_msg(&wm);
    assert!(wm.stt_status().recording);
    assert!(!msg.contains("Hint:"), "got: {msg}");
}

#[test]
fn mock_provider_skips_endpoint_probe() {
    let cfg = bilterm::stt::SttConfig {
        endpoint: UNREACHABLE.to_string(),
        ..Default::default()
    };
    let pf = VoicePreflight::run(&cfg, SttBackendKind::Mock);
    assert!(pf.recorder.is_ok());
    assert!(pf.endpoint_status.is_ok());
    assert!(pf.is_ready());
    assert!(pf.hints().is_empty());
}

// ---------- (c) live workspace renderer wiring ----------

mod live_renderer {
    use bilterm::ui::WorkspaceWidget;
    use bilterm::ui::design::AppearanceProfile;
    use bilterm::wm::WindowManager;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    fn screen(profile: Option<AppearanceProfile>, gui: bool) -> String {
        let mut wm = WindowManager::new_headless(20, 100);
        wm.config_mut().appearance.profile = profile;
        wm.set_gui_surface(gui);
        let area = Rect::new(0, 0, 100, 20);
        wm.resize_all(area).unwrap();
        let mut buf = Buffer::empty(area);
        WorkspaceWidget::new(&wm).render(area, &mut buf);
        (0..area.height)
            .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn non_classic_profile_shows_tui_badge() {
        let s = screen(Some(AppearanceProfile::Obsidian), false);
        assert!(s.contains("[TUI]"), "{s}");
    }

    #[test]
    fn non_classic_profile_shows_gui_badge_on_gui_surface() {
        let s = screen(Some(AppearanceProfile::Obsidian), true);
        assert!(s.contains("[GUI]") && !s.contains("[TUI]"), "{s}");
    }

    #[test]
    fn classic_profile_never_shows_surface_badge() {
        for gui in [false, true] {
            let s = screen(None, gui);
            assert!(!s.contains("[TUI]") && !s.contains("[GUI]"), "{s}");
            let s = screen(Some(AppearanceProfile::Legacy), gui);
            assert!(!s.contains("[TUI]") && !s.contains("[GUI]"), "{s}");
        }
    }
}
