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

#[test]
fn drawer_supports_all_four_edges_and_visibility_modes_including_autohide_triggers() {
    let mut wm = WindowManager::new_headless(24, 100);
    let area = Rect::new(0, 0, 100, 24);

    // Default: OnDemand, Left, Overlay -> initially hidden
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Left);
    assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Overlay);
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
    let mut wm = WindowManager::new_headless(24, 100);
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
    let mut wm = WindowManager::new_headless(26, 110);
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
    let mut wm = WindowManager::new_headless(24, 100);
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

    // Press 'd' to cycle edge (Left -> Right), 'm' to toggle Overlay -> Push, 'p' to pin Fixed
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.drawer_config().edge, DrawerEdge::Right);
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
    assert!(glyph_stream.contains("HERDR"));
    let svg = gui_surface.export_svg("Bilterm Herdr Drawer");
    assert!(svg.contains("<svg"));

    // Press 'x' inside drawer to close selected pane (#1), leaving pane #2
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)).unwrap();
    assert_eq!(wm.pane_count(), 1);
    assert_eq!(wm.active_pane_id(), p2);
}
