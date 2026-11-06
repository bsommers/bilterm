//! Tests for Non-Modal `SESSION MAP` Window & Dual Rendering (Phase 54, VIEW-02).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use bilterm::action::Action;
use bilterm::config::BiltermConfig;
use bilterm::ui::session_map_gui::{draw_session_map_vector, SESSION_MAP_TEXTURE_ID};
use bilterm::ui::session_map_ui::render_session_map;
use bilterm::ui::theme::ThemePalette;
use bilterm::wm::session_map::SessionMapOp;
use bilterm::wm::WindowManager;

#[test]
fn test_session_map_lifecycle_and_non_modal_semantics() {
    let mut wm = WindowManager::new_headless(24, 80);
    assert!(!wm.session_map().active);
    assert!(!wm.session_map().captures_input());

    // 1. Open
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();
    assert!(wm.session_map().active);
    assert!(wm.session_map().focused);
    assert!(wm.session_map().captures_input());

    // 2. Blur releases keyboard without closing window
    wm.apply(Action::SessionMap(SessionMapOp::Blur)).unwrap();
    assert!(wm.session_map().active);
    assert!(!wm.session_map().focused);
    assert!(!wm.session_map().captures_input());

    // 3. Dismiss on unpinned closes
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();
    wm.apply(Action::SessionMap(SessionMapOp::Dismiss)).unwrap();
    assert!(!wm.session_map().active);

    // 4. Pin keeps window open on dismiss
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();
    wm.apply(Action::SessionMap(SessionMapOp::Pin(Some(true)))).unwrap();
    assert!(wm.session_map().pinned);
    wm.apply(Action::SessionMap(SessionMapOp::Dismiss)).unwrap();
    assert!(wm.session_map().active);
    assert!(!wm.session_map().focused); // blurred only

    // 5. Close always closes even if pinned
    wm.apply(Action::SessionMap(SessionMapOp::Close)).unwrap();
    assert!(!wm.session_map().active);
}

#[test]
fn test_session_map_colon_command_parsing() {
    let parse = |cmd: &str| bilterm::colon_command::parse_colon_command(cmd);

    assert_eq!(
        parse(":sessions map"),
        Some(Ok(Action::SessionMap(SessionMapOp::Toggle)))
    );
    assert_eq!(
        parse(":packing"),
        Some(Ok(Action::SessionMap(SessionMapOp::Toggle)))
    );
    assert_eq!(
        parse(":sessions map open"),
        Some(Ok(Action::SessionMap(SessionMapOp::Open)))
    );
    assert_eq!(
        parse(":packing close"),
        Some(Ok(Action::SessionMap(SessionMapOp::Close)))
    );
    assert_eq!(
        parse(":sessions map pin on"),
        Some(Ok(Action::SessionMap(SessionMapOp::Pin(Some(true)))))
    );
    assert_eq!(
        parse(":packing pin off"),
        Some(Ok(Action::SessionMap(SessionMapOp::Pin(Some(false)))))
    );
    assert_eq!(
        parse(":sessions map dock toggle"),
        Some(Ok(Action::SessionMap(SessionMapOp::Dock(None))))
    );
    assert_eq!(
        parse(":sessions map pickup 2"),
        Some(Ok(Action::SessionMap(SessionMapOp::Pickup(Some(2)))))
    );
    assert_eq!(
        parse(":sessions map target dev"),
        Some(Ok(Action::SessionMap(SessionMapOp::TargetSession("dev".to_string()))))
    );
    assert_eq!(
        parse(":sessions map drop"),
        Some(Ok(Action::SessionMap(SessionMapOp::Drop)))
    );
    assert_eq!(
        parse(":sessions map cancel"),
        Some(Ok(Action::SessionMap(SessionMapOp::CancelDrag)))
    );
    assert_eq!(
        parse(":sessions map scale +10"),
        Some(Ok(Action::SessionMap(SessionMapOp::ScaleStep(10))))
    );
}

#[test]
fn test_session_map_tui_rendering_snapshot() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.apply(Action::SessionMap(SessionMapOp::Open)).unwrap();

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let cfg = BiltermConfig::default();
    let palette = ThemePalette::default();
    let glyphs = bilterm::ui::glyphs::GlyphSet::default();

    render_session_map(&wm, area, &cfg, &palette, &glyphs, &mut buf);

    // Extract rendered buffer text
    let mut screen = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let symbol = buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" ");
            screen.push_str(symbol);
        }
        screen.push('\n');
    }

    assert!(screen.contains("SESSION MAP"), "Must render title");
    assert!(screen.contains("[o]"), "Must render unpinned badge");
    assert!(screen.contains("[□]"), "Must render dock badge");
    assert!(screen.contains("[×]"), "Must render close badge");
    assert!(screen.contains("Space/m:Pickup"), "Must render footer key hint");
}

#[test]
fn test_session_map_gui_vector_rendering() {
    let wm = WindowManager::new_headless(24, 80);
    let rect = Rect::new(10, 5, 60, 20);
    let cell_w = 10;
    let cell_h = 20;

    let texture = draw_session_map_vector(&wm, rect, cell_w, cell_h);
    assert!(texture.is_some());
    let tex = texture.unwrap();

    assert_eq!(tex.id, SESSION_MAP_TEXTURE_ID);
    assert_eq!(tex.width_px, 60 * 10);
    assert_eq!(tex.height_px, 20 * 20);
    assert_eq!(tex.rgba_bytes.len(), (600 * 400 * 4) as usize);
}
