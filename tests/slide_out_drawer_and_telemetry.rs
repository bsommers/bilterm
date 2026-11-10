use std::sync::Once;

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::renderer::{
    GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer, WorkspaceFrameSnapshot,
};
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::drawer::BRAILLE_SPINNER_FRAMES;
use bilterm::wm::{DrawerEdge, DrawerLayoutMode, DrawerVisibilityMode, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

static INIT: Once = Once::new();

fn init_env() {
    INIT.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-slideout-drawer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

fn wm_new(rows: u16, cols: u16) -> WindowManager {
    init_env();
    WindowManager::new_headless(rows, cols)
}

#[test]
fn drawer_supports_all_four_edges_and_visibility_modes_including_autohide_triggers() {
    let mut wm = wm_new(24, 100);
    let area = Rect::new(0, 0, 100, 24);

    // Default: OnDemand, Left, Push -> initially hidden
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Left);
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Push);
    assert!(!wm.is_drawer_effectively_visible());
    assert!(wm.drawer_rect(area).is_none());

    // Fixed mode: always visible across all 4 edges
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::Fixed));
    assert!(wm.is_drawer_effectively_visible());

    wm.set_drawer_edge(Some(DrawerEdge::Left));
    let left_rect = wm.drawer_rect(area).expect("left drawer rect");
    assert_eq!(left_rect.x, 0);
    assert_eq!(left_rect.height, 24);

    wm.set_drawer_edge(Some(DrawerEdge::Right));
    let right_rect = wm.drawer_rect(area).expect("right drawer rect");
    assert_eq!(right_rect.x + right_rect.width, 100);
    assert_eq!(right_rect.height, 24);

    wm.set_drawer_edge(Some(DrawerEdge::Top));
    let top_rect = wm.drawer_rect(area).expect("top drawer rect");
    assert_eq!(top_rect.y, 0);
    assert_eq!(top_rect.width, 100);

    wm.set_drawer_edge(Some(DrawerEdge::Bottom));
    let bottom_rect = wm.drawer_rect(area).expect("bottom drawer rect");
    assert_eq!(bottom_rect.y + bottom_rect.height, 24);
    assert_eq!(bottom_rect.width, 100);

    // AutoHide mode: hidden until leader pending, mouse edge hover, or background pane alert
    wm.close_drawer();
    wm.set_drawer_edge(Some(DrawerEdge::Left));
    wm.set_drawer_visibility(Some(DrawerVisibilityMode::AutoHide));
    wm.drawer_mut().open = false;
    wm.drawer_mut().focused = false;
    assert!(!wm.is_drawer_effectively_visible());

    // Trigger 1: Leader key pending (`Ctrl+X`)
    wm.set_leader_pending(true);
    assert!(wm.is_drawer_effectively_visible());
    wm.set_leader_pending(false);
    assert!(!wm.is_drawer_effectively_visible());

    // Trigger 2: Mouse hover near Left dock edge (col 0)
    let hover_ev = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 0,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, hover_ev, area).unwrap();
    assert!(wm.is_drawer_effectively_visible());

    // Moving away from drawer hides AutoHide drawer
    let leave_ev = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 80,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, leave_ev, area).unwrap();
    assert!(!wm.is_drawer_effectively_visible());

    // Trigger 3: Background pane OSC 9 notification alert
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.focus_and_raise(1);
    wm.pane_mut(p2)
        .unwrap()
        .process_pty_output(b"\x1b]9;Build finished\x07");
    assert!(wm.has_background_pane_alert());
    assert!(wm.is_drawer_effectively_visible());
}

#[test]
fn drawer_overlay_mode_preserves_pty_size_while_push_mode_reflows_workspace_panes() {
    let mut wm = wm_new(24, 100);
    wm.config_mut().drop_shadows = true;
    let area = Rect::new(0, 0, 100, 24);
    wm.resize_all(area).unwrap();
    let initial_vt_size = wm.active_pane().vt().size();
    assert_eq!(initial_vt_size, (23, 100));

    // Open drawer in Overlay mode (Left, width 34): PTY size MUST NOT change (zero SIGWINCH)
    wm.set_drawer_edge(Some(DrawerEdge::Left));
    wm.set_drawer_layout(Some(DrawerLayoutMode::Overlay));
    wm.toggle_drawer();
    wm.resize_all(area).unwrap();
    assert!(wm.is_drawer_effectively_visible());
    assert_eq!(
        wm.active_pane().vt().size(),
        initial_vt_size,
        "Overlay drawer must never resize underlying PTYs"
    );

    // Render workspace in Overlay mode and verify drawer header + drop shadow
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("[D:Left] [M:Float] [P:OnDemand]"));

    // Switch to Push mode: workspace_area subtracts drawer width (34 cols -> 66 cols remaining)
    wm.set_drawer_layout(Some(DrawerLayoutMode::Push));
    wm.resize_all(area).unwrap();
    assert_eq!(
        wm.active_pane().vt().size(),
        (23, 66),
        "Push drawer must subtract drawer width and reflow pane"
    );

    // Dock Top in Push mode (height 6 rows -> 18 rows remaining, 17 canvas rows)
    wm.set_drawer_edge(Some(DrawerEdge::Top));
    wm.resize_all(area).unwrap();
    assert_eq!(wm.active_pane().vt().size(), (17, 100));
}

