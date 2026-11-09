//! Phase 61-03 (`FIX-03`): Per-window prompt mode/anchor, raw passthrough, Vi copy mode,
//! prompt text, and scrollback offset preservation across focus switches, layout changes,
//! and session/layout-profile snapshots.

use bilterm::input::handle_wm_key_event;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::pane::{PromptAnchor, PromptMode, PromptMouseDrag};
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{SavedWorkspaceSession, WindowManager, WorkspaceLayoutMode, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const COLS: u16 = 80;
const ROWS: u16 = 24;
const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: COLS,
    height: ROWS,
};

fn isolate_xdg() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!(
            "bilterm-phase61-state-preserved-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: runs once before any WindowManager is constructed in this test binary.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::set_var("XDG_STATE_HOME", &dir);
        }
    });
}

fn new_two_pane_wm() -> WindowManager {
    isolate_xdg();
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(AREA).unwrap();
    wm
}

fn snap(wm: &WindowManager) -> String {
    let mut parts = Vec::new();
    for id in [1usize, 2usize] {
        if let Some(p) = wm.pane(id) {
            parts.push(format!(
                "#{id}[mode={:?} anchor={:?} copy={} text={:?} sb={}]",
                p.prompt_mode(),
                p.prompt().anchor,
                p.copy_mode().active,
                p.prompt().text(),
                p.vt().scrollback_offset()
            ));
        }
    }
    format!("active={} {}", wm.active_pane_id(), parts.join(" "))
}

fn left_down(wm: &mut WindowManager, col: u16, row: u16) {
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(wm, ev, AREA).unwrap();
}

fn left_drag(wm: &mut WindowManager, col: u16, row: u16) {
    let ev = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(wm, ev, AREA).unwrap();
}

fn pane_inner_rect(wm: &WindowManager, pane_id: usize) -> Rect {
    let rects = wm.compute_workspace_rects(AREA);
    let &(_, outer) = rects.iter().find(|&&(id, _)| id == pane_id).unwrap();
    if wm.is_framed() {
        inset_window_frame(outer)
    } else {
        outer
    }
}

#[test]
fn click_on_inactive_oia_routing_cell_only_focuses() {
    let mut wm = new_two_pane_wm();
    assert_eq!(wm.active_pane_id(), 2, "{}", snap(&wm));
    assert_eq!(wm.pane(1).unwrap().prompt_mode(), PromptMode::Floating);
    assert_eq!(wm.pane(1).unwrap().prompt().anchor, PromptAnchor::Top);

    // Click inactive pane #1 OIA routing cell at (21, 22): focuses only, keeps Floating/Top.
    left_down(&mut wm, 21, 22);
    assert_eq!(wm.active_pane_id(), 1, "{}", snap(&wm));
    assert_eq!(
        wm.pane(1).unwrap().prompt_mode(),
        PromptMode::Floating,
        "focusing click on inactive OIA routing cell must not cycle prompt mode: {}",
        snap(&wm)
    );
    assert_eq!(wm.pane(1).unwrap().prompt().anchor, PromptAnchor::Top);

    // Second click on now-active pane #1 OIA routing cell cycles Floating -> Fixed.
    left_down(&mut wm, 21, 22);
    assert_eq!(wm.pane(1).unwrap().prompt_mode(), PromptMode::Fixed);
    assert_eq!(wm.pane(1).unwrap().prompt().anchor, PromptAnchor::SplitTop);

    // Reset pane 1 to Floating/Top, focus pane 2, then test OIA pin cell on inactive pane 1.
    wm.pane_mut(1).unwrap().set_prompt_mode(PromptMode::Floating);
    wm.pane_mut(1).unwrap().set_prompt_anchor(PromptAnchor::Top);
    wm.focus_and_raise(2);
    assert_eq!(wm.active_pane_id(), 2);

    let inner1 = pane_inner_rect(&wm, 1);
    let status_row = inner1.y + inner1.height - 1;
    // Pin cell sits after routing_end (rel_col ~36 when version badge is off, ~43 when on); disable version badge for deterministic offset.
    wm.pane_mut(1).unwrap().set_show_version_badge(false);
    let pin_col = inner1.x + 37;
    left_down(&mut wm, pin_col, status_row);
    assert_eq!(wm.active_pane_id(), 1);
    assert_eq!(
        wm.pane(1).unwrap().prompt().anchor,
        PromptAnchor::Top,
        "focusing click on inactive OIA pin cell must not cycle anchor"
    );
    // Second click on active pane #1 OIA pin cell cycles Top -> Bottom.
    left_down(&mut wm, pin_col, status_row);
    assert_eq!(wm.pane(1).unwrap().prompt().anchor, PromptAnchor::Bottom);

    // Now test prompt top-border [▲]/[▼] badge on inactive pane 1.
    wm.focus_and_raise(2);
    assert_eq!(wm.active_pane_id(), 2);
    let inner1 = pane_inner_rect(&wm, 1);
    let pr1 = wm.pane(1).unwrap().prompt_rect(inner1);
    let top_btn_col = (pr1.x + pr1.width).saturating_sub(8);
    left_down(&mut wm, top_btn_col, pr1.y);
    assert_eq!(wm.active_pane_id(), 1);
    assert_eq!(
        wm.pane(1).unwrap().prompt().anchor,
        PromptAnchor::Bottom,
        "focusing click on inactive prompt top-border badge must not change anchor"
    );
    assert!(
        wm.pane(1).unwrap().mouse_drag().is_none(),
        "focusing click on inactive prompt top border must not start MovePrompt drag"
    );
    // Second click on now-active pane #1 [▲] badge pins to Top.
    left_down(&mut wm, top_btn_col, pr1.y);
    assert_eq!(wm.pane(1).unwrap().prompt().anchor, PromptAnchor::Top);
}

