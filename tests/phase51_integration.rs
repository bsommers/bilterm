//! Phase 51 cross-plan integration: the AGENTS list keys and mouse driven through the real input
//! path (`handle_wm_key_event` / `handle_wm_mouse_event` -> `Action` -> `WindowManager::apply`) with
//! the resulting state asserted, dock release and real-workspace geometry for a docked list, and
//! menubar priority over the floating agents windows.

use std::sync::Mutex;

use bilterm::action::Action;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::modals::{menubar_category_item_count, menubar_dropdown_rect, menubar_item_command};
use bilterm::ui::shell::ShellLayout;
use bilterm::ui::typography::PanelTarget;
use bilterm::wm::agents_list_cmd::{AgentsListOp, ListFilter};
use bilterm::wm::agents_list_geometry::ListBadge;
use bilterm::wm::panel_dock::DockPanel;
use bilterm::wm::{SplitDirection, WindowManager, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("bilterm-p51-integ-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Serialized by ENV_LOCK.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
        std::env::set_var("XDG_DATA_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn fixture(cols: u16, rows: u16, panes: usize) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn ws_of(wm: &WindowManager, area: Rect) -> Rect {
    ShellLayout::compute(area, wm.config(), wm.pane_count()).workspace
}

fn key(wm: &mut WindowManager, code: KeyCode) {
    handle_wm_key_event(wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

fn ev(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE }
}

fn click(wm: &mut WindowManager, area: Rect, col: u16, row: u16) {
    handle_wm_mouse_event(wm, ev(MouseEventKind::Down(MouseButton::Left), col, row), area).unwrap();
}

fn sizes(wm: &WindowManager) -> Vec<(u16, u16)> {
    wm.ordered_pane_ids().iter().map(|&id| wm.pane(id).unwrap().vt().size()).collect()
}

fn row_ids(wm: &WindowManager) -> Vec<usize> {
    wm.agents_list_rows().iter().map(|r| r.pane_id).collect()
}

// ------------------------------------------------------------ keys, end to end

#[test]
fn e2e_keys_select_jump_pin_filter_scale() {
    with_temp_xdg(|| {
        let (mut wm, _) = fixture(120, 40, 3);
        wm.update_agent_states(std::time::Instant::now());
        let ids = row_ids(&wm);
        assert_eq!(ids.len(), 3);
        let active = wm.active_pane_id();
        // The chord opens the list; a fresh open selects the active pane's row.
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL | KeyModifiers::ALT)).unwrap();
        assert!(wm.agents_list().captures_input());
        assert_eq!(wm.agents_list().selected, Some(active));

        // select: Home / End / Up / Down move the stable selection.
        key(&mut wm, KeyCode::Home);
        assert_eq!(wm.agents_list().selected, Some(ids[0]));
        key(&mut wm, KeyCode::Down);
        assert_eq!(wm.agents_list().selected, Some(ids[1]));
        key(&mut wm, KeyCode::End);
        assert_eq!(wm.agents_list().selected, Some(ids[2]));
        key(&mut wm, KeyCode::Down);
        assert_eq!(wm.agents_list().selected, Some(ids[2]), "clamped at the last row");
        key(&mut wm, KeyCode::Up);
        assert_eq!(wm.agents_list().selected, Some(ids[1]));

        // scale: ] / [ step the panel scale and report through the summary.
        let base = wm.agents_list_summary().scale_pct;
        key(&mut wm, KeyCode::Char(']'));
        assert_eq!(wm.agents_list_summary().scale_pct, base + 10);
        key(&mut wm, KeyCode::Char('['));
        assert_eq!(wm.agents_list_summary().scale_pct, base);

        // filter: panes with no detected agent drop out; the selection never dangles.
        key(&mut wm, KeyCode::Char('f'));
        assert_eq!(wm.agents_list().filter, ListFilter::Agents);
        assert!(row_ids(&wm).is_empty(), "plain shells are not agents");
        assert_eq!(wm.agents_list().selected, None);
        key(&mut wm, KeyCode::Char('f'));
        assert_eq!(wm.agents_list().filter, ListFilter::All);
        assert_eq!(row_ids(&wm), ids);

        // pin: Esc on a pinned list only releases the keyboard.
        key(&mut wm, KeyCode::Char('*'));
        assert!(wm.agents_list().pinned);
        key(&mut wm, KeyCode::Esc);
        assert!(wm.agents_list().active && !wm.agents_list().focused && wm.agents_list().pinned);
        wm.open_agents_list();

        // jump: Enter focuses the selected pane; pinned, the list stays open.
        key(&mut wm, KeyCode::Home);
        let target = ids[0];
        assert_ne!(target, wm.active_pane_id(), "precondition: jump moves focus");
        key(&mut wm, KeyCode::Enter);
        assert_eq!(wm.active_pane_id(), target);
        assert!(wm.agents_list().active, "pinned list survives a jump");
        assert!(!wm.agents_list().focused, "jump hands the keyboard to the pane");

        // Unpinned: jump closes the list.
        wm.open_agents_list();
        key(&mut wm, KeyCode::Char('*'));
        assert!(!wm.agents_list().pinned);
        key(&mut wm, KeyCode::End);
        key(&mut wm, KeyCode::Enter);
        assert_eq!(wm.active_pane_id(), ids[2]);
        assert!(!wm.agents_list().active);
    });
}

#[test]
fn e2e_keys_dock_reflows_and_dismiss_releases() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(120, 40, 3);
        let full = sizes(&wm);
        wm.open_agents_list();
        key(&mut wm, KeyCode::Char('d'));
        assert!(wm.agents_list().docked, "d docks the list");
        let docked = sizes(&wm);
        assert_ne!(docked, full, "docking reflows the panes");
        let slot = wm.agents_list_rect(ws_of(&wm, area)).expect("docked slot");
        for (_, r) in wm.compute_workspace_rects(ws_of(&wm, area)) {
            assert_eq!(r.intersection(slot).area(), 0, "{r:?} overlaps the docked list {slot:?}");
        }
        key(&mut wm, KeyCode::Char('d'));
        assert!(!wm.agents_list().docked);
        assert_eq!(sizes(&wm), full, "releasing the dock reflows back");

        // Esc (Dismiss, unpinned) closes a docked list and gives the strip back.
        key(&mut wm, KeyCode::Char('d'));
        assert_ne!(sizes(&wm), full);
        key(&mut wm, KeyCode::Esc);
        assert!(!wm.agents_list().active && !wm.agents_list().docked);
        assert_eq!(sizes(&wm), full, "closing a docked list reflows the panes");
    });
}

