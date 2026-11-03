//! Coverage for `src/input/modal.rs::handle_wm_modal_key`: slide-out drawer and MCP console
//! (including its file browser).

use std::sync::Once;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use bilterm::input::{AppAction, handle_wm_key_event};
use bilterm::mcp_console::McpRowAction;
use bilterm::ui::file_browser::FileBrowserState;
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::{DrawerLayoutMode, DrawerVisibilityMode, Modal, WindowManager};

static ENV: Once = Once::new();

fn isolate() {
    ENV.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-modal-wm-cov-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // SAFETY: executed exactly once, before any test in this binary reads the environment.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
            std::env::set_var("XDG_STATE_HOME", dir.join("state"));
            std::env::set_var("XDG_DATA_HOME", dir.join("data"));
        }
    });
}

fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn km(code: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}
fn ch(c: char) -> KeyEvent {
    k(KeyCode::Char(c))
}
fn wm() -> WindowManager {
    isolate();
    WindowManager::new_headless(30, 100)
}
fn press(wm: &mut WindowManager, key: KeyEvent) -> AppAction {
    handle_wm_key_event(wm, key).unwrap()
}
fn status(wm: &WindowManager) -> String {
    wm.active_pane().status_message().unwrap_or_default().to_string()
}

/// Window manager with 3 panes (ids 1,2,3) and the drawer focused.
fn drawer_wm() -> WindowManager {
    let mut w = wm();
    w.split_focused(SplitDirection::Horizontal).unwrap();
    w.split_focused(SplitDirection::Vertical).unwrap();
    w.focus_drawer();
    w
}

// ---------------------------------------------------------------------------------------------
// Drawer
// ---------------------------------------------------------------------------------------------

#[test]
fn drawer_q_and_f7_close_the_drawer() {
    for key in [ch('q'), k(KeyCode::F(7))] {
        let mut w = drawer_wm();
        assert!(w.drawer().focused);
        assert_eq!(press(&mut w, key), AppAction::Handled);
        assert!(!w.drawer().focused);
        assert!(!w.drawer().open);
    }
}

#[test]
fn drawer_f11_cycles_visual_style() {
    let mut w = drawer_wm();
    press(&mut w, k(KeyCode::F(11)));
    assert!(status(&w).starts_with("Visual Style:"), "got {:?}", status(&w));
    assert!(w.drawer().focused);
}

#[test]
fn drawer_minimize_and_maximize_keys() {
    for min in [ch('_'), ch('-')] {
        let mut w = drawer_wm();
        press(&mut w, min);
        assert_eq!(status(&w), "Slide-Out Drawer: MINIMIZED [_]");
        assert!(!w.drawer().maximized);
    }
    for max in [ch('M'), k(KeyCode::F(5))] {
        let mut w = drawer_wm();
        press(&mut w, max);
        assert_eq!(status(&w), "Slide-Out Drawer: MAXIMIZED [□]");
        assert!(w.drawer().maximized);
    }
}

#[test]
fn drawer_horizontal_scroll_then_right_at_end_leaves_drawer() {
    let mut w = drawer_wm();
    assert_eq!(w.drawer().h_scroll, 0);
    press(&mut w, k(KeyCode::Left));
    assert_eq!(w.drawer().h_scroll, 0, "Left clamps at 0");
    press(&mut w, k(KeyCode::Right));
    assert_eq!(w.drawer().h_scroll, 6);
    assert!(w.drawer().focused);
    press(&mut w, k(KeyCode::Left));
    assert_eq!(w.drawer().h_scroll, 0);
    press(&mut w, ch('l'));
    assert_eq!(w.drawer().h_scroll, 6);
    press(&mut w, ch('h'));
    assert_eq!(w.drawer().h_scroll, 0);
    // Keep pressing Right until the scroll can no longer advance; the next Right leaves the drawer.
    let mut guard = 0;
    while w.drawer().focused && guard < 50 {
        press(&mut w, k(KeyCode::Right));
        guard += 1;
    }
    assert!(!w.drawer().focused, "Right at end of text unfocuses the drawer");
    assert!(w.drawer().h_scroll > 0);
}