#[test]
fn click_text_area_still_focuses_and_passes_through() {
    let mut wm = new_two_pane_wm();
    assert_eq!(wm.active_pane_id(), 2);

    let inner1 = pane_inner_rect(&wm, 1);
    let canvas_row = inner1.y + inner1.height / 2 + 2;
    let canvas_col = inner1.x + 4;

    left_down(&mut wm, canvas_col, canvas_row);
    assert_eq!(wm.active_pane_id(), 1);
    assert_eq!(
        wm.pane(1).unwrap().mouse_drag(),
        Some(PromptMouseDrag::CanvasCharSelect)
    );

    left_drag(&mut wm, canvas_col + 5, canvas_row);
    assert!(wm.pane(1).unwrap().copy_mode().active);
    assert!(wm.pane(1).unwrap().copy_mode().has_selection());
}

#[test]
fn copy_mode_survives_focus_switches() {
    for mode in [WorkspaceLayoutMode::Tiled, WorkspaceLayoutMode::Stacked] {
        let mut wm = new_two_pane_wm();
        if wm.layout_mode() != mode {
            wm.toggle_layout_mode();
        }
        assert_eq!(wm.active_pane_id(), 2);
        wm.pane_mut(2).unwrap().enter_copy_mode();
        assert!(wm.pane(2).unwrap().copy_mode().active);

        // Alt+Tab via handle_wm_key_event -> pane 1 becomes active, pane 2 stays in copy mode.
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::ALT),
        )
        .unwrap();
        assert_eq!(wm.active_pane_id(), 1);
        assert!(
            wm.pane(2).unwrap().copy_mode().active,
            "copy mode must survive Alt+Tab in {mode:?}: {}",
            snap(&wm)
        );

        // Typing 'x' while pane 1 is active goes into pane 1's prompt, not pane 2's copy handler.
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        )
        .unwrap();
        assert_eq!(wm.pane(1).unwrap().prompt().text(), "x");
        assert!(wm.pane(2).unwrap().copy_mode().active);

        // Alt+Tab back to pane 2.
        handle_wm_key_event(
            &mut wm,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::ALT),
        )
        .unwrap();
        assert_eq!(wm.active_pane_id(), 2);
        assert!(wm.pane(2).unwrap().copy_mode().active);

        // focus_and_raise(1) and focus_direction
        wm.focus_and_raise(1);
        assert!(wm.pane(2).unwrap().copy_mode().active);
        wm.focus_direction(1, 0);
        assert!(wm.pane(2).unwrap().copy_mode().active);

        // Mouse click focus on pane 1 (in Tiled mode where pane 1 is visible alongside pane 2)
        if mode == WorkspaceLayoutMode::Tiled {
            wm.focus_and_raise(2);
            let inner1 = pane_inner_rect(&wm, 1);
            left_down(&mut wm, inner1.x + 2, inner1.y + inner1.height - 1);
            assert_eq!(wm.active_pane_id(), 1);
            assert!(wm.pane(2).unwrap().copy_mode().active);
        }
    }
}

