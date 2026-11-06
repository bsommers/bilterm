//! Phase 51-09 (UIT-04, D-04): the `:arrange` family docks the agents list, the AGENT TRAFFIC window
//! and the MCP console beside the panes; with no panel open the output is unchanged.

use bilterm::action::Action;
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::wm::drawer::{DrawerEdge, DrawerLayoutMode};
use bilterm::wm::panel_dock::DockPanel;
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::layout::Rect;

fn fixture(cols: u16, rows: u16, panes: usize) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    for _ in 1..panes {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn run(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("{cmd}: {e}"));
    execute_wm_colon_command(wm, parsed);
}

fn pane_rects(wm: &WindowManager, area: Rect) -> Vec<(usize, Rect)> {
    let mut v = wm.compute_workspace_rects(area);
    v.sort_by_key(|(id, _)| *id);
    v
}

fn disjoint(a: Rect, b: Rect) -> bool {
    a.intersection(b).area() == 0
}

fn pane_sizes(wm: &WindowManager) -> Vec<(u16, u16)> {
    wm.ordered_pane_ids().iter().map(|&id| wm.pane(id).unwrap().vt().size()).collect()
}

/// Cells covered by `rects` inside `area` (every cell exactly once means a perfect tiling).
fn assert_tiles(rects: &[(usize, Rect)], area: Rect) {
    let mut seen = vec![0u8; usize::from(area.width) * usize::from(area.height)];
    for (_, r) in rects {
        assert!(r.x >= area.x && r.y >= area.y && r.right() <= area.right() && r.bottom() <= area.bottom(), "{r:?} outside {area:?}");
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                seen[usize::from(y - area.y) * usize::from(area.width) + usize::from(x - area.x)] += 1;
            }
        }
    }
    assert!(seen.iter().all(|&n| n == 1), "panes must tile the whole area exactly once");
}

const PATTERNS: [&str; 6] = ["grid", "columns", "rows", "main-side", "main-top", "stack-grid"];

#[test]
fn arrange_without_panels_is_unchanged() {
    for (cols, rows) in [(120u16, 40u16), (80, 24)] {
        for panes in [1usize, 2, 3, 5] {
            for pat in &PATTERNS[..5] {
                let (mut wm, area) = fixture(cols, rows, panes);
                run(&mut wm, &format!(":arrange {pat}"));
                let a = pane_rects(&wm, area);
                assert_tiles(&a, area);
                assert!(!wm.agents_list().docked && !wm.agents_window().docked);
                run(&mut wm, &format!(":arrange {pat}"));
                assert_eq!(pane_rects(&wm, area), a, "{pat} {panes} panes at {cols}x{rows} is deterministic");
            }
        }
    }
}

#[test]
fn arrange_docks_list_right_when_wide() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    run(&mut wm, ":arrange grid");
    assert!(wm.agents_list().docked);
    let slot = wm.agents_list_rect(area).unwrap();
    assert_eq!(slot, Rect::new(80, 0, 40, 40), "clamp(120/3, 28, 48) = 40 columns spanning the strip");
    for (_, r) in pane_rects(&wm, area) {
        assert!(disjoint(r, slot), "{r:?} overlaps the dock strip");
    }
    assert_tiles(&pane_rects(&wm, area), Rect::new(0, 0, 80, 40));
    wm.close_agents_list();
    wm.set_panel_docked(DockPanel::AgentsList, false);
    run(&mut wm, ":arrange grid");
    assert_tiles(&pane_rects(&wm, area), area);
}

#[test]
fn arrange_docks_bottom_when_narrow() {
    let (mut wm, area) = fixture(80, 24, 3);
    wm.open_agents_list();
    run(&mut wm, ":arrange grid");
    assert!(wm.agents_list().docked);
    let slot = wm.agents_list_rect(area).unwrap();
    assert_eq!(slot, Rect::new(0, 16, 80, 8), "max(6, 24/3) = 8 rows");
    assert_tiles(&pane_rects(&wm, area), Rect::new(0, 0, 80, 16));
}