#[test]
fn drawer_esc_and_tab_unfocus_but_keep_it_open() {
    for key in [k(KeyCode::Esc), k(KeyCode::Tab)] {
        let mut w = drawer_wm();
        press(&mut w, key);
        assert!(!w.drawer().focused);
        assert!(w.drawer().open);
    }
}

#[test]
fn drawer_alt_arrows_reparent_and_shift_alt_arrows_reorder_sessions() {
    let mut w = wm();
    w.focus_drawer();
    // Two extra sessions give the drawer three session headers to reparent / reorder.
    press(&mut w, ch('N'));
    w.focus_drawer();
    press(&mut w, ch('N'));
    assert!(w.drawer().focused);
    let headers = || {
        w.drawer_rows()
            .iter()
            .filter(|r| format!("{:?}", r.kind).starts_with("SessionHeader"))
            .count()
    };
    assert_eq!(headers(), 3);

    // Alt+Right indents the selected session under its predecessor; Alt+Left promotes it.
    let target = w.selected_drawer_session_name();
    press(&mut w, km(KeyCode::Right, KeyModifiers::ALT));
    let st = status(&w);
    assert!(st.starts_with("Reparented session '"), "got {st:?}");
    assert!(w.drawer().focused);
    let target = w.selected_drawer_session_name().max(target);
    press(&mut w, km(KeyCode::Left, KeyModifiers::ALT));
    assert_eq!(status(&w), format!("Promoted session '{target}' to root [IND]"));
    press(&mut w, ch('>'));
    assert!(status(&w).starts_with("Reparented session '"));
    press(&mut w, ch('<'));
    assert!(status(&w).starts_with("Promoted session '"));
    press(&mut w, ch('U'));
    assert!(status(&w).starts_with("Promoted session '"));
    press(&mut w, ch('u'));
    assert!(status(&w).starts_with("Promoted session '"));

    // K/J, Shift+Up/Down and Alt+Up/Down reorder sessions; each pair is its own inverse.
    press(&mut w, ch('K'));
    let a = status(&w);
    assert!(a.starts_with("Session order: "), "got {a:?}");
    press(&mut w, ch('J'));
    let b = status(&w);
    assert!(b.starts_with("Session order: ") && a != b);
    press(&mut w, km(KeyCode::Up, KeyModifiers::SHIFT));
    assert_eq!(status(&w), a);
    press(&mut w, km(KeyCode::Down, KeyModifiers::SHIFT));
    assert_eq!(status(&w), b);
    press(&mut w, km(KeyCode::Up, KeyModifiers::ALT));
    assert_eq!(status(&w), a);
    press(&mut w, km(KeyCode::Down, KeyModifiers::ALT));
    assert_eq!(status(&w), b);
    assert!(w.drawer().focused);
}

#[test]
fn drawer_capital_n_creates_session_and_reports_it() {
    let mut w = drawer_wm();
    press(&mut w, ch('N'));
    let st = status(&w);
    assert!(st.starts_with("Created new session '"), "got {st:?}");
    assert_eq!(w.pane_count(), 1, "a fresh session starts with a single pane");
    assert!(w.drawer().focused);
}

#[test]
fn drawer_b_branches_selected_session() {
    let mut w = drawer_wm();
    w.drawer_mut().selected_index = 0; // session header
    let before = w.drawer_rows().len();
    press(&mut w, ch('b'));
    assert!(status(&w).starts_with("Branched child session '"), "got {:?}", status(&w));
    assert!(w.drawer_rows().len() > before, "a child session header row was added");
    let mut w = drawer_wm();
    w.drawer_mut().selected_index = 0;
    press(&mut w, ch('B'));
    assert!(status(&w).starts_with("Branched child session '"));
    assert!(w.drawer().focused);
}