#[test]
fn drawer_telemetry_renders_hierarchy_spinners_sparklines_osc133_and_all_status_badges() {
    let mut wm = wm_new(26, 110);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();

    // Configure Pane 1: Alternate screen + Read-only lock + Notification
    {
        let pane1 = wm.pane_mut(1).unwrap();
        pane1.process_pty_output(b"\x1b[?1049h\x1b]9;Alert1\x07");
        pane1.set_read_only(true);
    }

    // Configure Pane 2: OSC 133 non-zero exit code (42) + SSH remote target
    {
        let pane2 = wm.pane_mut(p2).unwrap();
        pane2.process_pty_output(b"output stream bytes\r\n\x1b]133;D;42\x07");
        pane2.set_remote_target(Some("prod-db".to_string()));
    }

    // Configure Pane 3: Running command + TMUX title + Broadcast + Workspace recording
    {
        let pane3 = wm.pane_mut(p3).unwrap();
        pane3.set_title("TMUX %1".to_string());
        pane3.submit_command("cargo test").unwrap();
    }
    wm.set_broadcast_input(true);
    let tmp = tempfile::tempdir().unwrap();
    wm.start_workspace_recording(tmp.path().join("demo.cast"), false);
    wm.poll_all_ptys();

    let rows = wm.drawer_rows();
    let formatted: Vec<String> = rows.iter().map(|r| r.formatted_line()).collect();
    let joined = formatted.join("\n");

    // Hierarchy connectors & focus/z-order rank
    assert!(joined.contains("├─"), "missing tree branch connector: {joined}");
    assert!(joined.contains("╰─"), "missing tree leaf connector: {joined}");
    assert!(joined.contains("► [3] Z:1/3"), "missing focused Z-rank badge: {joined}");

    // Braille spinner & throughput sparkline
    assert!(
        BRAILLE_SPINNER_FRAMES.iter().any(|&c| joined.contains(c)),
        "missing braille spinner: {joined}"
    );
    assert!(joined.contains('▁'), "missing throughput sparkline: {joined}");

    // OSC 133 command state badges
    assert!(joined.contains("⚙ RUN"), "missing running command badge: {joined}");
    assert!(joined.contains("✗ 42"), "missing non-zero exit code badge: {joined}");

    // Mode, security & transport badges
    assert!(joined.contains("🖥 ALT"), "missing ALT screen badge: {joined}");
    assert!(joined.contains("🔒 PROT"), "missing PROT lock badge: {joined}");
    assert!(joined.contains("🔔 1"), "missing notification count badge: {joined}");
    assert!(joined.contains("⏺ REC"), "missing recording badge: {joined}");
    assert!(joined.contains("📡 BCAST"), "missing broadcast badge: {joined}");
    assert!(joined.contains("🌐 SSH"), "missing SSH transport badge: {joined}");
    assert!(joined.contains("🔗 TMUX"), "missing TMUX transport badge: {joined}");
}

#[test]
fn drawer_keyboard_and_mouse_navigation_work_across_tui_and_gui_surfaces() {
    let mut wm = wm_new(24, 100);
    let area = Rect::new(0, 0, 100, 24);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.active_pane_id(), p2);

    // Open and focus drawer via `:drawer focus`
    let cmd = parse_colon_command(":drawer focus").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    assert!(wm.drawer().focused);
    assert!(wm.is_drawer_effectively_visible());

    // Press 'k' to select Pane #1 row, then ' ' (Space) to preview/raise Pane #1 while keeping drawer focused
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane_id(), 1);
    assert!(wm.drawer().focused);

    // Press 'd' to cycle edge (Left -> Right), 'm' to toggle Push -> Overlay -> Push, 'p' to pin Fixed
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Right);
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Overlay);
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Push);
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);

    // Press 'z' to toggle zoom on selected pane
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.zoomed_pane(), Some(1));
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.zoomed_pane(), None);

    // Mouse border drag resize on Right-docked drawer's left border
    let dr = wm.drawer_rect(area).unwrap();
    let down_border = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: dr.x,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, down_border, area).unwrap();
    assert!(wm.drawer().resizing_drag);

    let drag_border = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 60,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, drag_border, area).unwrap();
    assert_eq!(wm.drawer_config().width_cols, 40);

    let up_border = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 60,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, up_border, area).unwrap();
    assert!(!wm.drawer().resizing_drag);

    // Verify both WorkspaceFrameSnapshot and GuiWorkspaceRenderer capture the drawer
    let frame_snap = WorkspaceFrameSnapshot::capture(&wm, area);
    assert!(frame_snap.drawer_rect.is_some());

    let mut gui_renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut gui_surface = GuiFrameSurface::new(8, 16);
    gui_renderer.render_workspace(&wm, area, &mut gui_surface);
    let glyph_stream: String = gui_surface.glyphs.iter().map(|g| g.text.as_str()).collect();
    assert!(glyph_stream.contains("SESSIONS"));
    let svg = gui_surface.export_svg("Bilterm Drawer");
    assert!(svg.contains("<svg"));

    // Press 'x' inside drawer to close selected pane (#1), leaving pane #2
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.pane_count(), 1);
    assert_eq!(wm.active_pane_id(), p2);
}