// ------------------------------------------------------------ mouse, end to end

#[test]
fn e2e_mouse_select_jump_dock_close() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    let ws = ws_of(&wm, area);
    let l = wm.agents_list_layout(ws).unwrap();
    let ids = row_ids(&wm);
    let active = wm.active_pane_id();
    let other = ids.iter().position(|&i| i != active).unwrap();
    let (c, r) = (l.rows_area.x + 2, l.rows_area.y + other as u16);
    click(&mut wm, area, c, r);
    assert_eq!(wm.agents_list().selected, Some(ids[other]), "first click selects");
    assert!(wm.agents_list().active);
    click(&mut wm, area, c, r);
    assert_eq!(wm.active_pane_id(), ids[other], "second click jumps");
    assert!(!wm.agents_list().active, "an unpinned list closes on jump");

    // Dock badge docks, close badge closes and releases the dock.
    let full = sizes(&wm);
    wm.open_agents_list();
    let badge = |wm: &WindowManager, b: ListBadge| {
        let l = wm.agents_list_layout(ws).unwrap();
        l.badges.iter().find(|(x, _)| *x == b).map(|(_, r)| (r.x, r.y)).unwrap()
    };
    let (bc, br) = badge(&wm, ListBadge::Dock);
    click(&mut wm, area, bc, br);
    assert!(wm.agents_list().docked);
    assert_ne!(sizes(&wm), full);
    let (bc, br) = badge(&wm, ListBadge::Close);
    click(&mut wm, area, bc, br);
    assert!(!wm.agents_list().active && !wm.agents_list().docked);
    assert_eq!(sizes(&wm), full);

    // Pin badge pins.
    wm.open_agents_list();
    let (bc, br) = badge(&wm, ListBadge::Pin);
    click(&mut wm, area, bc, br);
    assert!(wm.agents_list().pinned);
}

// ------------------------------------------------------------ dock release and real workspace