#[test]
fn drawer_colon_prefills_prompt_for_graft_rename_and_command() {
    let mut w = drawer_wm();
    w.drawer_mut().selected_index = 0;
    let name = w.selected_drawer_session_name();
    press(&mut w, ch('P'));
    assert!(!w.drawer().focused);
    assert_eq!(w.active_pane().prompt().text(), format!(":session graft {name} "));
    assert!(w.active_pane().prompt().force_command_prompt);
    assert!(!w.active_pane().prompt().manual_raw_override);

    let mut w = drawer_wm();
    w.drawer_mut().selected_index = 0;
    press(&mut w, ch('r'));
    assert!(!w.drawer().focused);
    assert_eq!(w.active_pane().prompt().text(), format!(":session rename {name} "));

    let mut w = drawer_wm();
    w.active_pane_mut().prompt_mut().manual_raw_override = true;
    press(&mut w, ch(':'));
    assert!(!w.drawer().focused);
    assert_eq!(w.active_pane().prompt().text(), ":");
    assert!(!w.active_pane().prompt().manual_raw_override);
    assert!(w.active_pane().prompt().force_command_prompt);
}

#[test]
fn drawer_s_saves_selected_session() {
    let mut w = drawer_wm();
    w.drawer_mut().selected_index = 0;
    press(&mut w, ch('s'));
    let st = status(&w);
    assert!(st.starts_with("Saved session '"), "got {st:?}");
    assert!(st.contains(".toml"));
    let mut w = drawer_wm();
    w.drawer_mut().selected_index = 0;
    press(&mut w, ch('S'));
    assert!(status(&w).starts_with("Saved session '"));
}

#[test]
fn drawer_selection_keys_move_wrap_and_clamp() {
    let mut w = drawer_wm();
    let len = w.drawer_rows().len();
    assert!(len >= 5);
    w.drawer_mut().selected_index = 0;
    press(&mut w, k(KeyCode::Up));
    assert_eq!(w.drawer().selected_index, len - 1, "Up wraps from first to last");
    press(&mut w, k(KeyCode::Down));
    assert_eq!(w.drawer().selected_index, 0, "Down wraps to first");
    press(&mut w, ch('j'));
    assert_eq!(w.drawer().selected_index, 1);
    press(&mut w, ch('k'));
    assert_eq!(w.drawer().selected_index, 0);

    press(&mut w, k(KeyCode::PageDown));
    assert_eq!(w.drawer().selected_index, 5.min(len - 1));
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(w.drawer().selected_index, 0);
    press(&mut w, k(KeyCode::End));
    assert_eq!(w.drawer().selected_index, len - 1);
    press(&mut w, k(KeyCode::PageDown));
    assert_eq!(w.drawer().selected_index, len - 1, "PageDown clamps at last row");
    press(&mut w, k(KeyCode::Home));
    assert_eq!(w.drawer().selected_index, 0);
    press(&mut w, ch('G'));
    assert_eq!(w.drawer().selected_index, len - 1);
    press(&mut w, ch('g'));
    assert_eq!(w.drawer().selected_index, 0);
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(w.drawer().selected_index, 0, "PageUp clamps at first row");
}

#[test]
fn drawer_enter_and_space_focus_selected_pane() {
    for key in [k(KeyCode::Enter), ch(' ')] {
        let mut w = drawer_wm();
        let rows = w.drawer_rows();
        let idx = rows
            .iter()
            .position(|r| format!("{:?}", r.kind) == "Pane(1)")
            .expect("pane 1 row");
        w.drawer_mut().selected_index = idx;
        assert_ne!(w.active_pane_id(), 1);
        press(&mut w, key);
        assert_eq!(w.active_pane_id(), 1);
        assert!(status(&w).starts_with("Selected Pane #1"));
    }
}