#[test]
fn arrange_skips_when_no_room() {
    let (mut wm, area) = fixture(30, 10, 2);
    wm.open_agents_list();
    run(&mut wm, ":arrange grid");
    assert!(!wm.agents_list().docked, "leftover under 20x6 keeps the panel floating");
    // `:arrange` itself overwrites the pane status afterwards (layout_profile.rs), so the refusal
    // message is asserted on the direct dock path.
    assert!(!wm.set_panel_docked(DockPanel::AgentsList, true));
    assert_eq!(wm.active_pane().status_message(), Some("Not enough room to dock panels"));
    assert_tiles(&pane_rects(&wm, area), area);
}

#[test]
fn arrange_sets_mcp_push_only_when_active() {
    let (mut wm, area) = fixture(120, 40, 2);
    wm.drawer_config_mut().layout = DrawerLayoutMode::Overlay;
    let before = wm.mcp_console().layout_mode;
    run(&mut wm, ":arrange grid");
    assert_eq!(wm.mcp_console().layout_mode, before, "inactive console untouched");
    assert_tiles(&pane_rects(&wm, area), area);

    wm.mcp_console_mut().active = true;
    run(&mut wm, ":arrange grid");
    assert_eq!(wm.mcp_console().layout_mode, DrawerLayoutMode::Push);
    assert_eq!(wm.drawer_config().layout, DrawerLayoutMode::Overlay, "drawer config is never touched");
    let mcp = wm.mcp_console().panel_rect(area).unwrap();
    for (_, r) in pane_rects(&wm, area) {
        assert!(disjoint(r, mcp));
    }
}

#[test]
fn resize_keeps_tiling() {
    let (mut wm, _) = fixture(120, 40, 3);
    wm.open_agents_list();
    run(&mut wm, ":arrange grid");
    for (cols, rows) in [(100u16, 30u16), (160, 50), (90, 30)] {
        let area = Rect::new(0, 0, cols, rows);
        wm.resize_all(area).unwrap();
        let slot = wm.agents_list_rect(area).unwrap();
        for (_, r) in pane_rects(&wm, area) {
            assert!(disjoint(r, slot), "{cols}x{rows}: {r:?} overlaps {slot:?}");
        }
        if cols >= 100 {
            assert_eq!(slot.width, (cols / 3).clamp(28, 48));
            assert_eq!(slot.right(), cols);
        } else {
            assert_eq!((slot.width, slot.bottom()), (cols, rows), "narrow uses the bottom strip");
        }
    }
}

#[test]
fn scaled_slot_fills() {
    let (mut wm, area) = fixture(120, 40, 2);
    wm.open_agents_list();
    run(&mut wm, ":arrange grid");
    assert_eq!(wm.agents_list_rect(area), Some(Rect::new(80, 0, 40, 40)), "unscaled space");
    wm.set_gui_surface(true);
    wm.set_scale(bilterm::ui::typography::PanelTarget::AgentsList, 125);
    let target = bilterm::ui::typography::PanelTarget::AgentsList;
    assert_eq!(wm.scale_pct(target), 125);
    let vws = wm.panel_virtual_area(area, target);
    let virt = wm.agents_list_rect(vws).unwrap();
    let back = bilterm::ui::typography::scaled_panel_real_rect(area, virt, 125);
    let slack = |a: u16, b: u16| a.abs_diff(b) <= 1;
    assert!(slack(back.x, 80) && slack(back.y, 0) && slack(back.right(), 120) && slack(back.bottom(), 40), "{back:?}");
    assert!(back.x >= 80 - 1);
    assert!(virt.right() <= vws.right() && virt.bottom() <= vws.bottom());
}

#[test]
fn layout_cycle_does_not_dock() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    wm.open_agents_window(None);
    let before = pane_rects(&wm, area);
    for cmd in [":layout tile", ":layout stack", ":layout cycle", ":layout tile"] {
        run(&mut wm, cmd);
        assert!(!wm.agents_list().docked && !wm.agents_window().docked, "{cmd}");
    }
    wm.apply(Action::LayoutCycle).unwrap();
    assert!(!wm.agents_list().docked && !wm.agents_window().docked);
    // Tiled again: same rects as before any layout command (panels never reserve space).
    wm.apply(Action::LayoutCycle).unwrap();
    run(&mut wm, ":layout tile");
    assert_eq!(pane_rects(&wm, area).len(), before.len());
    assert_tiles(&pane_rects(&wm, area), area);
}

