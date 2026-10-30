//! Integration tests for Phase 29 (`SCRL-01`, `SCRL-02`):
//! Smooth Scrolling by Default & Mainframe/Timeshare Block/Page-Mode Scroller.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::config::BiltermConfig;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::pane::{CompositePane, PaneScrollMode};
use bilterm::ui::CompositePaneWidget;
use bilterm::ui::renderer::{
    GuiFrameSurface, GuiWorkspaceRenderer, WorkspaceFrameRenderer, WorkspaceFrameSnapshot,
};
use bilterm::wm::WindowManager;

fn render_pane_text(pane: &CompositePane, area: Rect) -> String {
    let mut buf = Buffer::empty(area);
    let cfg = BiltermConfig::default();
    CompositePaneWidget::with_config(pane, true, &cfg).render(area, &mut buf);
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn default_scroll_mode_is_smooth_with_kinetic_damping_and_gui_subcell_offset() {
    let cfg = BiltermConfig::default();
    assert_eq!(cfg.scroll_mode, PaneScrollMode::Smooth);

    let mut wm = WindowManager::new_headless(12, 80);
    assert_eq!(wm.active_pane().scroll_mode(), PaneScrollMode::Smooth);

    // Populate 60 lines of output so scrollback has plenty of history
    {
        let pane = wm.active_pane_mut();
        for idx in 1..=60 {
            pane.process_pty_output(format!("line-{idx:03}\r\n").as_bytes());
        }
        assert_eq!(pane.vt().scrollback_offset(), 0);
        // Trigger smooth upward scroll (kinetic velocity + sub-cell phase)
        pane.scroll_up_in_mode(3);
    }

    assert!(wm.active_pane().vt().scrollback_offset() >= 3);
    assert!(wm.active_pane().scroll_state().is_kinetic_active());

    // Advance one tick so sub-cell phase is non-zero and verify GuiWorkspaceRenderer shifts canvas quads/glyphs
    assert!(wm.poll_all_ptys());
    let subcell_px = wm.active_pane().scroll_state().subcell_offset_px(16);
    assert!(
        subcell_px.abs() > 0.01,
        "expected non-zero sub-cell pixel offset during kinetic smooth scroll, got {subcell_px}"
    );

    let viewport = Rect::new(0, 0, 80, 12);
    let snap = WorkspaceFrameSnapshot::capture(&wm, viewport);
    assert!(
        snap.panes[0].subcell_scroll_offset_px.abs() > 0.01,
        "WorkspaceFrameSnapshot should expose subcell_scroll_offset_px"
    );

    let mut gui = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    gui.render_workspace(&wm, viewport, &mut surface);
    assert!(
        surface
            .glyphs
            .iter()
            .any(|g| (g.y_px.fract().abs() > 0.01) || ((g.y_px % 16.0).abs() > 0.01)),
        "expected GUI glyph runs on OutputCanvas to have sub-cell interpolated y_px coordinates"
    );

    // Let kinetic velocity damp to zero over subsequent frame ticks
    let offset_before_ticks = wm.active_pane().vt().scrollback_offset();
    for _ in 0..80 {
        wm.poll_all_ptys();
    }
    assert!(
        wm.active_pane().vt().scrollback_offset() >= offset_before_ticks,
        "kinetic inertia should carry scrollback at least as far as initial impulse"
    );
    assert!(
        !wm.active_pane().scroll_state().is_kinetic_active(),
        "kinetic scroll velocity should damp cleanly to rest"
    );
}

#[test]
fn toggle_block_page_mode_via_colon_command_leader_key_and_oia_badge_click() {
    let mut wm = WindowManager::new_headless(12, 80);
    let viewport = Rect::new(0, 0, 80, 12);
    wm.resize_all(viewport).unwrap();

    // 1. Toggle via :scroll page and :scroll smooth
    let cmd = parse_colon_command(":scroll page").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    assert_eq!(wm.active_pane().scroll_mode(), PaneScrollMode::BlockPage);

    let rendered = render_pane_text(wm.active_pane(), viewport);
    assert!(
        rendered.contains("SCRL:PAGE"),
        "expected SCRL:PAGE badge in OIA status bar:\n{rendered}"
    );

    let cmd = parse_colon_command(":scroll smooth").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    assert_eq!(wm.active_pane().scroll_mode(), PaneScrollMode::Smooth);
    let rendered_smooth = render_pane_text(wm.active_pane(), viewport);
    assert!(
        rendered_smooth.contains("SCRL:SMOOTH"),
        "expected SCRL:SMOOTH badge in OIA status bar after explicit :scroll:\n{rendered_smooth}"
    );

    // 2. Toggle via Ctrl+X PageDown leader chord
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
    )
    .unwrap();
    assert_eq!(wm.active_pane().scroll_mode(), PaneScrollMode::BlockPage);

    // 3. Toggle by clicking the right-hand OIA status bar region
    let status_row = viewport.height - 1;
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: viewport.width - 4,
            row: status_row,
            modifiers: KeyModifiers::NONE,
        },
        viewport,
    )
    .unwrap();
    assert_eq!(
        wm.active_pane().scroll_mode(),
        PaneScrollMode::Smooth,
        "clicking right side of OIA bar should toggle BlockPage <-> Smooth"
    );
}