#[test]
fn drawer_split_keys_create_windows_and_report_ids() {
    for key in [ch('n'), ch('+'), ch('c'), ch('w')] {
        let mut w = drawer_wm();
        let before = w.pane_count();
        press(&mut w, key);
        assert_eq!(w.pane_count(), before + 1, "{key:?}");
        let id = w.active_pane_id();
        assert_eq!(status(&w), format!("Created new window #{id}"));
    }
    for key in [ch('v'), ch('|'), ch('%')] {
        let mut w = drawer_wm();
        let before = w.pane_count();
        press(&mut w, key);
        assert_eq!(w.pane_count(), before + 1, "{key:?}");
        let id = w.active_pane_id();
        assert_eq!(status(&w), format!("Created new vertical window #{id}"));
    }
}

#[test]
fn drawer_x_and_delete_close_selected_pane_and_z_zooms() {
    for key in [ch('x'), k(KeyCode::Delete)] {
        let mut w = drawer_wm();
        let rows = w.drawer_rows();
        let idx = rows
            .iter()
            .position(|r| format!("{:?}", r.kind) == "Pane(2)")
            .unwrap();
        w.drawer_mut().selected_index = idx;
        let before = w.pane_count();
        press(&mut w, key);
        assert_eq!(w.pane_count(), before - 1);
        assert!(w.pane(2).is_none());
    }
    // x on a non-pane row (session header) closes nothing.
    let mut w = drawer_wm();
    w.drawer_mut().selected_index = 0;
    let before = w.pane_count();
    press(&mut w, ch('x'));
    assert_eq!(w.pane_count(), before);

    let mut w = drawer_wm();
    let idx = w
        .drawer_rows()
        .iter()
        .position(|r| format!("{:?}", r.kind) == "Pane(1)")
        .unwrap();
    w.drawer_mut().selected_index = idx;
    press(&mut w, ch('z'));
    assert_eq!(w.active_pane_id(), 1, "z focuses the selected pane before zooming");
}

