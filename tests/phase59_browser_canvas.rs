//! Integration and surface uniformity tests for Chrome Engine Web Browser Canvas (`WEB-01..02`, `UNI-R01`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use bilterm::action::{Action, BrowserOp};
use bilterm::api::json::JsonValue;
use bilterm::api::rpc::AutomationControlPlane;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::handle_wm_key_event;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::pane::browser_canvas::{BrowserNavHit, hit_test_browser_nav_bar, render_browser_nav_bar};
use bilterm::ui::media_canvas_gui::render_media_canvas_textures;
use bilterm::wm::WindowManager;

#[test]
fn browser_canvas_state_synthetic_lifecycle() {
    // Force synthetic browser for deterministic offline testing
    // SAFETY: Process-local test environment variable
    unsafe {
        std::env::set_var("BILTERM_BROWSER_SYNTHETIC", "1");
    }

    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();

    // 1. Open via colon command
    let cmd = parse_colon_command(":browser open https://example.com")
        .expect("colon command")
        .expect("valid syntax");
    assert_eq!(
        cmd,
        Action::Browser(BrowserOp::Open {
            url: "https://example.com".to_string(),
            pane_id: None,
        })
    );
    wm.apply(cmd).unwrap();

    let initial_seq = {
        let pane = wm.pane(p1).unwrap();
        let browser = pane.browser_canvas().expect("active browser canvas");
        assert_eq!(browser.current_url(), "https://example.com");
        assert_eq!(browser.backend_name(), "synthetic");
        let initial_frame = browser.cached_frame().expect("initial frame cached");
        assert!(initial_frame.seq >= 1);
        assert_eq!(initial_frame.rgba.len(), (initial_frame.width_px as usize) * (initial_frame.height_px as usize) * 4);
        initial_frame.seq
    };

    // 2. Navigate
    wm.apply(Action::Browser(BrowserOp::Open {
        url: "https://rust-lang.org".to_string(),
        pane_id: None,
    }))
    .unwrap();
    {
        let pane = wm.pane(p1).unwrap();
        let browser = pane.browser_canvas().unwrap();
        assert_eq!(browser.current_url(), "https://rust-lang.org");
        let f2 = browser.cached_frame().unwrap();
        assert!(f2.seq > initial_seq);
    }

    // 3. Back & Forward
    wm.apply(Action::Browser(BrowserOp::Back { pane_id: None })).unwrap();
    assert_eq!(wm.pane(p1).unwrap().browser_canvas().unwrap().current_url(), "https://example.com");

    wm.apply(Action::Browser(BrowserOp::Forward { pane_id: None })).unwrap();
    assert_eq!(wm.pane(p1).unwrap().browser_canvas().unwrap().current_url(), "https://rust-lang.org");

    // 4. Zoom
    wm.apply(Action::Browser(BrowserOp::Zoom { delta_pct: 25, pane_id: None })).unwrap();
    assert_eq!(wm.pane(p1).unwrap().browser_canvas().unwrap().zoom_pct, 125);

    // 5. Close
    wm.apply(Action::Browser(BrowserOp::Close { pane_id: None })).unwrap();
    assert!(wm.pane(p1).unwrap().browser_canvas().is_none());
}

