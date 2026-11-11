//! Navigator drawer two-line pane rows: config, `:drawer rows`, layout, hit-testing, scrolling.

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::config::BiltermConfig;
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::drawer::navigator::{
    navigator_comms_cells, navigator_dot_cells, navigator_fold_marker_cells, navigator_row_at,
};
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::drawer::{DrawerConfig, DrawerNavOp};
use bilterm::wm::{DrawerRowKind, SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::Widget;

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd).unwrap().unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

/// Points the config/state dirs at an empty temp dir (once per process) so saved sessions of the
/// developer's machine never show up as drawer rows.
fn isolate_xdg() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::env::temp_dir().join(format!("bilterm-two-line-rows-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: runs once, before any test in this file builds a WindowManager (every one calls `fixture`).
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            std::env::set_var("XDG_STATE_HOME", &dir);
        }
    });
}

/// Navigator drawer open, `n` panes titled p1..pn.
fn fixture(n: usize, cols: u16, rows: u16) -> (WindowManager, Rect) {
    isolate_xdg();
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    while wm.pane_count() < n {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    for id in 1..=n {
        wm.pane_mut(id).unwrap().process_pty_output(format!("\x1b]2;p{id}\x07").as_bytes());
    }
    run_colon(&mut wm, ":set drawer.style navigator");
    wm.resize_all(area).unwrap();
    wm.toggle_drawer();
    wm.drawer_mut().focused = true;
    (wm, area)
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn lines(buf: &Buffer) -> Vec<String> {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| (a.x..a.x + a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect()
}

fn line_of(ls: &[String], needle: &str) -> usize {
    ls.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("{needle} missing:\n{}", ls.join("\n")))
}

#[test]
fn config_default_is_two_and_toml_overrides() {
    assert_eq!(DrawerConfig::default().row_lines, 2);
    assert_eq!(DrawerConfig::default().navigator_row_lines(), 2);
    let cfg = BiltermConfig::from_toml_str("[drawer]\nrow_lines = 1\n").unwrap();
    assert_eq!(cfg.drawer.row_lines, 1);
    assert_eq!(cfg.drawer.navigator_row_lines(), 1);
    let cfg = BiltermConfig::from_toml_str("[drawer]\nrow_lines = 7\n").unwrap();
    assert_eq!(cfg.drawer.navigator_row_lines(), 2, "out-of-range values mean 2");
    let cfg = BiltermConfig::from_toml_str("[drawer]\nstyle = \"navigator\"\n").unwrap();
    assert_eq!(cfg.drawer.row_lines, 2, "omitted key defaults to 2");
}

#[test]
fn colon_drawer_rows_sets_and_validates() {
    let (mut wm, _) = fixture(2, 100, 30);
    assert_eq!(wm.drawer_config().row_lines, 2);
    run_colon(&mut wm, ":drawer rows 1");
    assert_eq!(wm.drawer_config().row_lines, 1);
    run_colon(&mut wm, ":drawer rows 2");
    assert_eq!(wm.drawer_config().row_lines, 2);
    for bad in [":drawer rows", ":drawer rows 3", ":drawer rows x"] {
        assert!(parse_colon_command(bad).unwrap().is_err(), "{bad} must be rejected");
    }
}

#[test]
fn two_line_rows_stack_pane_text_over_detail() {
    let (wm, area) = fixture(2, 100, 30);
    let ls = lines(&render(&wm, area));
    let a = line_of(&ls, "#1 p1");
    let b = line_of(&ls, "#2 p2");
    assert_eq!(b, a + 2, "each pane row takes two lines");
    assert!(!ls[a + 1].contains("#2"), "line 2 is the detail line");
    let detail = wm.drawer_rows()[1].navigator.as_ref().unwrap().detail.clone().expect("detail");
    // The drawer clips the detail to its width, and `user@host` is as long as the machine makes it
    // (CI runners have long host names): the row must start the detail text, not fit all of it.
    let shown: String = detail.chars().take(8).collect();
    assert!(ls[a + 1].contains(&shown), "detail line shows the prompt text: {:?} lacks {shown:?}", ls[a + 1]);
}

#[test]
fn one_line_rows_restore_compact_layout() {
    let (mut wm, area) = fixture(2, 100, 30);
    run_colon(&mut wm, ":drawer rows 1");
    let ls = lines(&render(&wm, area));
    assert_eq!(line_of(&ls, "#2 p2"), line_of(&ls, "#1 p1") + 1);
}

#[test]
fn workspace_and_session_rows_stay_one_line() {
    let (wm, _) = fixture(2, 100, 30);
    let rows = wm.drawer_rows();
    assert!(matches!(rows[0].kind, DrawerRowKind::SessionHeader(_)));
    assert!(rows[0].navigator.as_ref().unwrap().detail.is_none());
    assert!(rows[1].navigator.as_ref().unwrap().detail.is_some(), "pane rows carry a detail line");
}

#[test]
fn hit_testing_covers_both_lines_of_each_row() {
    let (mut wm, area) = fixture(3, 100, 30);
    let ls = lines(&render(&wm, area));
    let first = line_of(&ls, "#1 p1") as u16;
    let col = wm.drawer_rect(area).unwrap().x + 3;
    // rows: 0 = workspace, 1..=3 = panes at lines first, first+2, first+4.
    assert_eq!(navigator_row_at(&wm, area, col, first - 1), Some(0));
    for (i, base) in [first, first + 2, first + 4].into_iter().enumerate() {
        assert_eq!(navigator_row_at(&wm, area, col, base), Some(i + 1), "line 1 of pane {}", i + 1);
        assert_eq!(navigator_row_at(&wm, area, col, base + 1), Some(i + 1), "line 2 of pane {}", i + 1);
    }
    run_colon(&mut wm, ":drawer rows 1");
    for (i, base) in [first, first + 1, first + 2].into_iter().enumerate() {
        assert_eq!(navigator_row_at(&wm, area, col, base), Some(i + 1));
    }
    assert_eq!(navigator_row_at(&wm, area, col, first + 3), None, "no stale second lines");
}

#[test]
fn selection_highlight_covers_both_lines() {
    let (mut wm, area) = fixture(2, 100, 30);
    run_colon(&mut wm, ":drawer row 3"); // pane 2
    let buf = render(&wm, area);
    let y = line_of(&lines(&buf), "#2 p2") as u16;
    let x = wm.drawer_rect(area).unwrap().x + 4;
    let selected = |y: u16| buf[(x, y)].modifier.contains(Modifier::REVERSED);
    assert!(selected(y), "line 1 highlighted");
    assert!(selected(y + 1), "line 2 highlighted");
    assert!(!selected(y - 1), "row above not highlighted");
    assert!(!selected(y + 2), "row below not highlighted");
}

#[test]
fn dot_comms_and_fold_cells_sit_on_line_one() {
    let (mut wm, area) = fixture(2, 100, 30);
    wm.report_agent_state(1, Some(AgentState::Working));
    wm.report_agent_state(2, Some(AgentState::Idle));
    wm.update_agent_states(std::time::Instant::now());
    let ls = lines(&render(&wm, area));
    let a = line_of(&ls, "#1 p1") as u16;
    let ys: Vec<u16> = navigator_dot_cells(&wm, area).iter().map(|d| d.y).collect();
    assert_eq!(ys, vec![a, a + 2], "dots on the first line of each two-line row");
    assert!(navigator_comms_cells(&wm, area).iter().all(|c| (c.y - a).is_multiple_of(2)));
    let fold = navigator_fold_marker_cells(&wm, area);
    assert_eq!(fold.len(), 1);
    assert_eq!(fold[0].y, a - 1, "workspace row stays above the first pane row");
}

#[test]
fn scrolling_keeps_selected_two_line_row_visible() {
    let (mut wm, area) = fixture(8, 100, 14);
    for _ in 0..8 {
        let _ = wm.apply(Action::DrawerNav(DrawerNavOp::Next));
    }
    let sel = wm.drawer().selected_index;
    let ls = lines(&render(&wm, area));
    let dr = wm.drawer_rect(area).unwrap();
    let col = dr.x + 3;
    let y = (dr.y..dr.y + dr.height)
        .find(|&y| navigator_row_at(&wm, area, col, y) == Some(sel))
        .unwrap_or_else(|| panic!("selected row {sel} not visible:\n{}", ls.join("\n")));
    let title = format!("#{sel} p{sel}");
    assert!(ls[usize::from(y)].contains(&title), "line {y} should hold {title}:\n{}", ls.join("\n"));
    assert_eq!(navigator_row_at(&wm, area, col, y + 1), Some(sel), "its detail line is visible too");
}