#[test]
fn drawer_stays_in_place_when_scrolling_clicking_or_pressing_enter_to_select_terminal() {
    let mut wm = wm_new(24, 100);
    let area = Rect::new(0, 0, 100, 24);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    assert_eq!(wm.active_pane_id(), p3);

    // Open drawer with F7 (default OnDemand mode)
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    assert!(wm.is_drawer_effectively_visible());
    assert!(wm.drawer().open);
    assert!(wm.drawer().focused);

    // Scroll up with Up arrow to select Pane #2 and press Enter:
    // Drawer MUST remain visible and focused in place!
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane_id(), p2);
    assert!(wm.is_drawer_effectively_visible(), "drawer must not disappear on Enter");
    assert!(wm.drawer().open);
    assert!(wm.drawer().focused);

    // Scroll up again to Pane #1 and press Enter:
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane_id(), 1);
    assert!(wm.is_drawer_effectively_visible());

    // Click row for Pane #3 (dr.y + 2 is row 0 SessionHeader, +3 is LayoutGroup, +4 is Pane 1, +5 is Pane 2, +6 is Pane 3)
    let dr = wm.drawer_rect(area).unwrap();
    let click_p3 = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: dr.x + 4,
        row: dr.y + 6,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut wm, click_p3, area).unwrap();
    assert_eq!(wm.active_pane_id(), p3);
    assert!(wm.is_drawer_effectively_visible(), "drawer must not disappear when clicking a row");

    // Press Tab to move keyboard focus to the active terminal pane while keeping drawer open in place
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap();
    assert!(!wm.drawer().focused);
    assert!(wm.is_drawer_effectively_visible(), "drawer must remain visible in place when typing in pane");
}

#[test]
fn per_window_colors_drawer_ctrl_x_raw_fkeys_and_session_save_rename_work() {
    use bilterm::ui::WorkspaceWidget;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use ratatui::widgets::Widget;

    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_new(24, 100);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    let area = Rect::new(0, 0, 100, 24);

    // 1. Per-window theme & element color overrides (:window cursor-color, :window bg, :theme)
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.active_pane_id(), p2);
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":window cursor-color #ff00ff").unwrap().unwrap(),
    );
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":window bg #112233").unwrap().unwrap(),
    );
    let p2_pal = wm.pane_chrome(p2).resolve_palette(wm.config());
    assert_eq!(p2_pal.cursor_fg, Some(Color::Rgb(0xFF, 0x00, 0xFF)));
    assert_eq!(p2_pal.window_frame_bg, Color::Rgb(0x11, 0x22, 0x33));
    // Pane #1 remains on its own independent palette
    let p1_pal = wm.pane_chrome(1).resolve_palette(wm.config());
    assert_ne!(p1_pal.window_frame_bg, Color::Rgb(0x11, 0x22, 0x33));

    // Render WorkspaceWidget and verify Pane #2 canvas cells receive #112233 background (not Borland Blue)
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let p2_sample = buf.cell((75, 10)).unwrap();
    assert_eq!(p2_sample.bg, Color::Rgb(0x11, 0x22, 0x33));

    // 2. Drawer Ctrl+X leader chords while drawer is focused + maximize/minimize
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    assert!(wm.drawer().open && wm.drawer().focused);
    // Press Ctrl+X d while drawer is focused -> closes drawer
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)).unwrap();
    assert!(wm.is_leader_pending());
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)).unwrap();
    assert!(!wm.drawer().open);

    // Re-open drawer, test maximize [□] (M) and minimize [_] (-)
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    let normal_w = wm.drawer_rect(area).unwrap().width;
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('M'), KeyModifiers::SHIFT)).unwrap();
    assert!(wm.drawer().maximized);
    assert!(wm.drawer_rect(area).unwrap().width > normal_w);
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE)).unwrap();
    assert!(!wm.drawer().maximized);

    // 3. Session save & rename (:save, :rename, and drawer 's'/'r' hotkeys)
    execute_wm_colon_command(&mut wm, parse_colon_command(":save work-alpha").unwrap().unwrap());
    assert_eq!(wm.config().history_session, "work-alpha");
    assert!(tmp.path().join("work-alpha.toml").is_file());

    execute_wm_colon_command(&mut wm, parse_colon_command(":rename work-beta").unwrap().unwrap());
    assert_eq!(wm.config().history_session, "work-beta");
    assert!(tmp.path().join("work-beta.toml").is_file());

    // 4. RAW mode F-keys (F7 toggles drawer in RAW shell mode; Shift+F2 sends raw VT escape sequence)
    wm.close_drawer();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE)).unwrap();
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        bilterm::pty::line_discipline::InputRoutingMode::RawPtyPassthrough
    );
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    assert!(wm.drawer().open);
    wm.close_drawer();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(2), KeyModifiers::SHIFT)).unwrap();
    assert_eq!(wm.active_pane().last_pty_write(), b"\x1bOQ");

    // 5. Snazzy + Glass visual style modes
    execute_wm_colon_command(&mut wm, parse_colon_command(":style both").unwrap().unwrap());
    assert!(wm.config().snazzy_mode && wm.config().glass_mode);
}

