//! Integration and surface uniformity tests for Video Player Canvas (`VID-01..02`, `UNI-R01`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use bilterm::action::{Action, VideoOp};
use bilterm::api::json::JsonValue;
use bilterm::api::rpc::AutomationControlPlane;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::handle_wm_key_event;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::pane::video_canvas::{VideoOsdHit, hit_test_video_osd_bar, render_video_osd_bar};
use bilterm::ui::media_canvas_gui::render_media_canvas_textures;
use bilterm::wm::WindowManager;

#[test]
fn video_canvas_state_synthetic_lifecycle() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();

    // 1. Open via colon command
    let cmd = parse_colon_command(":video open demo://smpte")
        .expect("colon command")
        .expect("valid syntax");
    assert_eq!(
        cmd,
        Action::Video(VideoOp::Open {
            source: "demo://smpte".to_string(),
            pane_id: None,
        })
    );
    wm.apply(cmd).unwrap();

    let pane = wm.pane(p1).unwrap();
    let video = pane.video_canvas().expect("active video canvas");
    let snap = video.snapshot();
    assert_eq!(snap.source, "demo://smpte");
    assert_eq!(snap.backend_name, "synthetic");
    assert!(snap.playing);

    // 2. Tick to advance frame
    let advanced = wm.pane_mut(p1).unwrap().tick_media_canvases(500);
    assert!(advanced, "tick advances frame");
    let f1 = wm.pane(p1).unwrap().video_canvas().unwrap().cached_frame().expect("cached frame");
    assert!(f1.seq >= 1);

    // 3. Play / Pause toggle
    wm.apply(Action::Video(VideoOp::TogglePlayPause { pane_id: None })).unwrap();
    assert!(!wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().playing);

    wm.apply(Action::Video(VideoOp::Play { pane_id: None })).unwrap();
    assert!(wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().playing);

    // 4. Seek Relative & Seek Percent
    wm.apply(Action::Video(VideoOp::SeekRelative { delta_ms: 10_000, pane_id: None })).unwrap();
    assert!(wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().position_ms >= 10_000);

    wm.apply(Action::Video(VideoOp::SeekPercent { pct: 50, pane_id: None })).unwrap();
    let snap_50 = wm.pane(p1).unwrap().video_canvas().unwrap().snapshot();
    assert_eq!(snap_50.position_ms, snap_50.duration_ms / 2);

    // 5. Volume, Mute, Loop, Speed
    wm.apply(Action::Video(VideoOp::VolumeDelta { delta_pct: -20, pane_id: None })).unwrap();
    assert_eq!(wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().volume_pct, 80);

    wm.apply(Action::Video(VideoOp::ToggleMute { pane_id: None })).unwrap();
    assert!(wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().muted);

    wm.apply(Action::Video(VideoOp::SetSpeed { speed_x100: 150, pane_id: None })).unwrap();
    assert_eq!(wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().speed_x100, 150);

    // 6. Close
    wm.apply(Action::Video(VideoOp::Close { pane_id: None })).unwrap();
    assert!(wm.pane(p1).unwrap().video_canvas().is_none());
}

#[test]
fn video_canvas_osd_bar_and_mouse_hit_testing() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    wm.pane_mut(p1).unwrap().open_video("demo://plasma");

    let screen = Rect::new(0, 0, 80, 24);
    let canvas_rect = wm.pane(p1).unwrap().canvas_rect(screen);
    let osd_rect = Rect::new(canvas_rect.x, canvas_rect.bottom().saturating_sub(1), canvas_rect.width, 1);

    // Hit test buttons
    assert_eq!(hit_test_video_osd_bar(osd_rect, osd_rect.x + 1, osd_rect.y), Some(VideoOsdHit::TogglePlayPause));
    assert_eq!(hit_test_video_osd_bar(osd_rect, osd_rect.x + 5, osd_rect.y), Some(VideoOsdHit::SeekBack));
    assert_eq!(hit_test_video_osd_bar(osd_rect, osd_rect.x + 9, osd_rect.y), Some(VideoOsdHit::SeekForward));
    assert_eq!(hit_test_video_osd_bar(osd_rect, osd_rect.right() - 2, osd_rect.y), Some(VideoOsdHit::Close));

    // Render bar into Buffer
    let mut buf = Buffer::empty(osd_rect);
    let video = wm.pane(p1).unwrap().video_canvas().unwrap();
    render_video_osd_bar(&mut buf, osd_rect, video, true);

    let row_str: String = (osd_rect.x..osd_rect.right()).map(|c| buf[(c, osd_rect.y)].symbol().chars().next().unwrap_or(' ')).collect();
    assert!(row_str.contains("[⏸]"));
    assert!(row_str.contains("[⏮]"));
    assert!(row_str.contains("[⏭]"));
    assert!(row_str.contains("[✕]"));

    // Click Play/Pause toggle via mouse handler
    let play_pause_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: osd_rect.x + 1,
        row: osd_rect.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, play_pause_click, screen).unwrap();
    assert!(!wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().playing);

    // Click Close button via mouse handler
    let close_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: osd_rect.right() - 2,
        row: osd_rect.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, close_click, screen).unwrap();
    assert!(wm.pane(p1).unwrap().video_canvas().is_none(), "Close button closes video canvas");
}