#[test]
fn close_agents_list_releases_dock_and_reflows() {
    let (mut wm, area) = fixture(120, 40, 3);
    let full = sizes(&wm);
    wm.open_agents_list();
    assert!(wm.set_panel_docked(DockPanel::AgentsList, true));
    assert_ne!(sizes(&wm), full);
    wm.close_agents_list();
    assert!(!wm.agents_list().docked, "close releases the flag");
    assert_eq!(sizes(&wm), full, "panes reflow without waiting for a resize");
    assert_eq!(wm.effective_workspace_area(ws_of(&wm, area)), ws_of(&wm, area));
    // Every closing path: close/toggle/dismiss verbs.
    for op in [AgentsListOp::Close, AgentsListOp::Toggle, AgentsListOp::Dismiss] {
        wm.open_agents_list();
        wm.set_panel_docked(DockPanel::AgentsList, true);
        wm.apply(Action::AgentsList(op.clone())).unwrap();
        assert!(!wm.agents_list().active && !wm.agents_list().docked, "{op:?}");
        assert_eq!(sizes(&wm), full, "{op:?}");
    }
    // A pinned dismiss only blurs: still open and still docked.
    wm.open_agents_list();
    wm.set_panel_docked(DockPanel::AgentsList, true);
    wm.apply(Action::AgentsList(AgentsListOp::Pin(Some(true)))).unwrap();
    wm.apply(Action::AgentsList(AgentsListOp::Dismiss)).unwrap();
    assert!(wm.agents_list().active && wm.agents_list().docked);
}

#[test]
fn docked_list_internal_geometry_uses_the_real_workspace() {
    // 12 rows fit in the docked slot (full workspace height) but not in the floating default
    // window (70% of the height), so scroll clamping reveals which workspace was used.
    let (mut wm, area) = fixture(120, 20, 12);
    wm.open_agents_list();
    assert!(wm.set_panel_docked(DockPanel::AgentsList, true));
    let ws = ws_of(&wm, area);
    let layout = wm.agents_list_layout(ws).expect("docked layout");
    assert_eq!(layout.outer, wm.panel_dock_slot(ws, DockPanel::AgentsList).unwrap(), "slot, not a floating fallback");
    assert!(layout.visible_rows >= 12, "precondition: all rows fit when docked ({})", layout.visible_rows);
    wm.apply(Action::AgentsList(AgentsListOp::Scroll(100))).unwrap();
    assert_eq!(wm.agents_list().scroll, 0, "all rows are visible: nothing to scroll");
    wm.apply(Action::AgentsList(AgentsListOp::Last)).unwrap();
    assert_eq!(wm.agents_list().scroll, 0, "selecting the last row needs no scroll in the tall slot");
}

#[test]
fn docked_list_click_and_render_agree_with_the_slot() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    assert!(wm.set_panel_docked(DockPanel::AgentsList, true));
    let ws = ws_of(&wm, area);
    let l = wm.agents_list_layout(ws).expect("docked layout");
    assert_eq!(l.outer, Rect::new(80, ws.y, 40, ws.height), "right strip, full workspace height");

    // Render: the frame title sits on the slot's top row, the rows inside the rows area.
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(&wm).render(area, &mut buf);
    let top: String = (l.outer.x..l.outer.right()).map(|x| buf[(x, l.outer.y)].symbol().to_string()).collect();
    assert!(top.contains("AGENTS"), "title drawn in the docked slot: {top:?}");

    // Click: a row inside the slot selects it through the real handler.
    let ids = row_ids(&wm);
    let sel = wm.agents_list().selected;
    let target = ids.iter().copied().find(|&i| Some(i) != sel).unwrap();
    let idx = ids.iter().position(|&i| i == target).unwrap();
    click(&mut wm, area, l.rows_area.x + 2, l.rows_area.y + idx as u16);
    assert_eq!(wm.agents_list().selected, Some(target));

    // Panes keep working outside the strip: a click in the first pane focuses it, the list stays open.
    let rects = wm.compute_workspace_rects(ws);
    let (pid, pr) = rects.iter().find(|(id, _)| *id != wm.active_pane_id()).copied().unwrap();
    click(&mut wm, area, pr.x + 2, pr.y + 2);
    assert_eq!(wm.active_pane_id(), pid);
    assert!(wm.agents_list().active && wm.agents_list().docked);
    assert!(!wm.agents_list().focused, "the outside press blurred the list");
}