#[test]
fn prompt_modes_and_text_survive_switches() {
    let mut wm = new_two_pane_wm();
    wm.pane_mut(1).unwrap().set_prompt_mode(PromptMode::Fixed);
    wm.pane_mut(1).unwrap().prompt_mut().set_text("abc");
    wm.pane_mut(2)
        .unwrap()
        .set_prompt_mode(PromptMode::RegularTerminal);

    // Alt+Tab, F6, toggle quake and drawer, toggle_layout_mode twice
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Tab, KeyModifiers::ALT),
    )
    .unwrap();
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE),
    )
    .unwrap();
    wm.toggle_quick_terminal();
    wm.toggle_quick_terminal();
    wm.toggle_drawer();
    wm.toggle_drawer();
    wm.toggle_layout_mode();
    wm.toggle_layout_mode();

    assert_eq!(wm.pane(1).unwrap().prompt_mode(), PromptMode::Fixed);
    assert_eq!(wm.pane(1).unwrap().prompt().anchor, PromptAnchor::SplitTop);
    assert_eq!(wm.pane(1).unwrap().prompt().text(), "abc");
    assert_eq!(
        wm.pane(2).unwrap().prompt_mode(),
        PromptMode::RegularTerminal
    );
}

#[test]
fn persist_raw_passthrough_roundtrip() {
    let mut wm = new_two_pane_wm();
    wm.pane_mut(2)
        .unwrap()
        .set_prompt_mode(PromptMode::RegularTerminal);
    assert_eq!(
        wm.pane(2).unwrap().prompt_mode(),
        PromptMode::RegularTerminal
    );

    let snap_session = wm.snapshot_session("t");
    let sp1 = snap_session.panes.iter().find(|p| p.id == 1).unwrap();
    let sp2 = snap_session.panes.iter().find(|p| p.id == 2).unwrap();
    assert!(!sp1.raw_passthrough);
    assert!(sp2.raw_passthrough);

    // Reset pane 2 to Floating, then restore snapshot via apply_saved_session_snapshot.
    wm.pane_mut(2)
        .unwrap()
        .set_prompt_mode(PromptMode::Floating);
    assert_eq!(wm.pane(2).unwrap().prompt_mode(), PromptMode::Floating);

    wm.apply_saved_session_snapshot(&snap_session);
    assert_eq!(wm.pane(1).unwrap().prompt_mode(), PromptMode::Floating);
    assert_eq!(
        wm.pane(2).unwrap().prompt_mode(),
        PromptMode::RegularTerminal
    );

    // Legacy TOML missing `raw_passthrough` deserializes with `raw_passthrough == false`.
    let toml_str = toml::to_string(&snap_session).unwrap();
    let stripped = toml_str.replace("raw_passthrough = true\n", "");
    assert!(!stripped.contains("raw_passthrough"));
    let parsed: SavedWorkspaceSession = toml::from_str(&stripped).unwrap();
    for p in &parsed.panes {
        assert!(!p.raw_passthrough);
    }
}

#[test]
fn reflow_keeps_scroll_offset() {
    let mut wm = new_two_pane_wm();
    // Feed 60 lines of output into pane 2 so it builds scrollback history.
    let mut payload = String::new();
    for i in 0..60 {
        payload.push_str(&format!("line {i:03} abcdefghijklmnopqrstuvwxyz\r\n"));
    }
    wm.pane_mut(2)
        .unwrap()
        .process_pty_output(payload.as_bytes());
    wm.pane_mut(2).unwrap().vt_mut().scroll_up(10);
    assert_eq!(wm.pane(2).unwrap().vt().scrollback_offset(), 10);

    // Toggling Tiled -> Stacked changes pane 2's width (40 -> 80 cols), triggering column reflow.
    wm.toggle_layout_mode();
    let offset = wm.pane(2).unwrap().vt().scrollback_offset();
    let cap = wm.pane(2).unwrap().vt().scrollback_capacity();
    assert!(
        offset > 0 && offset <= cap,
        "expected scrollback_offset > 0 after reflow, got {offset} (cap {cap})"
    );
}
