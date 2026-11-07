//! Panel dock integration tests: traffic window move un-docks and clamps full workspace,
//! and `:arrange` preserves the no-room notice when panels cannot fit.

use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
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

#[test]
fn move_docked_traffic_window_releases_dock_and_clamps_full_workspace() {
    let (mut wm, area) = fixture(120, 40, 2);
    wm.open_agents_list();
    wm.open_agents_window(None);
    run(&mut wm, ":arrange grid");

    assert!(wm.agents_window().docked, "traffic window should be docked");
    assert!(wm.agents_list().docked, "agents list should be docked");
    let docked_rect = wm.agents_window_rect(area).expect("docked slot");
    assert!(
        docked_rect.x >= 70,
        "dock strip should be placed on the right, got rect {docked_rect:?}"
    );

    // Move the window by -10 columns horizontally and -5 rows vertically.
    wm.move_agents_window(-10, -5);

    assert!(
        !wm.agents_window().docked,
        "moving window must release the dock"
    );
    let moved_rect = wm.agents_window_rect(area).expect("floating rect");
    assert_eq!(
        moved_rect.x,
        docked_rect.x - 10,
        "x coordinate shifted cleanly"
    );
    assert_eq!(
        moved_rect.y,
        docked_rect.y - 5,
        "y coordinate shifted cleanly"
    );
    assert_eq!(moved_rect.width, docked_rect.width);
    assert_eq!(moved_rect.height, docked_rect.height);

    // Move beyond the right edge of workspace: clamps cleanly without jumping to reduced area
    wm.move_agents_window(100, 0);
    let right_clamped = wm.agents_window_rect(area).expect("clamped rect");
    assert_eq!(
        right_clamped.x + right_clamped.width,
        area.width,
        "clamped against full workspace boundary"
    );
}

#[test]
fn arrange_preserves_no_room_status_notice() {
    // 25x8 workspace with 2 panes: not enough room to dock panels (PANE_MIN is 20x6, strip needs 6 rows)
    let (mut wm, _) = fixture(25, 8, 2);
    wm.open_agents_list();
    assert!(wm.agents_list().active);

    run(&mut wm, ":arrange grid");

    assert!(
        !wm.agents_list().docked,
        "list should stay floating when no room"
    );
    let status = wm
        .active_pane()
        .status_message()
        .expect("status message must be present");
    assert!(
        status.contains("(not enough room to dock panels)"),
        "status must preserve no-room notice: '{status}'"
    );
    assert!(
        status.starts_with("Auto-arranged 2 window(s) as 'grid'"),
        "status must announce auto-arrange count and pattern: '{status}'"
    );

    // Also verify :layout columns pattern alias
    run(&mut wm, ":layout columns");
    let status2 = wm
        .active_pane()
        .status_message()
        .expect("status message must be present");
    assert!(
        status2.contains("(not enough room to dock panels)"),
        "status2 must preserve no-room notice: '{status2}'"
    );
}