#[test]
fn pre_dock_area_matches_effective() {
    let (mut wm, _) = fixture(120, 40, 2);
    let full = Rect::new(0, 0, 120, 40);
    for edge in [DrawerEdge::Left, DrawerEdge::Right, DrawerEdge::Top, DrawerEdge::Bottom] {
        for mcp in [false, true] {
            for push in [false, true] {
                wm.drawer_config_mut().edge = edge;
                wm.drawer_config_mut().layout = if push { DrawerLayoutMode::Push } else { DrawerLayoutMode::Overlay };
                wm.drawer_mut().open = true;
                wm.mcp_console_mut().active = mcp;
                wm.mcp_console_mut().layout_mode = DrawerLayoutMode::Push;
                assert_eq!(wm.pre_dock_area(full), wm.effective_workspace_area(full), "{edge:?} mcp={mcp} push={push}");
            }
        }
    }
    wm.drawer_mut().open = false;
    wm.mcp_console_mut().active = false;
    wm.open_agents_list();
    assert_eq!(wm.pre_dock_area(full), full);
    wm.set_panel_docked(DockPanel::AgentsList, true);
    assert_eq!(wm.pre_dock_area(full), full);
    assert_eq!(wm.effective_workspace_area(full), Rect::new(0, 0, 80, 40));
}

#[test]
fn arrange_docks_both_panels_stacked() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    wm.open_agents_window(None);
    run(&mut wm, ":arrange grid");
    let list = wm.agents_list_rect(area).unwrap();
    let traffic = wm.agents_window_rect(area).unwrap();
    assert!(wm.agents_list().docked && wm.agents_window().docked);
    assert!(disjoint(list, traffic));
    assert_eq!(list.height, traffic.height, "equal heights");
    assert_eq!(list.x, traffic.x);
    assert!(list.y < traffic.y, "list on top, traffic below");
    assert!(traffic.bottom() <= 40);
    for (_, r) in pane_rects(&wm, area) {
        assert!(disjoint(r, list) && disjoint(r, traffic));
    }
}

#[test]
fn traffic_window_docks_on_arrange() {
    let (mut wm, area) = fixture(120, 40, 2);
    let full_sizes = pane_sizes(&wm);
    wm.open_agents_window(None);
    run(&mut wm, ":arrange grid");
    assert!(wm.agents_window().docked);
    let slot = wm.agents_window_rect(area).unwrap();
    assert_eq!(slot, Rect::new(80, 0, 40, 40));
    assert_ne!(pane_sizes(&wm), full_sizes, "pane PTYs shrink when docked");
    for (_, r) in pane_rects(&wm, area) {
        assert!(disjoint(r, slot));
    }
    run(&mut wm, ":agents close");
    assert!(!wm.agents_window().docked);
    assert_tiles(&pane_rects(&wm, area), area);
    run(&mut wm, ":arrange grid");
    assert_eq!(pane_sizes(&wm), full_sizes, "PTYs back to the full workspace");
}

#[test]
fn manual_geometry_releases_dock() {
    for cmd in [
        ":agents move 1 0",
        ":agents resize 1 0",
        ":agents rect 5 5 60 20",
        ":agents size 60x20",
        ":agents size auto",
        ":agents max",
    ] {
        let (mut wm, area) = fixture(120, 40, 2);
        let full_sizes = pane_sizes(&wm);
        wm.open_agents_window(None);
        run(&mut wm, ":arrange grid");
        assert!(wm.agents_window().docked, "{cmd}");
        run(&mut wm, cmd);
        assert!(!wm.agents_window().docked, "{cmd} releases");
        assert_tiles(&pane_rects(&wm, area), area);
        assert_eq!(pane_sizes(&wm), full_sizes, "{cmd} reflows the panes");
    }
    // A drag lowers to SetRect.
    let (mut wm, area) = fixture(120, 40, 2);
    wm.open_agents_window(None);
    run(&mut wm, ":arrange grid");
    wm.set_agents_rect(Rect::new(10, 3, 60, 20));
    assert!(!wm.agents_window().docked);
    assert_tiles(&pane_rects(&wm, area), area);
}