#[test]
fn modern_liquid_glass_per_window_vs_global_theme_cursor_and_ssh_work() {
    use bilterm::pane::completion::compute_completions;
    use bilterm::remote::RemoteHostProfile;
    use bilterm::ui::cursor::CaretStyle;
    use bilterm::ui::modals::HELP_LINES;
    use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use ratatui::widgets::Widget;
    use std::path::Path;

    let mut wm = wm_new(24, 100);
    let area = Rect::new(0, 0, 100, 24);
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.active_pane_id(), p2);

    // 1. Per-window :theme by default vs :theme global
    let p1_initial_bg = wm.pane_chrome(1).resolve_palette(wm.config()).window_frame_bg;
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":theme dracula").unwrap().unwrap(),
    );
    let p1_after_p2_theme = wm.pane_chrome(1).resolve_palette(wm.config()).window_frame_bg;
    let p2_dracula_bg = wm.pane_chrome(p2).resolve_palette(wm.config()).window_frame_bg;
    assert_eq!(
        p1_after_p2_theme, p1_initial_bg,
        "changing :theme on pane 2 must NOT change pane 1's theme"
    );
    assert_ne!(p2_dracula_bg, p1_initial_bg);

    // :theme global 3278 updates ALL windows
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":theme global 3278").unwrap().unwrap(),
    );
    let p1_global = wm.pane_chrome(1).resolve_palette(wm.config());
    let p2_global = wm.pane_chrome(p2).resolve_palette(wm.config());
    assert_eq!(p1_global.active_border_fg, p2_global.active_border_fg);

    // 2. Per-window vs global :cursor and :cursor cursor-color
    wm.focus_and_raise(p2);
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":cursor cursor-color crimson").unwrap().unwrap(),
    );
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":cursor bar").unwrap().unwrap(),
    );
    let (p1_cfg, p1_pal) = wm.pane_chrome(1).resolve_config_and_palette(wm.config());
    let (p2_cfg, p2_pal) = wm.pane_chrome(p2).resolve_config_and_palette(wm.config());
    assert_eq!(p2_cfg.cursor_style, CaretStyle::VerticalBar);
    assert_eq!(p2_pal.cursor_fg, Some(Color::Rgb(255, 42, 42)));
    assert_eq!(
        p1_cfg.cursor_style,
        CaretStyle::SolidBlock,
        "pane 1 cursor style must remain untouched when pane 2 cursor changes"
    );
    assert_ne!(
        p1_pal.cursor_fg,
        Some(Color::Rgb(255, 42, 42)),
        "pane 1 cursor color must remain untouched when pane 2 cursor color changes"
    );

    // :cursor global underline cyan updates ALL windows' cursors
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":cursor global underline cursor-color cyan")
            .unwrap()
            .unwrap(),
    );
    let (p1_cfg_g, p1_pal_g) = wm.pane_chrome(1).resolve_config_and_palette(wm.config());
    let (p2_cfg_g, p2_pal_g) = wm.pane_chrome(p2).resolve_config_and_palette(wm.config());
    assert_eq!(p1_cfg_g.cursor_style, CaretStyle::Underscore);
    assert_eq!(p2_cfg_g.cursor_style, CaretStyle::Underscore);
    assert_eq!(p1_pal_g.cursor_fg, Some(Color::Rgb(0, 240, 255)));
    assert_eq!(p2_pal_g.cursor_fg, Some(Color::Rgb(0, 240, 255)));

    // Followed by a single-window override on pane 1 leaves pane 2 at global underline cyan
    wm.focus_and_raise(1);
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":cursor hollow cursor-color amber")
            .unwrap()
            .unwrap(),
    );
    let (p1_cfg_mix, p1_pal_mix) = wm.pane_chrome(1).resolve_config_and_palette(wm.config());
    let (p2_cfg_mix, p2_pal_mix) = wm.pane_chrome(p2).resolve_config_and_palette(wm.config());
    assert_eq!(p1_cfg_mix.cursor_style, CaretStyle::BlinkingHollowBlock);
    assert_eq!(p1_pal_mix.cursor_fg, Some(Color::Rgb(255, 176, 0)));
    assert_eq!(p2_cfg_mix.cursor_style, CaretStyle::Underscore);
    assert_eq!(p2_pal_mix.cursor_fg, Some(Color::Rgb(0, 240, 255)));

    // 3. Modern Minimal (:modern) and Liquid Glass (:liquid) modes
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":theme global turbo-blue").unwrap().unwrap(),
    );
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":modern on").unwrap().unwrap(),
    );
    assert!(wm.config().modern_mode && wm.config().is_modern());
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let snap = buffer_to_snapshot_string(&buf);
    assert!(snap.contains("❯ Command · TOP"), "modern command bar title expected: {snap}");
    assert!(snap.contains("◢ Bilterm"), "modern status pill expected: {snap}");
    assert!(snap.contains('╭'), "modern rounded outline expected: {snap}");

    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":liquid on").unwrap().unwrap(),
    );
    assert!(wm.config().liquid_glass && wm.config().is_modern() && wm.config().is_glass());
    let mut buf_liq = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf_liq);
    let snap_liq = buffer_to_snapshot_string(&buf_liq);
    assert!(snap_liq.contains("💧"), "liquid glass badge expected: {snap_liq}");

    // 4. :ssh parsing, completion, help, and no pre-login PROMPT_COMMAND bootstrap echo
    let prof = RemoteHostProfile::new("devbox", "user@example.com");
    assert!(
        !prof.remote_osc133_bootstrap,
        "remote_osc133_bootstrap must default to false to prevent pre-login PROMPT_COMMAND echo"
    );
    assert!(parse_colon_command(":ssh").unwrap().is_ok());
    assert!(parse_colon_command(":ssh list").unwrap().is_ok());
    assert!(parse_colon_command(":ssh -p 2222 user@example.com").unwrap().is_ok());
    assert!(parse_colon_command(":ssh user@example.com 2222").unwrap().is_ok());

    let (_, _, ssh_cmd_cands) = compute_completions(":ss", 3, Path::new("."));
    assert!(
        ssh_cmd_cands.iter().any(|c| c.replacement == ":ssh "),
        "expected ':ssh ' with trailing space in completion candidates, got {ssh_cmd_cands:?}"
    );
    let (_, _, ssh_arg_cands) = compute_completions(":ssh ", 5, Path::new("."));
    assert!(
        ssh_arg_cands.iter().any(|c| c.display == "list"),
        "expected subcommand candidates for ':ssh ', got {ssh_arg_cands:?}"
    );
    assert!(
        HELP_LINES.iter().take(20).any(|(k, _)| k.contains(":ssh")),
        "expected :ssh in top section of F1 HELP_LINES"
    );
}