#[test]
fn mainframe_block_page_mode_holds_after_full_page_with_more_and_advances_cleanly() {
    let mut wm = WindowManager::new_headless(11, 80);
    let viewport = Rect::new(0, 0, 80, 11);
    wm.resize_all(viewport).unwrap();

    // Enable BlockPage mode (canvas height is 10 rows because 1 row is OIA status bar)
    let cmd = parse_colon_command(":scroll block").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    assert_eq!(wm.active_pane().scroll_mode(), PaneScrollMode::BlockPage);

    // Reset page anchor (as if submitting a command) and stream 35 lines of output (3.5 pages)
    {
        let pane = wm.active_pane_mut();
        pane.reset_page_anchor();
        let mut payload = String::new();
        for i in 1..=35 {
            payload.push_str(&format!("MAINFRAME-ROW-{i:02}\r\n"));
        }
        pane.process_pty_output(payload.as_bytes());
    }

    // Verify the viewport froze on Page 1 with MORE... in the OIA status bar,
    // while all 35 lines were buffered without loss.
    assert!(
        wm.active_pane().is_page_holding(),
        "pane should enter page-hold once 1 full viewport page of output has arrived"
    );
    assert!(
        wm.active_pane().pending_page_lines() >= 20,
        "expected >= 20 pending buffered lines below held page, got {}",
        wm.active_pane().pending_page_lines()
    );

    let page1 = render_pane_text(wm.active_pane(), viewport);
    assert!(
        page1.contains("MORE..."),
        "expected IBM 3270 MORE... status in OIA bar:\n{page1}"
    );
    assert!(
        page1.contains("SCRL:PAGE"),
        "expected SCRL:PAGE badge in OIA bar:\n{page1}"
    );
    assert!(
        page1.contains("MAINFRAME-ROW-04"),
        "expected first page to stay frozen at MAINFRAME-ROW-04 (visible under top prompt):\n{page1}"
    );
    assert!(
        !page1.contains("MAINFRAME-ROW-35"),
        "latest output must not overwrite page 1 while MORE... hold is active:\n{page1}"
    );

    // Stream 5 more lines while held and confirm Page 1 remains locked on screen and no bytes are lost
    wm.active_pane_mut()
        .process_pty_output(b"MAINFRAME-ROW-36\r\nMAINFRAME-ROW-37\r\n");
    let page1_after = render_pane_text(wm.active_pane(), viewport);
    assert!(
        page1_after.contains("MAINFRAME-ROW-04"),
        "Page 1 must remain locked while additional PTY output streams in:\n{page1_after}"
    );

    // Advance 1 full page via Space
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(wm.active_pane().is_page_holding());
    let page2 = render_pane_text(wm.active_pane(), viewport);
    assert!(
        page2.contains("MAINFRAME-ROW-14"),
        "Space should advance to Page 2 (MAINFRAME-ROW-14 visible under prompt):\n{page2}"
    );

    // Step backward 1 full page via PF7 (F7) and verify HOLDING indicator appears
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)).unwrap();
    assert!(wm.active_pane().is_page_holding());
    assert!(wm.active_pane().is_holding_inspected());
    let page1_revisited = render_pane_text(wm.active_pane(), viewport);
    assert!(
        page1_revisited.contains("HOLDING"),
        "stepping back a page in BlockPage mode should display HOLDING in OIA:\n{page1_revisited}"
    );
    assert!(
        page1_revisited.contains("MAINFRAME-ROW-04"),
        "PF7 should step back to Page 1 (MAINFRAME-ROW-04 visible under prompt):\n{page1_revisited}"
    );

    // Advance via PF8 (F8), PageDown, PA2 (Alt+PageDown), and Enter until caught up to live tail
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE)).unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
    )
    .unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::ALT),
    )
    .unwrap();
    handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();

    assert!(
        !wm.active_pane().is_page_holding(),
        "pane should release page-hold once caught up to live tail"
    );
    assert_eq!(wm.active_pane().vt().scrollback_offset(), 0);
    let tail_view = render_pane_text(wm.active_pane(), viewport);
    assert!(
        tail_view.contains("MAINFRAME-ROW-37"),
        "live tail should show the final buffered line MAINFRAME-ROW-37:\n{tail_view}"
    );
}