#[test]
fn arrange_is_stable_across_repeats() {
    let (mut wm, area) = fixture(120, 40, 3);
    wm.open_agents_list();
    wm.open_agents_window(None);
    run(&mut wm, ":arrange grid");
    let panes = pane_rects(&wm, area);
    let slots = (wm.agents_list_rect(area), wm.agents_window_rect(area));
    run(&mut wm, ":arrange grid");
    assert_eq!(pane_rects(&wm, area), panes);
    assert_eq!((wm.agents_list_rect(area), wm.agents_window_rect(area)), slots);
    run(&mut wm, ":layout cycle");
    assert!(wm.agents_list().docked && wm.agents_window().docked, "cycle keeps docks");
    wm.set_agents_rect(Rect::new(1, 1, 50, 20));
    assert!(wm.agents_list().docked, "list stays docked");
    assert!(!wm.agents_window().docked);
    // The traffic window left the strip; the list now owns all of it.
    assert_eq!(wm.agents_list_rect(area), Some(Rect::new(80, 0, 40, 40)));
}

#[test]
fn agents_window_rect_unchanged_when_not_docked() {
    let (mut wm, area) = fixture(120, 40, 2);
    wm.open_agents_window(None);
    assert_eq!(wm.agents_window_rect(area), Some(Rect::new(12, 6, 96, 28)), "centred 80% x 70%");
    wm.set_agents_rect(Rect::new(5, 5, 60, 20));
    assert_eq!(wm.agents_window_rect(area), Some(Rect::new(5, 5, 60, 20)));
    wm.toggle_agents_maximize();
    assert_eq!(wm.agents_window_rect(area), Some(area));
}

#[test]
fn window_file_size() {
    let n = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/wm/agents_window.rs"))
        .unwrap()
        .lines()
        .count();
    assert!(n <= 1190, "agents_window.rs has {n} lines");
}

#[test]
fn stack_grid_floating_rects_account_for_docks() {
    let (mut wm, area) = fixture(120, 40, 4);
    wm.open_agents_list();
    run(&mut wm, ":arrange stack-grid");
    assert!(wm.agents_list().docked);
    let slot = wm.agents_list_rect(area).unwrap();
    let rects = pane_rects(&wm, area);
    assert_eq!(rects.len(), 4);
    for (_, r) in &rects {
        assert!(disjoint(*r, slot), "{r:?} overlaps {slot:?}");
    }
    for (i, (_, a)) in rects.iter().enumerate() {
        for (_, b) in &rects[i + 1..] {
            assert!(disjoint(*a, *b), "floating grid cells overlap: {a:?} {b:?}");
        }
    }
}

#[test]
fn arrange_autosave_ignores_docked_flag() {
    let saved = |with_list: bool| {
        let tmp = tempfile::tempdir().unwrap();
        let (mut wm, _) = fixture(120, 40, 3);
        wm.config_mut().session_dir = Some(tmp.path().to_string_lossy().into_owned());
        if with_list {
            wm.open_agents_list();
        }
        wm.auto_arrange_windows("grid");
        assert_eq!(wm.agents_list().docked, with_list);
        std::fs::read_to_string(tmp.path().join("default.toml")).unwrap()
    };
    // The persisted profile carries split tree and mode only, never the docked flag.
    assert_eq!(saved(false), saved(true));
}

#[test]
fn dock_flag_not_in_config() {
    let (mut wm, _) = fixture(120, 40, 2);
    wm.open_agents_list();
    run(&mut wm, ":arrange grid");
    assert!(wm.agents_list().docked);
    assert!(!format!("{:?}", wm.config()).contains("docked"));
}