#[test]
fn drawer_layout_edge_and_visibility_keys() {
    let mut w = drawer_wm();
    let edge0 = w.drawer_config().edge;
    press(&mut w, ch('d'));
    assert_ne!(w.drawer_config().edge, edge0);
    assert_eq!(w.drawer_config().edge, edge0.cycle());

    let mut w = drawer_wm();
    let layout0 = w.drawer_config().layout;
    press(&mut w, ch('m'));
    let layout1 = w.drawer_config().layout;
    assert_ne!(layout0, layout1);
    let expect = if layout1 == DrawerLayoutMode::Push {
        "Push-Out reflow"
    } else {
        "Fly-Out overlay"
    };
    assert_eq!(
        status(&w),
        format!("Slide-Out Drawer Layout Mode: {} ({expect})", layout1.label())
    );

    let mut w = drawer_wm();
    assert_eq!(w.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
    press(&mut w, ch('p'));
    assert_eq!(w.drawer_config().visibility, DrawerVisibilityMode::Fixed);
    assert!(w.drawer().open);
}

#[test]
fn drawer_arrange_keys_set_workspace_layout_mode() {
    use bilterm::wm::WorkspaceLayoutMode;
    let mut w = drawer_wm();
    press(&mut w, ch('a'));
    assert!(status(&w).starts_with("Auto-arranged 3 window(s) as 'grid'"), "{}", status(&w));
    assert_eq!(w.layout_mode(), WorkspaceLayoutMode::Tiled);
    press(&mut w, ch('A'));
    assert!(status(&w).contains("'stack-grid'"), "{}", status(&w));
    assert_eq!(w.layout_mode(), WorkspaceLayoutMode::Stacked);
    // 't' from a non-tiled mode arranges a grid (back to tiled)...
    press(&mut w, ch('t'));
    assert!(status(&w).contains("'grid'"), "{}", status(&w));
    assert_eq!(w.layout_mode(), WorkspaceLayoutMode::Tiled);
    // ...and from tiled it cascades.
    press(&mut w, ch('T'));
    assert!(status(&w).contains("'cascade'"), "{}", status(&w));
    assert_ne!(w.layout_mode(), WorkspaceLayoutMode::Tiled);
}

#[test]
fn drawer_capital_l_saves_window_positions() {
    let mut w = drawer_wm();
    press(&mut w, ch('L'));
    assert!(status(&w).starts_with("Saved mid-flight window positions"), "{}", status(&w));
}

#[test]
fn drawer_unmapped_key_is_swallowed_and_keeps_focus() {
    let mut w = drawer_wm();
    let sel = w.drawer().selected_index;
    let panes = w.pane_count();
    assert_eq!(press(&mut w, ch('~')), AppAction::Handled);
    assert!(w.drawer().focused);
    assert_eq!(w.drawer().selected_index, sel);
    assert_eq!(w.pane_count(), panes);
}

#[test]
fn drawer_leader_key_and_pending_chord_bypass_the_drawer() {
    let mut w = drawer_wm();
    // Ctrl+X is the default leader; the drawer yields so the leader handler arms the chord.
    press(&mut w, km(KeyCode::Char('x'), KeyModifiers::CONTROL));
    assert!(w.is_leader_pending());
    assert!(w.drawer().focused);
    // While the chord is pending, the next key is dispatched as a leader chord, not drawer 'q'.
    press(&mut w, ch('q'));
    assert!(!w.is_leader_pending(), "pending flag is consumed by the chord");
}

// ---------------------------------------------------------------------------------------------
// MCP console
// ---------------------------------------------------------------------------------------------

fn long_rows() -> (Vec<String>, Vec<McpRowAction>) {
    let lines: Vec<String> = (0..30)
        .map(|i| format!("row {i:02} {}", "x".repeat(80)))
        .collect();
    let actions = vec![McpRowAction::None; 30];
    (lines, actions)
}

fn mcp_plain() -> WindowManager {
    let mut w = wm();
    let (lines, actions) = long_rows();
    w.mcp_console_mut()
        .open_with_interactive_view("PLAIN", lines, actions);
    w
}

fn mcp_tree() -> WindowManager {
    let mut w = wm();
    w.split_focused(SplitDirection::Vertical).unwrap();
    let lines = w.build_full_tree_lines();
    let title = "MCP SESSION TREE HIERARCHY".to_string();
    w.mcp_console_mut().open_with_view(title, lines);
    w
}

#[test]
fn mcp_esc_and_q_close_console() {
    for key in [k(KeyCode::Esc), ch('q')] {
        let mut w = mcp_plain();
        assert!(w.mcp_console().active);
        assert_eq!(press(&mut w, key), AppAction::Handled);
        assert!(!w.mcp_console().active);
    }
}

#[test]
fn mcp_horizontal_scroll_h_l_and_arrows() {
    let mut w = mcp_plain();
    press(&mut w, k(KeyCode::Right));
    assert_eq!(w.mcp_console().h_scroll, 8);
    press(&mut w, ch('l'));
    assert_eq!(w.mcp_console().h_scroll, 16);
    press(&mut w, k(KeyCode::Left));
    assert_eq!(w.mcp_console().h_scroll, 8);
    press(&mut w, ch('h'));
    assert_eq!(w.mcp_console().h_scroll, 0);
    press(&mut w, ch('h'));
    assert_eq!(w.mcp_console().h_scroll, 0, "clamped at zero");
}

#[test]
fn mcp_plain_row_navigation_and_paging() {
    let mut w = mcp_plain();
    assert_eq!(w.mcp_console().selected_row, 0);
    press(&mut w, k(KeyCode::Down));
    assert_eq!(w.mcp_console().selected_row, 1);
    press(&mut w, ch('j'));
    assert_eq!(w.mcp_console().selected_row, 2);
    press(&mut w, k(KeyCode::Up));
    press(&mut w, ch('k'));
    assert_eq!(w.mcp_console().selected_row, 0);
    press(&mut w, k(KeyCode::Up));
    assert_eq!(w.mcp_console().selected_row, 0, "Up clamps at the first row");
    press(&mut w, k(KeyCode::PageDown));
    assert_eq!(w.mcp_console().selected_row, 5);
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(w.mcp_console().selected_row, 0);
    press(&mut w, k(KeyCode::End));
    press(&mut w, k(KeyCode::Home));
    assert_eq!(w.mcp_console().selected_row, 0);
    assert_eq!(w.mcp_console().scroll_offset, 0);
}

#[test]
fn mcp_tree_home_end_enter_and_refresh() {
    let mut w = mcp_tree();
    press(&mut w, k(KeyCode::End));
    let last = w.mcp_console().selected_row;
    assert!(last > 0);
    press(&mut w, k(KeyCode::Home));
    let first = w.mcp_console().selected_row;
    assert!(first < last);
    // Enter on a pane row focuses that pane.
    let target = w
        .mcp_console()
        .row_actions
        .iter()
        .position(|a| *a == McpRowAction::FocusPane(1))
        .expect("pane 1 row");
    w.mcp_console_mut().selected_row = target;
    press(&mut w, k(KeyCode::Enter));
    assert_eq!(w.active_pane_id(), 1);
    assert_eq!(status(&w), "MCP: Focused Pane #1");
    // 'r' refreshes the tree view and keeps the console open.
    press(&mut w, ch('r'));
    assert!(w.mcp_console().active);
    press(&mut w, ch('R'));
    assert!(w.mcp_console().active);
}

#[test]
fn mcp_layout_and_dock_edge_keys() {
    let mut w = mcp_plain();
    let l0 = w.mcp_console().layout_mode;
    press(&mut w, ch('m'));
    let l1 = w.mcp_console().layout_mode;
    assert_ne!(l0, l1);
    assert_eq!(
        status(&w),
        format!("MCP Window Mode: {} (Press 'd' to cycle dock edge)", l1.label())
    );
    press(&mut w, ch('M'));
    assert_eq!(w.mcp_console().layout_mode, l0);

    let e0 = w.mcp_console().edge;
    press(&mut w, ch('d'));
    assert_eq!(w.mcp_console().edge, e0.cycle());
    assert_eq!(w.mcp_console().layout_mode, DrawerLayoutMode::Push);
    assert_eq!(
        status(&w),
        format!("MCP Window Docked (Push): {}", e0.cycle().label())
    );
    press(&mut w, ch('D'));
    assert_eq!(w.mcp_console().edge, e0.cycle().cycle());
}

#[test]
fn mcp_split_keys_create_windows() {
    for key in [ch('c'), ch('+')] {
        let mut w = mcp_tree();
        let before = w.pane_count();
        press(&mut w, key);
        assert_eq!(w.pane_count(), before + 1);
        let id = w.active_pane_id();
        assert_eq!(status(&w), format!("MCP: Created new window #{id}"));
        assert!(w.mcp_console().active);
    }
    for key in [ch('v'), ch('|')] {
        let mut w = mcp_tree();
        let before = w.pane_count();
        press(&mut w, key);
        assert_eq!(w.pane_count(), before + 1);
        let id = w.active_pane_id();
        assert_eq!(status(&w), format!("MCP: Created new vertical window #{id}"));
    }
}

#[test]
fn mcp_graft_keys_prefill_prompt_and_close_console() {
    for key in [ch('g'), ch('G'), ch('p'), ch('P')] {
        let mut w = mcp_tree();
        let target = match w.mcp_console().selected_action() {
            Some(McpRowAction::SessionNode(n)) => n,
            _ => w.config().history_session.clone(),
        };
        press(&mut w, key);
        assert!(!w.mcp_console().active);
        assert_eq!(w.active_pane().prompt().text(), format!(":mcp graft {target} "));
        assert!(w.active_pane().prompt().force_command_prompt);
    }
    // Non-session selection falls back to the configured history session.
    let mut w = mcp_plain();
    let sess = w.config().history_session.clone();
    press(&mut w, ch('g'));
    assert_eq!(w.active_pane().prompt().text(), format!(":mcp graft {sess} "));
}

#[test]
fn mcp_freeze_branch_and_move_keys_update_tree() {
    let mut w = mcp_tree();
    assert_eq!(
        w.mcp_console().selected_action(),
        Some(McpRowAction::SessionNode("main".into()))
    );
    press(&mut w, ch('f'));
    assert_eq!(status(&w), "MCP: Executed 'freeze main'");
    assert!(w.mcp_console().view_lines.iter().any(|l| l.contains("FROZEN")));
    press(&mut w, ch('F'));
    assert!(
        !w.mcp_console().view_lines.iter().any(|l| l.contains("FROZEN")),
        "second toggle thaws the node"
    );

    for key in [ch('n'), ch('N'), ch('b'), ch('B')] {
        let mut w = mcp_tree();
        let before = w.mcp_console().view_lines.len();
        press(&mut w, key);
        assert!(status(&w).starts_with("MCP: Spawned session branch 'main/sub-"), "{key:?}");
        assert_eq!(w.mcp_console().view_lines.len(), before + 1, "{key:?}");
        assert!(w.mcp_console().active);
    }

    for key in [ch('J'), km(KeyCode::Down, KeyModifiers::SHIFT), km(KeyCode::Down, KeyModifiers::ALT)] {
        let mut w = mcp_tree();
        press(&mut w, key);
        assert_eq!(status(&w), "MCP: Moved session 'main' down", "{key:?}");
    }
    for key in [ch('K'), km(KeyCode::Up, KeyModifiers::SHIFT), km(KeyCode::Up, KeyModifiers::ALT)] {
        // 'main' is already the first session: moving up is a boundary no-op.
        let mut w = mcp_tree();
        let before = w.mcp_console().view_lines.clone();
        press(&mut w, key);
        assert_eq!(w.mcp_console().view_lines, before, "{key:?}");
        assert!(w.mcp_console().active);
    }
}

#[test]
fn mcp_reparent_keys_graft_and_promote_nodes() {
    for key in [ch('>'), km(KeyCode::Right, KeyModifiers::ALT)] {
        let mut w = mcp_tree();
        press(&mut w, ch('n')); // give 'main' a child to reparent against
        press(&mut w, key);
        assert!(status(&w).starts_with("MCP: Grafted 'main' under"), "{key:?}: {}", status(&w));
    }
    for key in [ch('<'), ch('u'), ch('U'), km(KeyCode::Left, KeyModifiers::ALT)] {
        let mut w = mcp_tree();
        press(&mut w, key);
        assert_eq!(status(&w), "MCP: Promoted 'main' to root [IND]", "{key:?}");
        assert!(
            w.mcp_console().view_lines.iter().any(|l| l.contains("main [IND]")),
            "{key:?}"
        );
    }
}

#[test]
fn mcp_ctrl_modified_keys_and_unmapped_keys_do_not_close_console() {
    let mut w = mcp_plain();
    press(&mut w, ch('~'));
    assert!(w.mcp_console().active);
    // Ctrl+<letter> skips the MCP key table entirely: Ctrl+Q reaches the global quit, which on
    // the final root pane opens the Exit Guard instead of closing the console with 'q'.
    let act = press(&mut w, km(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert_eq!(act, AppAction::Handled);
    assert!(matches!(w.modal(), Modal::ExitGuard(_)));
}

#[test]
fn mcp_is_bypassed_while_leader_pending() {
    let mut w = mcp_plain();
    w.set_leader_pending(true);
    press(&mut w, ch('q'));
    assert!(!w.is_leader_pending());
    assert!(w.mcp_console().active, "leader chord, not MCP 'q', consumed the key");
}

// MCP file browser ----------------------------------------------------------------------------

fn browser_wm() -> (WindowManager, std::path::PathBuf) {
    let mut w = wm();
    let dir = std::env::temp_dir().join(format!("bilterm-modal-fb-{}", std::process::id()));
    let sub = dir.join("alpha_dir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(dir.join("beta.txt"), "hello\nworld\n").unwrap();
    std::fs::write(dir.join("gamma.txt"), "g\n").unwrap();
    let canon = std::fs::canonicalize(&dir).unwrap();
    w.mcp_console_mut().active = true;
    w.mcp_console_mut().file_browser = Some(FileBrowserState::new(dir.clone(), None));
    w.mcp_console_mut().refresh_file_browser();
    (w, canon)
}

#[test]
fn file_browser_typing_filters_and_backspace_pops_then_navigates_up() {
    let (mut w, dir) = browser_wm();
    let rows_all = w.mcp_console().view_lines.len();
    assert!(rows_all >= 3);
    press(&mut w, ch('b'));
    press(&mut w, ch('e'));
    assert_eq!(w.mcp_console().file_browser.as_ref().unwrap().filter, "be");
    assert!(w.mcp_console().view_lines.len() < rows_all);
    press(&mut w, k(KeyCode::Backspace));
    assert_eq!(w.mcp_console().file_browser.as_ref().unwrap().filter, "b");
    // Ctrl+U clears the filter.
    press(&mut w, km(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert_eq!(w.mcp_console().file_browser.as_ref().unwrap().filter, "");
    assert_eq!(w.mcp_console().view_lines.len(), rows_all);
    // Backspace with empty filter navigates to the parent directory.
    press(&mut w, k(KeyCode::Backspace));
    let cur = w.mcp_console().file_browser.as_ref().unwrap().current_dir.clone();
    assert_eq!(Some(cur.as_path()), dir.parent());
}

#[test]
fn file_browser_ctrl_and_alt_chars_are_not_typed_into_filter() {
    let (mut w, _) = browser_wm();
    press(&mut w, km(KeyCode::Char('z'), KeyModifiers::ALT));
    assert_eq!(w.mcp_console().file_browser.as_ref().unwrap().filter, "");
    press(&mut w, k(KeyCode::F(6)));
    assert!(w.mcp_console().active);
}

#[test]
fn file_browser_selection_paging_and_enter_activation() {
    let (mut w, dir) = browser_wm();
    let r0 = w.mcp_console().selected_row;
    press(&mut w, k(KeyCode::Down));
    let r1 = w.mcp_console().selected_row;
    assert!(r1 > r0, "Down advances to the next interactive row");
    press(&mut w, k(KeyCode::Up));
    assert_eq!(w.mcp_console().selected_row, r0);
    press(&mut w, k(KeyCode::End));
    let last = w.mcp_console().selected_row;
    assert!(last > r1);
    press(&mut w, k(KeyCode::Home));
    assert_eq!(w.mcp_console().selected_row, r0);
    press(&mut w, k(KeyCode::PageDown));
    assert_eq!(w.mcp_console().selected_row, last.min(r0 + 8).max(r0));
    press(&mut w, k(KeyCode::PageUp));
    assert_eq!(w.mcp_console().selected_row, r0);
    assert!(w.mcp_console().active);
    // Enter on the directory row navigates into it.
    let dir_row = w
        .mcp_console()
        .row_actions
        .iter()
        .position(|a| matches!(a, McpRowAction::BrowseDir(p) if p.ends_with("alpha_dir")));
    let i = dir_row.expect("alpha_dir row present");
    w.mcp_console_mut().selected_row = i;
    press(&mut w, k(KeyCode::Enter));
    let cur = w.mcp_console().file_browser.as_ref().unwrap().current_dir.clone();
    assert_eq!(cur, dir.join("alpha_dir"));
}

#[test]
fn file_browser_esc_closes_console_and_browser() {
    let (mut w, _) = browser_wm();
    press(&mut w, k(KeyCode::Esc));
    assert!(!w.mcp_console().active);
    assert!(w.mcp_console().file_browser.is_none());
}