#[test]
fn rect_mode_works_in_raw_non_3270_mode_via_keyboard_mouse_and_palette() {
    use bilterm::colon_command::execute_colon_command;
    use bilterm::pane::CopySelectionKind;
    use bilterm::pty::line_discipline::InputRoutingMode;

    let mut wm = wm_new(20, 60);
    let area = Rect::new(0, 0, 60, 20);
    wm.resize_all(area).unwrap();

    wm.active_pane_mut().process_pty_output(
        b"\x1b[H\x1b[2JAAA_111_ZZZ\r\nBBB_222_YYY\r\nCCC_333_XXX\r\n",
    );

    // Switch active pane into RAW mode (non-3270 command prompt) via F12
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE)).unwrap();
    assert_eq!(
        wm.active_pane().effective_routing_mode(),
        InputRoutingMode::RawPtyPassthrough
    );
    assert!(!wm.active_pane().is_prompt_visible());

    // 1. Open Command Palette with Ctrl+P while in RAW mode, type `:rect mode`, and press Enter
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert!(wm.palette_state().is_some(), "Ctrl+P must open Palette in RAW mode");
    for ch in ":rect mode".chars() {
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)).unwrap();
    }
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(wm.active_pane().copy_mode().rect_mode);
    assert!(
        wm.active_pane().copy_mode().active,
        ":rect mode in RAW mode must immediately activate rectangular canvas selection"
    );
    assert_eq!(
        wm.active_pane().copy_mode().selection_kind,
        CopySelectionKind::Block
    );

    // 2. Exit copy mode with Esc (keeping rect_mode = true) and verify Shift+Arrows in RAW mode starts rectangular selection
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    assert!(!wm.active_pane().copy_mode().active);
    assert!(wm.active_pane().copy_mode().rect_mode);

    wm.active_pane_mut().process_pty_output(b"\x1b[1;5H"); // row 0, col 4 ('1')
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT)).unwrap();
    assert!(wm.active_pane().copy_mode().active);
    assert_eq!(
        wm.active_pane().copy_mode().selection_kind,
        CopySelectionKind::Block
    );
    // Press 'x' in RAW mode to yank AND paste the 3x3 rectangular block into the raw PTY
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().prompt().yank_buffer, "111\n222\n333");

    // 3. Mouse drag in RAW mode with rect_mode active selects rectangular block
    wm.active_pane_mut().process_pty_output(
        b"\x1b[H\x1b[2JAAA_111_ZZZ\r\nBBB_222_YYY\r\nCCC_333_XXX\r\n",
    );
    let _ = execute_colon_command(
        wm.active_pane_mut(),
        parse_colon_command(":rect mode on").unwrap().unwrap(),
    );
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    let (_, _, inner) = wm.hit_test_pane(10, 5, area).unwrap();
    let canvas = wm.active_pane().canvas_rect(inner);
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: canvas.x,
            row: canvas.y,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: canvas.x + 2,
            row: canvas.y + 1,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: canvas.x + 2,
            row: canvas.y + 1,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    // Explicit copy by default: mouse drag highlights block, Enter/y explicitly copies it
    assert_eq!(wm.active_pane().prompt().yank_buffer, "111\n222\n333");
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().prompt().yank_buffer, "AAA\nBBB");

    // 4. Alt+R in RAW mode enters rectangular copy mode directly
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT)).unwrap();
    assert!(wm.active_pane().copy_mode().active);
    assert_eq!(
        wm.active_pane().copy_mode().selection_kind,
        CopySelectionKind::Block
    );

    // 5. Liquid Glass ZenTerm/REX multi-row graphical Drawer, MCP Console, and Command Palette
    let mut wm_liq = wm_new(26, 110);
    let area_liq = Rect::new(0, 0, 110, 26);
    let p2 = wm_liq.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm_liq.active_pane_id(), p2);
    execute_wm_colon_command(
        &mut wm_liq,
        parse_colon_command(":liquid on").unwrap().unwrap(),
    );
    execute_wm_colon_command(
        &mut wm_liq,
        parse_colon_command(":drawer open").unwrap().unwrap(),
    );
    let mut buf_liq = Buffer::empty(area_liq);
    WorkspaceWidget::new(&wm_liq).render(area_liq, &mut buf_liq);
    let snap_liq = buffer_to_snapshot_string(&buf_liq);
    assert!(
        snap_liq.contains("SESSIONS") && snap_liq.contains("💧"),
        "expected Liquid Glass SESSIONS drawer header in snapshot: {snap_liq}"
    );
    assert!(
        !snap_liq.contains("HERDR"),
        "drawer header should say SESSIONS, not HERDR: {snap_liq}"
    );
    assert!(
        snap_liq.contains("Active Workspace"),
        "expected 2-row graphical card sub-line in Liquid Glass drawer: {snap_liq}"
    );

    // Mouse click on 2-row card for Pane #1 inside Liquid Glass drawer selects Pane #1
    let dr = wm_liq.drawer_rect(area_liq).unwrap();
    // Header is 2 rows inside border (dr.y + 1, dr.y + 2); item 0 (SessionHeader) is rows +3..+4;
    // item 1 (LayoutGroup) is rows +5..+6; item 2 (Pane #1) is rows +7..+8
    handle_wm_mouse_event(
        &mut wm_liq,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: dr.x + 5,
            row: dr.y + 7,
            modifiers: KeyModifiers::NONE,
        },
        area_liq,
    )
    .unwrap();
    assert_eq!(wm_liq.active_pane_id(), 1);

    // Open MCP Console in Liquid Glass mode and verify multi-row control deck header
    execute_wm_colon_command(
        &mut wm_liq,
        parse_colon_command(":mcp open").unwrap().unwrap(),
    );
    let mut buf_mcp = Buffer::empty(area_liq);
    WorkspaceWidget::new(&wm_liq).render(area_liq, &mut buf_mcp);
    let snap_mcp = buffer_to_snapshot_string(&buf_mcp);
    assert!(
        snap_mcp.contains("Layout:") && snap_mcp.contains("Refresh (r)"),
        "expected spacious multi-row MCP toolbar in Liquid Glass mode: {snap_mcp}"
    );
    assert!(
        snap_liq.contains("● ● ●") && !snap_liq.contains('╔'),
        "expected macOS/ZenTerm traffic-light header and no DOS double-line border in Liquid Glass mode: {snap_liq}"
    );

    // 6. Verify all Liquid Glass & Modern UI glyphs rasterize geometrically in --gui
    for ch in ['╭', '╮', '╰', '╯', '▎', '▍', '▏', '◈', '◌', '×', '⌟', '◧', '⧉', '💧', '🔒', '🔍'] {
        assert!(
            bilterm::ui::gui_font::is_geometric_glyph(ch),
            "expected {ch:?} (U+{:04X}) to be classified as geometric glyph",
            ch as u32
        );
        let mut px = vec![0xFF00_0000u32; 12 * 20];
        bilterm::ui::gui_font::draw_geometric_cell_bgra(
            &mut px,
            12,
            20,
            0,
            0,
            12,
            20,
            ch,
            [110, 231, 255, 255],
        );
        assert!(
            px.iter().any(|&p| p != 0xFF00_0000),
            "expected draw_geometric_cell_bgra to draw visible pixels for {ch:?} (U+{:04X})",
            ch as u32
        );
    }
}