#[test]
fn scaled_docked_list_hits_the_slot() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    wm.set_panel_docked(DockPanel::AgentsList, true);
    wm.apply(Action::AgentsList(AgentsListOp::ScaleStep(50))).unwrap();
    assert_eq!(wm.agents_list_summary().scale_pct, 150);
    let ws = ws_of(&wm, area);
    let vws = wm.panel_virtual_area(ws, PanelTarget::AgentsList);
    assert!(wm.agents_list_rect(vws).is_some(), "scaled docked slot exists");
    assert!(wm.agents_list().docked);
}

// ------------------------------------------------------------ menubar priority

/// Places `rect` over the View dropdown, opens the menu and clicks the item at `item`.
fn find_entry(cmd: &str) -> (usize, usize) {
    (0..8)
        .flat_map(|c| (0..menubar_category_item_count(c).min(32)).map(move |i| (c, i)))
        .find(|&(c, i)| menubar_item_command(c, i) == Some(cmd))
        .unwrap_or_else(|| panic!("no menu entry for {cmd}"))
}

fn dropdown_item_cell(wm: &WindowManager, area: Rect, cat: usize, item: usize) -> (Rect, u16, u16) {
    let dropdown = menubar_dropdown_rect(cat, ws_of(wm, area));
    let inner = inset_window_frame(dropdown);
    (dropdown, inner.x + 2, inner.y + item as u16)
}

#[test]
fn open_menubar_beats_the_agents_list_window() {
    let (mut wm, area) = fixture(120, 40, 2);
    let (cat, item) = find_entry(":zoom");
    let before_zoom = wm.zoomed_pane();
    wm.set_menubar_open_category(Some(cat));
    let (dropdown, c, r) = dropdown_item_cell(&wm, area, cat, item);
    wm.open_agents_list();
    wm.apply(Action::AgentsList(AgentsListOp::SetRect {
        x: dropdown.x,
        y: dropdown.y,
        w: dropdown.width.max(24),
        h: dropdown.height.max(8),
    }))
    .unwrap();
    wm.set_menubar_open_category(Some(cat));
    let rect = wm.agents_list_rect(ws_of(&wm, area)).unwrap();
    assert!(rect.contains((c, r).into()), "precondition: the list covers the dropdown item");
    let selected = wm.agents_list().selected;
    click(&mut wm, area, c, r);
    assert!(wm.menubar_state().is_none(), "the dropdown item got the click");
    assert_ne!(wm.zoomed_pane(), before_zoom, "and ran its command");
    assert_eq!(wm.agents_list().selected, selected, "the list did not take the click");
}

#[test]
fn open_menubar_beats_the_traffic_window() {
    let (mut wm, area) = fixture(120, 40, 2);
    let (cat, item) = find_entry(":zoom");
    let before_zoom = wm.zoomed_pane();
    wm.open_agents_window(None);
    let (dropdown, c, r) = dropdown_item_cell(&wm, area, cat, item);
    wm.set_agents_rect(Rect::new(dropdown.x, dropdown.y, 60, 20));
    wm.set_menubar_open_category(Some(cat));
    let rect = wm.agents_window_rect(ws_of(&wm, area)).unwrap();
    assert!(rect.contains((c, r).into()), "precondition: the window covers the dropdown item");
    click(&mut wm, area, c, r);
    assert!(wm.menubar_state().is_none(), "the dropdown item got the click");
    assert_ne!(wm.zoomed_pane(), before_zoom, "and ran its command");
}

#[test]
fn windows_still_take_clicks_when_no_menu_is_open() {
    let (mut wm, area) = fixture(120, 40, 2);
    wm.open_agents_list();
    let l = wm.agents_list_layout(ws_of(&wm, area)).unwrap();
    let ids = row_ids(&wm);
    let sel = wm.agents_list().selected;
    let target = ids.iter().copied().find(|&i| Some(i) != sel).unwrap();
    let idx = ids.iter().position(|&i| i == target).unwrap();
    click(&mut wm, area, l.rows_area.x + 2, l.rows_area.y + idx as u16);
    assert_eq!(wm.agents_list().selected, Some(target));
}