#[test]
fn browser_canvas_nav_bar_and_mouse_hit_testing() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    wm.pane_mut(p1).unwrap().open_browser("https://bilterm.dev");

    let screen = Rect::new(0, 0, 80, 24);
    let canvas_rect = wm.pane(p1).unwrap().canvas_rect(screen);
    let nav_rect = Rect::new(canvas_rect.x, canvas_rect.y, canvas_rect.width, 1);

    // Hit test buttons
    assert_eq!(hit_test_browser_nav_bar(nav_rect, nav_rect.x + 1, nav_rect.y), Some(BrowserNavHit::Back));
    assert_eq!(hit_test_browser_nav_bar(nav_rect, nav_rect.x + 5, nav_rect.y), Some(BrowserNavHit::Forward));
    assert_eq!(hit_test_browser_nav_bar(nav_rect, nav_rect.x + 9, nav_rect.y), Some(BrowserNavHit::Reload));
    assert_eq!(hit_test_browser_nav_bar(nav_rect, nav_rect.right() - 2, nav_rect.y), Some(BrowserNavHit::Close));

    // Render bar into Buffer
    let mut buf = Buffer::empty(nav_rect);
    let browser = wm.pane(p1).unwrap().browser_canvas().unwrap();
    render_browser_nav_bar(&mut buf, nav_rect, browser, true);

    let row_str: String = (nav_rect.x..nav_rect.right()).map(|c| buf[(c, nav_rect.y)].symbol().chars().next().unwrap_or(' ')).collect();
    assert!(row_str.contains("[◄]"));
    assert!(row_str.contains("[►]"));
    assert!(row_str.contains("[✕]"));
    assert!(row_str.contains("bilterm.dev"));

    // Click Close button via mouse handler
    let close_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: nav_rect.right() - 2,
        row: nav_rect.y,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, close_click, screen).unwrap();
    assert!(wm.pane(p1).unwrap().browser_canvas().is_none(), "Close button closes browser");
}

#[test]
fn browser_canvas_gui_texture_with_prompt_cutout() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();
    wm.pane_mut(p1).unwrap().open_browser("https://example.com");

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
    assert_eq!(img.id, 910_000 + p1 as u32);
    assert!(img.width_px > 0);
    assert!(img.height_px > 0);

    // Verify cutout: cells inside prompt_rect have alpha == 0
    let eff_cw = 10;
    let eff_ch = 18;
    let v_y = canvas_rect.y + 1;
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
fn browser_canvas_uni_r01_parity() {
    let mut wm = WindowManager::new_headless(24, 80);
    let p1 = wm.active_pane_id();

    // 1. Key chord: Leader . toggles browser
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE)).unwrap();
    // Leader was not pending yet, so press leader first
    let leader_chord = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    handle_wm_key_event(&mut wm, leader_chord).unwrap();
    assert!(wm.is_leader_pending());
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE)).unwrap();
    assert!(wm.pane(p1).unwrap().browser_canvas().is_some(), "Leader . opened browser");

    // 2. Global chord: Ctrl+Alt+B toggles browser off
    let ctrl_alt_b = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL | KeyModifiers::ALT);
    handle_wm_key_event(&mut wm, ctrl_alt_b).unwrap();
    assert!(wm.pane(p1).unwrap().browser_canvas().is_none(), "Ctrl+Alt+B closed browser");

    // 3. Colon command: :bcanvas alias
    let cmd = parse_colon_command(":bcanvas open https://rust-lang.org").unwrap().unwrap();
    wm.apply(cmd).unwrap();
    assert_eq!(
        wm.pane(p1).unwrap().browser_canvas().unwrap().current_url(),
        "https://rust-lang.org"
    );

    // 4. RPC: browser.control back & browser.state
    let mut cp = AutomationControlPlane::new_headless(24, 80);
    cp.dispatch_method("browser.open", &JsonValue::parse(r#"{"url":"https://crates.io"}"#).unwrap()).unwrap();
    let state = cp.dispatch_method("browser.state", &JsonValue::parse("{}").unwrap()).unwrap();
    assert_eq!(state.get("active").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(state.get("url").and_then(JsonValue::as_str), Some("https://crates.io"));

    // 5. MCP tools/call
    let mcp_call = r#"{"jsonrpc":"2.0","id":99,"method":"tools/call","params":{"name":"bilterm_browser","arguments":{"action":"close"}}}"#;
    let resp_str = cp.handle_mcp_line(mcp_call).expect("mcp response");
    let resp_json = JsonValue::parse(&resp_str).unwrap();
    assert!(resp_json.get("result").is_some());
    let state_after = cp.dispatch_method("browser.state", &JsonValue::parse("{}").unwrap()).unwrap();
    assert_eq!(state_after.get("active").and_then(JsonValue::as_bool), Some(false));
}