#[test]
fn explicit_copy_clipboard_history_directional_focus_and_single_cursor_in_multi_window_session() {
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_new(24, 120);
    let area = Rect::new(0, 0, 120, 24);
    wm.config_mut().history_dir = Some(tmp.path().to_string_lossy().into_owned());
    wm.config_mut().history_session = "alpha-session".to_string();
    wm.sync_history_config();

    let p1 = wm.active_pane_id();
    wm.active_pane_mut().process_pty_output(b"SECRET_TOKEN_ONE\r\n");

    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.active_pane_mut().process_pty_output(b"NOISE_IN_PANE_TWO\r\n");

    let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
    wm.active_pane_mut().process_pty_output(b"THIRD_WINDOW_TEXT\r\n");
    wm.resize_all(area).unwrap();

    // 1. Focus Pane #1 and select + explicitly copy "SECRET"
    wm.focus_and_raise(p1);
    let p1_outer = wm
        .compute_workspace_rects(area)
        .into_iter()
        .find(|(id, _)| *id == p1)
        .unwrap()
        .1;
    let c1 = wm
        .active_pane()
        .canvas_rect(bilterm::wm::inset_window_frame(p1_outer));
    for (kind, col) in [
        (MouseEventKind::Down(MouseButton::Left), c1.x),
        (MouseEventKind::Drag(MouseButton::Left), c1.x + 5),
        (MouseEventKind::Up(MouseButton::Left), c1.x + 5),
    ] {
        handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind,
                column: col,
                row: c1.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
    }
    assert!(wm.active_pane().copy_mode().has_selection());
    // Phase 62-01 (copy_on_select default true): mouse release auto-copies into the yank ring.
    assert_eq!(wm.active_pane().prompt().yank_buffer, "SECRET");

    // Explicitly copy with 'y'
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().prompt().yank_buffer, "SECRET");
    assert_eq!(wm.active_pane().prompt().yank_history, vec!["SECRET".to_string()]);

    // 2. Errant click and accidental drag inside Pane #2 must NOT overwrite yank_buffer
    let p2_outer = wm
        .compute_workspace_rects(area)
        .into_iter()
        .find(|(id, _)| *id == p2)
        .unwrap()
        .1;
    let c2 = wm
        .pane(p2)
        .unwrap()
        .canvas_rect(bilterm::wm::inset_window_frame(p2_outer));
    // GUARD-03: the first click on inactive Pane #2 only focuses it; the drag that follows a
    // second press is the accidental selection under test.
    for (kind, col) in [
        (MouseEventKind::Down(MouseButton::Left), c2.x),
        (MouseEventKind::Up(MouseButton::Left), c2.x),
        (MouseEventKind::Down(MouseButton::Left), c2.x),
        (MouseEventKind::Drag(MouseButton::Left), c2.x + 4),
        (MouseEventKind::Up(MouseButton::Left), c2.x + 4),
    ] {
        handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind,
                column: col,
                row: c2.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
    }
    assert_eq!(wm.active_pane_id(), p2);
    assert!(wm.active_pane().copy_mode().has_selection());
    // Pane #1's yank buffer and session clipboard history remain intact ("SECRET", not overwritten by "NOISE")
    assert_eq!(wm.pane(p1).unwrap().prompt().yank_buffer, "SECRET");
    // Phase 62-01 auto-copy-on-release: pane #2 records its own "NOISE" selection, appended
    // to (never replacing) the session history and never touching pane #1's buffer.
    assert_eq!(wm.active_pane().prompt().yank_buffer, "NOISE");
    assert_eq!(
        wm.active_pane().history_store().load_recent_clipboard(10),
        vec!["NOISE".to_string(), "SECRET".to_string()]
    );

    // Now explicitly copy "NOISE" in Pane #2 and verify clipboard history ring has both entries
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.active_pane().prompt().yank_buffer, "NOISE");
    let persisted = wm.active_pane().history_store().load_recent_clipboard(10);
    assert_eq!(persisted, vec!["NOISE".to_string(), "SECRET".to_string()]);

    // 3. Per-session uniqueness: switching to a different session name has an isolated clipboard log
    wm.config_mut().history_session = "beta-session".to_string();
    wm.sync_history_config();
    assert!(
        wm.active_pane()
            .history_store()
            .load_recent_clipboard(10)
            .is_empty()
    );
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":clipboard copy BETA_CLIP").unwrap().unwrap(),
    );
    assert_eq!(
        wm.active_pane().history_store().load_recent_clipboard(10),
        vec!["BETA_CLIP".to_string()]
    );

    // Switch back to "alpha-session" and verify its clipboard history is still ["NOISE", "SECRET"]
    wm.config_mut().history_session = "alpha-session".to_string();
    wm.sync_history_config();
    assert_eq!(
        wm.active_pane().history_store().load_recent_clipboard(10),
        vec!["NOISE".to_string(), "SECRET".to_string()]
    );

    // 4. Directional window switching (Arrows, VIM hjkl, WASD)
    // Currently on p2 (top-right); p3 is bottom-right, p1 is left
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":focus down").unwrap().unwrap(),
    );
    assert_eq!(wm.active_pane_id(), p3);

    execute_wm_colon_command(&mut wm, parse_colon_command(":focus k").unwrap().unwrap());
    assert_eq!(wm.active_pane_id(), p2);

    execute_wm_colon_command(&mut wm, parse_colon_command(":focus a").unwrap().unwrap());
    assert_eq!(wm.active_pane_id(), p1);

    // Alt+Right when prompt is empty moves focus right (to p2 or p3)
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Right, KeyModifiers::ALT)).unwrap();
    assert_ne!(wm.active_pane_id(), p1);

    // Alt+Shift+H moves focus back left to p1
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('H'), KeyModifiers::ALT | KeyModifiers::SHIFT),
    )
    .unwrap();
    assert_eq!(wm.active_pane_id(), p1);

    // Alt+Shift+D (WASD right) moves focus right
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::ALT | KeyModifiers::SHIFT),
    )
    .unwrap();
    assert_ne!(wm.active_pane_id(), p1);

    // 5. Single-cursor invariant across 3 windows (both in CommandPrompt and RawPtyPassthrough modes):
    // Clicking between p1, p2, p3 must never leave a second reversed/cursor cell in an inactive window.
    for id in [p1, p2, p3] {
        wm.focus_and_raise(id);
        execute_wm_colon_command(&mut wm, parse_colon_command(":mode raw").unwrap().unwrap());
    }
    // Distinct columns per click: Phase 62-02 multi-click (same cell within 400ms) is a
    // double-click word selection, which is not what this single-click invariant exercises.
    for (click_idx, click_target) in [p1, p2, p3, p1].into_iter().enumerate() {
        let target_outer = wm
            .compute_workspace_rects(area)
            .into_iter()
            .find(|(id, _)| *id == click_target)
            .unwrap()
            .1;
        let tc = wm
            .pane(click_target)
            .unwrap()
            .canvas_rect(bilterm::wm::inset_window_frame(target_outer));
        handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: tc.x + 2 + click_idx as u16,
                row: tc.y + 1,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        handle_wm_mouse_event(
            &mut wm,
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: tc.x + 2 + click_idx as u16,
                row: tc.y + 1,
                modifiers: KeyModifiers::NONE,
            },
            area,
        )
        .unwrap();
        assert_eq!(wm.active_pane_id(), click_target);
        // Plain click on canvas must NOT leave copy_mode.active turned on
        for id in [p1, p2, p3] {
            assert!(
                !wm.pane(id).unwrap().copy_mode().active,
                "plain click #{click_idx} (target {click_target}) should not activate copy_mode cursor on pane #{id}"
            );
        }
    }
}