#[test]
fn video_canvas_gui_texture_with_prompt_cutout() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    wm.pane_mut(p1).unwrap().open_video("demo://smpte");

    let screen = Rect::new(0, 0, 80, 24);
    let canvas_rect = wm.pane(p1).unwrap().canvas_rect(screen);
    let pr = wm.pane(p1).unwrap().prompt_rect(screen);
    let prompt_rect = Some(pr);

    let mut images = Vec::new();
    render_media_canvas_textures(
        wm.pane(p1).unwrap(),
        canvas_rect,
        prompt_rect,
        p1,
        10,
        18,
        &mut images,
    );

    assert_eq!(images.len(), 1);
    let img = &images[0];
    assert_eq!(img.id, 920_000 + p1 as u32);
    assert!(img.width_px > 0);
    assert!(img.height_px > 0);

    // Verify cutout: cells inside prompt_rect have alpha == 0
    let eff_cw = 10;
    let eff_ch = 18;
    let v_y = canvas_rect.y;
    if pr.y >= v_y {
        let sample_px_x = ((pr.x - canvas_rect.x) * eff_cw + 4) as usize;
        let sample_px_y = ((pr.y - v_y) * eff_ch + 4) as usize;
        let idx = (sample_px_y * (img.width_px as usize) + sample_px_x) * 4;
        if idx + 3 < img.rgba_bytes.len() {
            assert_eq!(img.rgba_bytes[idx + 3], 0, "alpha cutout under floating prompt");
        }
    }
}

#[test]
fn video_canvas_uni_r01_parity() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();

    // 1. Key chord: Leader ' toggles video
    let leader_chord = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    handle_wm_key_event(&mut wm, leader_chord).unwrap();
    assert!(wm.is_leader_pending());
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('\''), KeyModifiers::NONE)).unwrap();
    assert!(wm.pane(p1).unwrap().video_canvas().is_some(), "Leader ' opened video canvas");

    // 2. Global chord: Ctrl+Alt+Y toggles video off
    let ctrl_alt_y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    handle_wm_key_event(&mut wm, ctrl_alt_y).unwrap();
    assert!(wm.pane(p1).unwrap().video_canvas().is_none(), "Ctrl+Alt+Y closed video canvas");

    // 3. Colon command: :vcanvas alias
    let cmd = parse_colon_command(":vcanvas open demo://plasma").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert_eq!(
        wm.pane(p1).unwrap().video_canvas().unwrap().snapshot().source,
        "demo://plasma"
    );

    // 4. RPC: video.control seek & video.state
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    cp.dispatch_method("video.open", &JsonValue::parse(r#"{"source":"demo://smpte"}"#).unwrap()).unwrap();
    let state = cp.dispatch_method("video.state", &JsonValue::parse("{}").unwrap()).unwrap();
    assert_eq!(state.get("active").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(state.get("source").and_then(JsonValue::as_str), Some("demo://smpte"));

    // 5. MCP tools/call
    let mcp_call = r#"{"jsonrpc":"2.0","id":101,"method":"tools/call","params":{"name":"bilterm_video","arguments":{"action":"close"}}}"#;
    let resp_str = cp.handle_mcp_line(mcp_call).expect("mcp response");
    let resp_json = JsonValue::parse(&resp_str).unwrap();
    assert!(resp_json.get("result").is_some());
    let state_after = cp.dispatch_method("video.state", &JsonValue::parse("{}").unwrap()).unwrap();
    assert_eq!(state_after.get("active").and_then(JsonValue::as_bool), Some(false));
}