#[test]
fn block_page_mode_underlying_vt_row0_and_pa1_or_c_cancel_to_live_tail() {
    let mut wm = WindowManager::new_headless(11, 80);
    let viewport = Rect::new(0, 0, 80, 11);
    wm.resize_all(viewport).unwrap();

    let cmd = parse_colon_command(":scroll page").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);

    {
        let pane = wm.active_pane_mut();
        pane.reset_page_anchor();
        for i in 1..=28 {
            pane.process_pty_output(format!("BLOCK-LINE-{i:02}\r\n").as_bytes());
        }
    }

    // Even when the 3-row top floating prompt occludes canvas rows 0..2 in the composite widget,
    // the underlying VT viewport row 0 is anchored at BLOCK-LINE-01.
    assert!(wm.active_pane().is_page_holding());
    assert!(
        wm.active_pane()
            .vt()
            .viewport_line_text(0)
            .contains("BLOCK-LINE-01"),
        "expected VT viewport row 0 to hold BLOCK-LINE-01, got {:?}",
        wm.active_pane().vt().viewport_line_text(0)
    );

    // Advance 1 page via :more colon command
    let more_cmd = parse_colon_command(":more").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, more_cmd);
    assert!(wm.active_pane().is_page_holding());
    assert!(
        wm.active_pane()
            .vt()
            .viewport_line_text(0)
            .contains("BLOCK-LINE-11"),
        "expected :more to advance VT viewport row 0 to BLOCK-LINE-11, got {:?}",
        wm.active_pane().vt().viewport_line_text(0)
    );

    // Cancel page-hold immediately via 'c' (clear hold -> jump to live tail)
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(!wm.active_pane().is_page_holding());
    assert_eq!(wm.active_pane().vt().scrollback_offset(), 0);
}

#[test]
fn vt_worker_handle_tracks_block_page_hold_and_advances_across_thread_seam() {
    use bilterm::pty::PtyConfig;
    use bilterm::vt::worker::VtWorkerHandle;
    use std::time::{Duration, Instant};

    // 30 output lines, then stay alive briefly so the worker sees them all.
    let (program, script) = if cfg!(windows) {
        (
            "cmd.exe",
            "(for /L %i in (1,1,30) do @echo WORKER-LINE-%i) & ping -n 2 127.0.0.1 >nul",
        )
    } else {
        (
            "/bin/sh",
            "i=1; while [ $i -le 30 ]; do printf 'WORKER-LINE-%02d\\r\\n' $i; i=$((i+1)); done; sleep 0.2",
        )
    };
    let flag = if cfg!(windows) { "/c" } else { "-c" };
    let handle = VtWorkerHandle::spawn_pty(
        PtyConfig {
            program: program.to_string(),
            args: vec![flag.to_string(), script.to_string()],
            rows: 10,
            cols: 80,
            ..Default::default()
        },
        500,
        || {},
    )
    .expect("spawn shell worker");

    handle.set_scroll_mode(PaneScrollMode::BlockPage);

    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if handle.is_page_holding() && handle.pending_page_lines() >= 10 {
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }

    assert!(
        handle.is_page_holding(),
        "VtWorkerHandle should enter page-hold when child shell outputs >10 rows in BlockPage mode"
    );
    assert!(handle.pending_page_lines() > 0);

    // Step back/forward across the worker channel
    handle.page_back();
    let deadline2 = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline2 && !handle.is_holding_inspected() {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(handle.is_holding_inspected());

    // Advance until live tail
    for _ in 0..5 {
        handle.page_advance();
    }
    let deadline3 = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline3 && handle.is_page_holding() {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !handle.is_page_holding(),
        "VtWorkerHandle should release page-hold after advancing past all pending pages"
    );
}