#[test]
fn da2_query_drawer_new_win_sess_and_visual_parent_moves_work() {
    use bilterm::vt::VtParser;
    use bilterm::wm::drawer::DrawerRowKind;

    // 1. Secondary Device Attributes (DA2: CSI > c / CSI > 0 c) & XTVERSION (CSI > q) for `agy` CLI
    let mut vt = VtParser::new(24, 80, 1000);
    vt.process(b"\x1b[>c");
    assert_eq!(vt.take_query_responses(), vec![b"\x1b[>62;260;0c".to_vec()]);
    vt.process(b"\x1b[>0c");
    assert_eq!(vt.take_query_responses(), vec![b"\x1b[>62;260;0c".to_vec()]);
    vt.process(b"\x1b[>q");
    let xtver = vt.take_query_responses();
    assert_eq!(xtver.len(), 1);
    assert!(String::from_utf8_lossy(&xtver[0]).starts_with("\x1bP>|bilterm("));

    // 2. Slide-out drawer defaults to Push and supports new windows & sessions via keyboard & mouse
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = wm_new(24, 100);
    wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
    let area = Rect::new(0, 0, 100, 24);
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Push);

    // Open drawer via control sequence `Ctrl+X d`
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)).unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)).unwrap();
    assert!(wm.drawer().open && wm.drawer().focused);

    // Create a new window with 'c' while drawer is focused
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.pane_count(), 2);
    assert!(wm.drawer().focused);

    // Create another window by clicking `[+Win]` in the drawer footer (`dr.y + dr.height - 2`, `dr.x + 2`)
    let dr = wm.drawer_rect(area).unwrap();
    let footer_row = dr.y + dr.height - 2;
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: dr.x + 2,
            row: footer_row,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert_eq!(wm.pane_count(), 3);
    assert_eq!(wm.ordered_pane_ids(), vec![1, 2, 3]);

    // Visually move Pane #3 up above Pane #2 using 'K' in the drawer
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT)).unwrap();
    assert_eq!(wm.ordered_pane_ids(), vec![1, 3, 2]);
    // Move Pane #3 back down below Pane #2 using 'J'
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT)).unwrap();
    assert_eq!(wm.ordered_pane_ids(), vec![1, 2, 3]);

    // Create a new session by clicking `[+Sess]` in the drawer footer (`dr.x + 8`)
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: dr.x + 8,
            row: footer_row,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert_eq!(wm.config().history_session, "session-2");

    // Select the saved "default" session row in the drawer and reparent it under "session-2" with '>'
    let def_pos = wm
        .drawer_rows()
        .iter()
        .position(|r| r.kind == DrawerRowKind::SessionHeader("default".to_string()))
        .expect("default session row in drawer");
    wm.drawer_mut().selected_index = def_pos;
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('>'), KeyModifiers::SHIFT)).unwrap();
    assert_eq!(
        wm.session_tree().node("default").and_then(|n| n.parent.as_deref()),
        Some("session-2")
    );

    // Promote "default" back to a root session with '<'
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('<'), KeyModifiers::SHIFT)).unwrap();
    assert_eq!(
        wm.session_tree().node("default").and_then(|n| n.parent.as_deref()),
        None
    );
}
