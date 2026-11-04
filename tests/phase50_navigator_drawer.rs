//! Phase 50-02: navigator-style drawer tree (FLT-01, D-02). `style_` = config seam, `tree_` = row model + renderer.

use std::sync::Mutex;

use bilterm::colon_command::parse_colon_command;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::traffic::{TrafficEndpoint, TrafficKind};
use bilterm::wm::drawer::{DrawerConfig, DrawerStyle};
use bilterm::wm::{DrawerRowKind, SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-navigator-drawer-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY-equivalent: serialized by ENV_LOCK; every test in this file goes through here.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// Two-pane fixture with explicit OSC 2 titles.
fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.pane_mut(1).unwrap().process_pty_output(b"\x1b]2;alpha\x07ALPHA_OUT\r\n");
    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(p2).unwrap().process_pty_output(b"\x1b]2;beta\x07BETA_OUT\r\n");
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn open_drawer(wm: &mut WindowManager) {
    wm.toggle_drawer();
    wm.drawer_mut().focused = true;
}

fn render(wm: &WindowManager, area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn buffer_lines(buf: &Buffer) -> Vec<String> {
    let a = buf.area;
    (a.y..a.y + a.height)
        .map(|y| (a.x..a.x + a.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect()
}

#[test]
fn style_default_is_classic() {
    let d = DrawerConfig::default();
    assert_eq!(d.style, DrawerStyle::Classic);
    assert_eq!(d.navigator_width_cols, 28);
}

#[test]
fn style_set_via_colon_set_switches_rows() {
    with_temp_xdg(|| {
        let (mut wm, _area) = fixture(100, 30);
        run_colon(&mut wm, ":set drawer.style navigator");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Navigator);
        let rows = wm.drawer_rows();
        assert!(matches!(rows[0].kind, DrawerRowKind::SessionHeader(_)));
        assert!(rows[0].navigator.is_some());
        assert_eq!(rows[1].kind, DrawerRowKind::Pane(1));
        assert_eq!(rows[2].kind, DrawerRowKind::Pane(2));
    });
}

#[test]
fn tree_tracer_renders_one_line_per_pane() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        run_colon(&mut wm, ":set drawer.style navigator");
        open_drawer(&mut wm);
        let lines = buffer_lines(&render(&wm, area));
        assert!(lines.iter().any(|l| l.contains("WORKSPACES")), "title missing:\n{}", lines.join("\n"));
        let alpha = lines.iter().position(|l| l.contains("#1 alpha"));
        let beta = lines.iter().position(|l| l.contains("#2 beta"));
        assert!(alpha.is_some() && beta.is_some(), "pane lines missing:\n{}", lines.join("\n"));
        assert_ne!(alpha, beta, "panes must be on separate lines");
        assert_eq!(wm.drawer_rect(area).unwrap().width, 28);
    });
}

// ---- Task 2: full tree semantics ----

fn navigator_wm(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let (mut wm, area) = fixture(cols, rows);
    run_colon(&mut wm, ":set drawer.style navigator");
    (wm, area)
}

fn kinds_depths(wm: &WindowManager) -> Vec<(DrawerRowKind, u8)> {
    wm.drawer_rows()
        .into_iter()
        .map(|r| (r.kind.clone(), r.navigator.as_ref().expect("navigator meta").depth))
        .collect()
}

fn session(name: &str) -> DrawerRowKind {
    DrawerRowKind::SessionHeader(name.to_string())
}

/// Tree main > build > default, where "default" is the active session (headless default name).
fn nested_tree(wm: &mut WindowManager) {
    let t = wm.session_tree_mut();
    t.branch("main", "build").unwrap();
    t.branch("build", "default").unwrap();
}

/// Lines of the drawer interior only (inside the frame), for glyph checks.
fn drawer_interior(wm: &WindowManager, area: Rect) -> Vec<String> {
    let buf = render(wm, area);
    let r = wm.drawer_rect(area).unwrap();
    (r.y + 1..r.y + r.height - 1)
        .map(|y| (r.x + 1..r.x + r.width - 1).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
        .collect()
}

#[test]
fn tree_nested_sessions_indent_by_depth() {
    with_temp_xdg(|| {
        let (mut wm, _a) = navigator_wm(100, 30);
        nested_tree(&mut wm);
        let got = kinds_depths(&wm);
        assert_eq!(
            got,
            vec![
                (session("main"), 0),
                (session("build"), 1),
                (session("default"), 2),
                (DrawerRowKind::Pane(1), 3),
                (DrawerRowKind::Pane(2), 3),
            ]
        );
        let rows = wm.drawer_rows();
        assert_eq!(rows[0].navigator.as_ref().unwrap().level, bilterm::wm::drawer::NavigatorLevel::Workspace);
        assert_eq!(rows[1].navigator.as_ref().unwrap().level, bilterm::wm::drawer::NavigatorLevel::Session);
        assert!(rows[2].navigator.as_ref().unwrap().live);
        assert!(!rows[0].navigator.as_ref().unwrap().live);
    });
}

#[test]
fn tree_root_without_children_lists_panes_at_depth_1() {
    with_temp_xdg(|| {
        let (wm, _a) = navigator_wm(100, 30);
        assert_eq!(
            kinds_depths(&wm),
            vec![(session("default"), 0), (DrawerRowKind::Pane(1), 1), (DrawerRowKind::Pane(2), 1)]
        );
    });
}

#[test]
fn tree_fold_hides_descendants() {
    with_temp_xdg(|| {
        let (mut wm, _a) = navigator_wm(100, 30);
        nested_tree(&mut wm);
        assert!(wm.session_tree_mut().toggle_fold("main"));
        let rows = wm.drawer_rows();
        assert_eq!(rows.len(), 1, "only the collapsed workspace remains: {:?}", rows.iter().map(|r| &r.kind).collect::<Vec<_>>());
        let meta = rows[0].navigator.as_ref().unwrap();
        assert!(meta.collapsed && meta.has_children);
    });
}

#[test]
fn tree_saved_session_rollup() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        wm.session_tree_mut().upsert_session("archive", bilterm::wm::SessionNodeKind::Independent, None);
        wm.session_tree_mut().node_mut("archive").unwrap().pane_count = 3;
        let rows = wm.drawer_rows();
        let saved = rows.iter().find(|r| r.kind == session("archive")).expect("saved root row");
        let meta = saved.navigator.as_ref().unwrap();
        assert!(!meta.live);
        assert_eq!(meta.pane_count, 3);
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        assert!(inner.iter().any(|l| l.contains("archive") && l.trim_end().ends_with("3p -")), "{}", inner.join("\n"));
    });
}

#[test]
fn tree_marks_current_pane() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        open_drawer(&mut wm);
        let rows = wm.drawer_rows();
        let focused = rows.iter().find(|r| r.is_focused_pane).expect("a focused pane row");
        let DrawerRowKind::Pane(fid) = focused.kind else { panic!("focused row is not a pane") };
        let inner = drawer_interior(&wm, area);
        for r in rows.iter().filter(|r| matches!(r.kind, DrawerRowKind::Pane(_))) {
            let DrawerRowKind::Pane(id) = r.kind else { unreachable!() };
            let line = inner.iter().find(|l| l.contains(&format!("#{id} "))).expect("pane line");
            assert_eq!(line.starts_with('›'), id == fid, "marker mismatch for #{id}: {line:?}");
        }
    });
}

#[test]
fn tree_one_line_per_pane_in_liquid_glass() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        run_colon(&mut wm, ":liquid on");
        open_drawer(&mut wm);
        let lines = buffer_lines(&render(&wm, area));
        let a = lines.iter().position(|l| l.contains("#1 alpha")).expect("alpha line");
        let b = lines.iter().position(|l| l.contains("#2 beta")).expect("beta line");
        assert_eq!(b, a + 1, "rows must be single-line (no 2-row cards)");
    });
}

#[test]
fn tree_sanitizes_titles() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        wm.pane_mut(1).unwrap().set_title("ev\u{202E}il\u{009B}title");
        wm.pane_mut(2).unwrap().set_title("L".repeat(500));
        open_drawer(&mut wm);
        for r in wm.drawer_rows() {
            assert!(!r.title.contains('\u{202E}') && !r.title.contains('\u{009B}'), "{:?}", r.title);
            assert!(r.title.chars().count() <= 64);
        }
        let inner = drawer_interior(&wm, area);
        let text = inner.join("\n");
        assert!(text.contains("#1 eviltitle"), "sanitised title missing:\n{text}");
        assert!(!text.contains('\u{202E}') && !text.contains('\u{009B}'));
        let long = inner.iter().find(|l| l.contains("#2 LLL")).expect("long title line");
        assert!(long.contains('…'), "truncated title needs an ellipsis: {long:?}");
    });
}

#[test]
fn tree_ascii_glyph_pack() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        run_colon(&mut wm, ":set glyphs ascii");
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        assert!(inner.iter().any(|l| l.contains("#1 alpha")));
        for l in &inner {
            assert!(l.is_ascii(), "non-ASCII glyph in ascii pack: {l:?}");
        }
        assert!(inner.iter().any(|l| l.starts_with('>')), "ascii current marker missing");
    });
}

#[test]
fn tree_width_resize_writes_navigator_width() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        run_colon(&mut wm, ":drawer size 30");
        open_drawer(&mut wm);
        assert_eq!(wm.drawer_config().navigator_width_cols, 30);
        assert_eq!(wm.drawer_config().width_cols, 34);
        assert_eq!(wm.drawer_rect(area).unwrap().width, 30);
    });
}

#[test]
fn tree_top_edge_renders_vertical_list() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        run_colon(&mut wm, ":drawer top");
        open_drawer(&mut wm);
        let r = wm.drawer_rect(area).unwrap();
        assert_eq!(r.width, 100);
        let lines = buffer_lines(&render(&wm, area));
        let a = lines.iter().position(|l| l.contains("#1 alpha")).expect("alpha line");
        let b = lines.iter().position(|l| l.contains("#2 beta")).expect("beta line");
        assert_eq!(b, a + 1);
        assert!(a as u16 >= r.y && (b as u16) < r.y + r.height, "pane lines must sit inside the top drawer rect");
    });
}

// ---- Phase 50-04: state dots, counts, attention navigation (FLT-01, D-04/D-05) ----

/// Three-pane navigator fixture (ids 1..=3), titles alpha/beta/gamma.
fn three_pane_navigator(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let (mut wm, area) = navigator_wm(cols, rows);
    let p3 = wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.pane_mut(p3).unwrap().process_pty_output(b"\x1b]2;gamma\x07GAMMA_OUT\r\n");
    wm.resize_all(area).unwrap();
    (wm, area)
}

#[test]
fn attention_tracer_blocked_dot_and_jump() {
    with_temp_xdg(|| {
        let (mut wm, area) = three_pane_navigator(100, 30);
        wm.focus_and_raise(1);
        wm.report_agent_state(3, Some(AgentState::Blocked));
        wm.update_agent_states(std::time::Instant::now());
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        let line = inner.iter().find(|l| l.contains("#3 ")).expect("#3 line");
        assert!(line.contains('▲'), "blocked dot missing: {line:?}");
        run_colon(&mut wm, ":drawer attention");
        assert_eq!(wm.active_pane_id(), 3);
    });
}

fn now() -> std::time::Instant {
    std::time::Instant::now()
}

/// Reports `states[i]` for pane `i + 1`, applies them and returns the WM.
fn with_states(wm: &mut WindowManager, states: &[(u32, Option<AgentState>)]) {
    for &(id, st) in states {
        wm.report_agent_state(id as _, st);
    }
    wm.update_agent_states(now());
}

/// Splits until `n` panes exist and focuses pane 1.
fn n_pane_navigator(n: u32, cols: u16, rows: u16) -> (WindowManager, Rect) {
    let (mut wm, area) = navigator_wm(cols, rows);
    while (wm.pane_count() as u32) < n {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(area).unwrap();
    wm.focus_and_raise(1);
    (wm, area)
}

fn line_of<'a>(lines: &'a [String], needle: &str) -> &'a str {
    lines.iter().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("no line with {needle:?}:\n{}", lines.join("\n")))
}

#[test]
fn dots_render_all_five_states() {
    with_temp_xdg(|| {
        let (mut wm, area) = n_pane_navigator(5, 100, 30);
        with_states(
            &mut wm,
            &[
                (1, Some(AgentState::Working)),
                (2, Some(AgentState::Blocked)),
                (3, Some(AgentState::Idle)),
                (4, Some(AgentState::Done)),
                (5, Some(AgentState::Errored)),
            ],
        );
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        for (id, glyph) in [(1, '⠋'), (2, '▲'), (3, '○'), (4, '●'), (5, '✗')] {
            let l = line_of(&inner, &format!("#{id} "));
            assert!(l.contains(glyph), "#{id} wants {glyph}: {l:?}");
        }
        run_colon(&mut wm, ":set glyphs ascii");
        let inner = drawer_interior(&wm, area);
        for (id, glyph) in [(1, '*'), (2, '!'), (3, '-'), (4, '+'), (5, 'x')] {
            let l = line_of(&inner, &format!("#{id} "));
            let prefix: String = l.chars().take_while(|c| *c != '#').collect();
            assert!(prefix.contains(glyph), "#{id} ascii dot {glyph} missing before label: {l:?}");
            assert!(l.is_ascii(), "{l:?}");
        }
    });
}

#[test]
fn dots_real_signal_errored() {
    with_temp_xdg(|| {
        let (mut wm, area) = n_pane_navigator(2, 100, 30);
        wm.pane_mut(2).unwrap().process_pty_output(b"boom\r\n\x1b]133;D;42\x07");
        wm.update_agent_states(now());
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        assert!(line_of(&inner, "#2 ").contains('✗'), "{}", inner.join("\n"));
        assert!(!line_of(&inner, "#1 ").contains('✗'));
    });
}

#[test]
fn dots_workspace_counts_summary() {
    with_temp_xdg(|| {
        let (mut wm, area) = n_pane_navigator(3, 100, 30);
        with_states(
            &mut wm,
            &[(1, Some(AgentState::Blocked)), (2, Some(AgentState::Errored)), (3, Some(AgentState::Working))],
        );
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        let ws = line_of(&inner, "default");
        assert!(ws.contains("▲1 ✗1 ⠋1"), "counts summary missing: {ws:?}");
        assert!(ws.trim_end().ends_with("⠋1"), "counts are right aligned: {ws:?}");
        // Narrow drawer: only the segments that fit the space left after the name remain.
        run_colon(&mut wm, ":drawer size 18");
        let inner = drawer_interior(&wm, area);
        let ws = line_of(&inner, "default");
        assert!(ws.contains("▲1 ✗1") && !ws.contains("⠋1"), "truncated summary wrong: {ws:?}");
        assert!(unicode_width::UnicodeWidthStr::width(ws) <= 16, "{ws:?}");
    });
}

#[test]
fn dots_saved_workspace_shows_count_and_dash() {
    with_temp_xdg(|| {
        let (mut wm, area) = n_pane_navigator(2, 100, 30);
        wm.session_tree_mut().upsert_session("archive", bilterm::wm::SessionNodeKind::Independent, None);
        wm.session_tree_mut().node_mut("archive").unwrap().pane_count = 3;
        with_states(&mut wm, &[(1, Some(AgentState::Blocked))]);
        open_drawer(&mut wm);
        let inner = drawer_interior(&wm, area);
        let saved = line_of(&inner, "archive");
        assert!(saved.trim_end().ends_with("3p -"), "{saved:?}");
        for g in ['▲', '○', '●', '✗', '⠋'] {
            assert!(!saved.contains(g), "saved workspace must show no dots: {saved:?}");
        }
    });
}

fn jump(wm: &mut WindowManager) -> u32 {
    run_colon(wm, ":drawer attention");
    wm.active_pane_id() as u32
}

#[test]
fn attention_prefers_blocked_then_errored_then_done() {
    with_temp_xdg(|| {
        let (mut wm, _a) = n_pane_navigator(4, 100, 30);
        with_states(
            &mut wm,
            &[(1, Some(AgentState::Blocked)), (2, Some(AgentState::Errored)), (3, Some(AgentState::Done))],
        );
        wm.focus_and_raise(4);
        assert_eq!(jump(&mut wm), 1);
        assert_eq!(jump(&mut wm), 1, "stays in the Blocked tier while one remains");
        with_states(&mut wm, &[(1, None)]);
        assert_eq!(jump(&mut wm), 2, "then Errored");
        with_states(&mut wm, &[(2, None)]);
        assert_eq!(jump(&mut wm), 3, "then Done");
    });
}

#[test]
fn attention_cycles_and_wraps() {
    with_temp_xdg(|| {
        let (mut wm, _a) = n_pane_navigator(4, 100, 30);
        with_states(&mut wm, &[(2, Some(AgentState::Blocked)), (4, Some(AgentState::Blocked))]);
        assert_eq!(wm.active_pane_id(), 1);
        assert_eq!(jump(&mut wm), 2, "starts after the active pane");
        assert_eq!(jump(&mut wm), 4);
        assert_eq!(jump(&mut wm), 2, "wraps around");
    });
}

#[test]
fn attention_noop_when_none_sets_status() {
    with_temp_xdg(|| {
        let (mut wm, _a) = n_pane_navigator(3, 100, 30);
        wm.focus_and_raise(2);
        with_states(&mut wm, &[(1, Some(AgentState::Working))]);
        run_colon(&mut wm, ":drawer attention");
        assert_eq!(wm.active_pane_id(), 2);
        assert_eq!(wm.active_pane().status_message(), Some("No pane needs attention"));
    });
}

#[test]
fn attention_leader_bang_with_drawer_closed() {
    use bilterm::input::handle_wm_key_event;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    with_temp_xdg(|| {
        let (mut wm, area) = n_pane_navigator(3, 100, 30);
        with_states(&mut wm, &[(3, Some(AgentState::Blocked))]);
        assert!(wm.drawer_rect(area).is_none(), "drawer starts closed");
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)).unwrap();
        assert!(wm.is_leader_pending());
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE)).unwrap();
        assert_eq!(wm.active_pane_id(), 3);
    });
}

#[test]
fn style_colon_drawer_style_sets_and_toggles() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        run_colon(&mut wm, ":drawer style navigator");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Navigator);
        open_drawer(&mut wm);
        assert_eq!(wm.drawer_rect(area).unwrap().width, 28);
        run_colon(&mut wm, ":drawer style classic");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Classic);
        assert_eq!(wm.drawer_rect(area).unwrap().width, 34);
        run_colon(&mut wm, ":drawer style");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Navigator, "bare form toggles");
        run_colon(&mut wm, ":drawer style toggle");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Classic);
        assert!(parse_colon_command(":drawer style nope").unwrap().is_err());
    });
}

#[test]
fn keymap_leader_bindings_have_unique_keys() {
    let mut seen = std::collections::HashSet::new();
    for (key, _) in bilterm::input::keymap::DEFAULT_LEADER_BINDINGS {
        assert!(seen.insert(*key), "duplicate leader key {key:?}");
    }
}

#[test]
fn style_legacy_herdr_alias_maps_to_navigator() {
    with_temp_xdg(|| {
        let (mut wm, _area) = fixture(100, 30);
        run_colon(&mut wm, ":drawer style herdr");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Navigator);
        let cfg: DrawerConfig = toml::from_str("style = \"herdr\"\nherdr_width_cols = 31\n").unwrap();
        assert_eq!(cfg.style, DrawerStyle::Navigator);
        assert_eq!(cfg.navigator_width_cols, 31);
        let cfg: DrawerConfig = toml::from_str("style = \"navigator\"\nnavigator_width_cols = 29\n").unwrap();
        assert_eq!((cfg.style, cfg.navigator_width_cols), (DrawerStyle::Navigator, 29));
    });
}

// ---- Phase 50-06: keyboard-first navigator drawer (FLT-01, success criteria 1 and 3) ----

use bilterm::input::handle_wm_key_event;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(wm: &mut WindowManager, code: KeyCode) {
    handle_wm_key_event(wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

fn press_mod(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn ch(wm: &mut WindowManager, c: char) {
    press(wm, KeyCode::Char(c));
}

fn pane_row_count(wm: &WindowManager) -> usize {
    wm.drawer_rows().iter().filter(|r| matches!(r.kind, DrawerRowKind::Pane(_))).count()
}

#[test]
fn keys_tracer_fold_by_key_and_command() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        open_drawer(&mut wm);
        wm.drawer_mut().selected_index = 0;
        assert_eq!(pane_row_count(&wm), 2);

        ch(&mut wm, 'e');
        assert_eq!(pane_row_count(&wm), 0, "e folds the workspace row");
        let lines = buffer_lines(&render(&wm, area));
        assert!(!lines.iter().any(|l| l.contains("#1 alpha")), "pane rows gone from the render");

        run_colon(&mut wm, ":drawer fold");
        assert_eq!(pane_row_count(&wm), 2, ":drawer fold unfolds again");

        wm.drawer_mut().selected_index = 2; // pane #2
        run_colon(&mut wm, ":drawer enter");
        assert_eq!(wm.active_pane_id(), 2);
        wm.drawer_mut().selected_index = 1;
        run_colon(&mut wm, ":drawer enter");
        assert_eq!(wm.active_pane_id(), 1);
    });
}

// ---- Task 2: full navigator key table ----

use bilterm::action::Action;
use bilterm::input::keymap::{DEFAULT_GLOBAL_BINDINGS, NAVIGATOR_DRAWER_BINDINGS, default_global_action};
use bilterm::wm::drawer::DrawerNavOp;

/// Focused navigator drawer over `n` panes (focus on pane 1, selection on the first pane row).
fn focused_navigator(n: u32) -> (WindowManager, Rect) {
    let (mut wm, area) = n_pane_navigator(n, 100, 30);
    open_drawer(&mut wm);
    wm.drawer_mut().selected_index = 1;
    (wm, area)
}

#[test]
fn keys_jk_and_arrows_move_selection() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(3);
        wm.drawer_mut().selected_index = 0;
        ch(&mut wm, 'j');
        assert_eq!(wm.drawer().selected_index, 1);
        press(&mut wm, KeyCode::Down);
        assert_eq!(wm.drawer().selected_index, 2);
        ch(&mut wm, 'k');
        assert_eq!(wm.drawer().selected_index, 1);
        press(&mut wm, KeyCode::Up);
        assert_eq!(wm.drawer().selected_index, 0);
        // Shift/Alt+arrows keep their classic session-move meaning and never step the selection.
        press_mod(&mut wm, KeyCode::Down, KeyModifiers::SHIFT);
        assert_eq!(wm.drawer().selected_index, 0);
        press_mod(&mut wm, KeyCode::Down, KeyModifiers::ALT);
        assert_eq!(wm.drawer().selected_index, 0);
    });
}

#[test]
fn keys_enter_and_space_focus_selected_pane() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(3);
        wm.drawer_mut().selected_index = 3;
        press(&mut wm, KeyCode::Enter);
        assert_eq!(wm.active_pane_id(), 3);
        wm.drawer_mut().selected_index = 2;
        ch(&mut wm, ' ');
        assert_eq!(wm.active_pane_id(), 2);
    });
}

#[test]
fn keys_bang_jumps_to_attention_in_drawer() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(3);
        with_states(&mut wm, &[(3, Some(AgentState::Blocked))]);
        ch(&mut wm, '!');
        assert_eq!(wm.active_pane_id(), 3);
        assert_eq!(wm.selected_drawer_pane_id(), Some(3), "selection follows the jump");
    });
}

#[test]
fn keys_capital_n_creates_session_and_selects_it() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(2);
        let before = wm.config().history_session.clone();
        ch(&mut wm, 'N');
        assert_ne!(wm.config().history_session, before, "N switches to a new session");
        assert!(wm.active_pane().status_message().is_some_and(|m| m.contains("Created new session")));
        assert_eq!(wm.selected_drawer_pane_id(), Some(wm.active_pane_id()));
    });
}

#[test]
fn keys_n_c_plus_w_create_pane() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(2);
        for (i, c) in ['n', 'c', '+', 'w'].into_iter().enumerate() {
            ch(&mut wm, c);
            assert_eq!(wm.pane_count(), 3 + i, "key {c:?} adds one pane");
            assert_eq!(wm.selected_drawer_pane_id(), Some(wm.active_pane_id()));
        }
    });
}

#[test]
fn keys_r_opens_rename_prompt_and_rename_applies() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(2);
        ch(&mut wm, 'r');
        assert!(!wm.drawer().focused, "the prompt takes the keyboard");
        assert_eq!(wm.active_pane().prompt().text(), ":drawer rename ");

        wm.drawer_mut().focused = true;
        wm.active_pane_mut().prompt_mut().set_text("");
        run_colon(&mut wm, ":drawer rename");
        assert!(!wm.drawer().focused);
        assert_eq!(wm.active_pane().prompt().text(), ":drawer rename ");

        wm.drawer_mut().selected_index = 0;
        run_colon(&mut wm, ":drawer rename newname");
        assert_eq!(wm.config().history_session, "newname");
    });
}

#[test]
fn keys_x_and_delete_close_selected_pane() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(4);
        wm.drawer_mut().selected_index = 4;
        ch(&mut wm, 'x');
        assert_eq!(wm.pane_count(), 3);
        wm.drawer_mut().selected_index = 3;
        press(&mut wm, KeyCode::Delete);
        assert_eq!(wm.pane_count(), 2);
        wm.drawer_mut().selected_index = 2;
        run_colon(&mut wm, ":drawer close-pane");
        assert_eq!(wm.pane_count(), 1);
    });
}

#[test]
fn keys_colon_close_keeps_minimize_meaning() {
    with_temp_xdg(|| {
        let parsed = parse_colon_command(":drawer close").unwrap().unwrap();
        assert_eq!(parsed, Action::DrawerSize(0));
        let (mut wm, _a) = focused_navigator(3);
        wm.apply(parsed).unwrap();
        assert_eq!(wm.pane_count(), 3, "minimize closes no pane");
    });
}

#[test]
fn keys_pageup_pagedown_scroll_and_braces_resize() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(8);
        wm.drawer_mut().selected_index = 0;
        press(&mut wm, KeyCode::PageDown);
        assert_eq!(wm.drawer().selected_index, 5);
        press(&mut wm, KeyCode::PageDown);
        assert_eq!(wm.drawer().selected_index, 8, "clamped to the last row");
        press(&mut wm, KeyCode::PageUp);
        assert_eq!(wm.drawer().selected_index, 3);
        run_colon(&mut wm, ":drawer scroll -2");
        assert_eq!(wm.drawer().selected_index, 1);
        run_colon(&mut wm, ":drawer scroll 5");
        assert_eq!(wm.drawer().selected_index, 6);

        let classic_width = wm.drawer_config().width_cols;
        ch(&mut wm, '}');
        assert_eq!(wm.drawer_config().navigator_width_cols, 30);
        ch(&mut wm, '{');
        ch(&mut wm, '{');
        assert_eq!(wm.drawer_config().navigator_width_cols, 26);
        run_colon(&mut wm, ":drawer widen 2");
        assert_eq!(wm.drawer_config().navigator_width_cols, 28);
        run_colon(&mut wm, ":drawer widen -500");
        assert_eq!(wm.drawer_config().navigator_width_cols, 16, "clamped");
        assert_eq!(wm.drawer_config().width_cols, classic_width);
    });
}

#[test]
fn keys_unclaimed_keys_fall_through_to_classic() {
    use bilterm::wm::DrawerVisibilityMode;
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(2);
        let edge = wm.drawer_config().edge;
        ch(&mut wm, 'd');
        assert_ne!(wm.drawer_config().edge, edge, "d cycles the edge");
        ch(&mut wm, '*');
        assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed, "* pins");
        ch(&mut wm, ']');
        assert!(wm.active_pane().status_message().is_some_and(|m| m.starts_with("Panel scale")));
        ch(&mut wm, 'q');
        assert!(!wm.drawer().focused, "q leaves the drawer");
    });
}

/// Observable drawer/pane/session state used to compare two runs.
fn observe(wm: &WindowManager) -> String {
    let rows: Vec<String> = wm
        .drawer_rows()
        .iter()
        .map(|r| match &r.kind {
            DrawerRowKind::SessionHeader(_) => match r.navigator.as_ref() {
                Some(m) => format!("S(d{} c{})", m.depth, m.collapsed),
                None => "S".to_string(),
            },
            k => format!("{k:?}"),
        })
        .collect();
    format!(
        "sel={} active={} panes={} focused={} comms={:?} nav_w={} w={} prompt={:?} rows={rows:?}",
        wm.drawer().selected_index,
        wm.active_pane_id(),
        wm.pane_count(),
        wm.drawer().focused,
        (wm.agents_window().active, wm.drawer().navigator.comms_focus),
        wm.drawer_config().navigator_width_cols,
        wm.drawer_config().width_cols,
        wm.active_pane().prompt().text(),
    )
}

#[test]
fn keys_e_and_bang_unclaimed_by_classic() {
    with_temp_xdg(|| {
        let (mut wm, _a) = fixture(100, 30);
        open_drawer(&mut wm);
        wm.drawer_mut().selected_index = 0;
        with_states(&mut wm, &[(2, Some(AgentState::Blocked))]);
        let before = observe(&wm);
        ch(&mut wm, 'e');
        ch(&mut wm, '!');
        assert_eq!(observe(&wm), before);
        assert_eq!(wm.drawer_config().style, DrawerStyle::Classic);
    });
}

/// The `:drawer` colon form of an op, written as an exhaustive match (no wildcard): a new variant
/// fails to compile until it is listed here.
fn drawer_nav_colon_form(op: &DrawerNavOp) -> String {
    match op {
        DrawerNavOp::NextAttention => ":drawer attention".into(),
        DrawerNavOp::Fold => ":drawer fold".into(),
        DrawerNavOp::Activate => ":drawer enter".into(),
        DrawerNavOp::Next => ":drawer next".into(),
        DrawerNavOp::Prev => ":drawer prev".into(),
        DrawerNavOp::Row(i) => format!(":drawer row {}", i + 1),
        DrawerNavOp::Scroll(n) => format!(":drawer scroll {n}"),
        DrawerNavOp::Widen(n) => format!(":drawer widen {n}"),
        DrawerNavOp::NewSession => ":drawer new-session".into(),
        DrawerNavOp::NewPane => ":drawer new-pane".into(),
        DrawerNavOp::RenamePrompt => ":drawer rename".into(),
        DrawerNavOp::Rename(name) => format!(":drawer rename {name}"),
        DrawerNavOp::Close => ":drawer close-pane".into(),
        DrawerNavOp::ExpandAll => ":drawer expand all".into(),
        DrawerNavOp::CollapseAll => ":drawer collapse all".into(),
        DrawerNavOp::AddWindow(None) => ":drawer add window".into(),
        DrawerNavOp::AddWindow(Some(session)) => format!(":drawer add window {session}"),
        DrawerNavOp::AddPane(SplitDirection::Horizontal) => ":drawer add pane h".into(),
        DrawerNavOp::AddPane(SplitDirection::Vertical) => ":drawer add pane v".into(),
        DrawerNavOp::MenuOpen(None) => ":drawer menu".into(),
        DrawerNavOp::MenuOpen(Some((x, y))) => format!(":drawer menu {x} {y}"),
        DrawerNavOp::MenuClose => ":drawer menu close".into(),
        DrawerNavOp::MenuPick(i) => format!(":drawer menu pick {}", i + 1),
        DrawerNavOp::OpenComms => ":drawer comms".into(),
        DrawerNavOp::CommsFocus(true) => ":drawer comms-focus on".into(),
        DrawerNavOp::CommsFocus(false) => ":drawer comms-focus off".into(),
    }
}

/// One representative of every `DrawerNavOp` variant.
fn all_nav_ops() -> Vec<DrawerNavOp> {
    vec![
        DrawerNavOp::NextAttention,
        DrawerNavOp::Fold,
        DrawerNavOp::Activate,
        DrawerNavOp::Next,
        DrawerNavOp::Prev,
        DrawerNavOp::Row(2),
        DrawerNavOp::Scroll(-5),
        DrawerNavOp::Scroll(5),
        DrawerNavOp::Widen(-2),
        DrawerNavOp::Widen(2),
        DrawerNavOp::NewSession,
        DrawerNavOp::NewPane,
        DrawerNavOp::RenamePrompt,
        DrawerNavOp::Rename("renamed".into()),
        DrawerNavOp::Close,
        DrawerNavOp::ExpandAll,
        DrawerNavOp::CollapseAll,
        DrawerNavOp::AddWindow(None),
        DrawerNavOp::AddWindow(Some("other".into())),
        DrawerNavOp::AddPane(SplitDirection::Horizontal),
        DrawerNavOp::AddPane(SplitDirection::Vertical),
        DrawerNavOp::MenuOpen(None),
        DrawerNavOp::MenuOpen(Some((4, 7))),
        DrawerNavOp::MenuClose,
        DrawerNavOp::MenuPick(1),
        DrawerNavOp::OpenComms,
        DrawerNavOp::CommsFocus(true),
        DrawerNavOp::CommsFocus(false),
    ]
}

#[test]
fn keys_colon_equivalents_match_keys() {
    for (scope, mods, code, action) in NAVIGATOR_DRAWER_BINDINGS {
        let Action::DrawerNav(op) = action else { panic!("navigator row {code:?} must lower to DrawerNav") };
        let (mods, code, form, scope) = (*mods, *code, drawer_nav_colon_form(op), *scope);
        // Rows of the comms scope only apply while the selected row's glyph has focus.
        let setup = |wm: &mut WindowManager| {
            with_states(wm, &[(3, Some(AgentState::Blocked))]);
            if scope == bilterm::input::keymap::DrawerKeyScope::NavigatorComms {
                wm.record_traffic(TrafficEndpoint::User, TrafficEndpoint::Pane(1), TrafficKind::Broadcast, "x", now());
                wm.update_agent_states(now());
                run_colon(wm, ":drawer comms-focus on");
                assert!(wm.drawer().navigator.comms_focus.is_some(), "setup must focus the glyph");
            }
        };
        let by_key = with_temp_xdg(|| {
            let (mut wm, _a) = focused_navigator(4);
            setup(&mut wm);
            press_mod(&mut wm, code, mods);
            observe(&wm)
        });
        let by_colon = with_temp_xdg(|| {
            let (mut wm, _a) = focused_navigator(4);
            setup(&mut wm);
            run_colon(&mut wm, &form);
            observe(&wm)
        });
        assert_eq!(by_key, by_colon, "key {code:?} vs `{form}`");
    }
}

#[test]
fn keymap_navigator_drawer_keys_unique() {
    let mut seen = Vec::new();
    for (scope, mods, code, _) in NAVIGATOR_DRAWER_BINDINGS {
        let triple = (*scope, *mods, *code);
        assert!(!seen.contains(&triple), "duplicate navigator drawer chord {triple:?}");
        seen.push(triple);
    }
}

#[test]
fn keymap_navigator_drawer_keys_do_not_shadow_globals() {
    for (_, mods, code, _) in NAVIGATOR_DRAWER_BINDINGS {
        let key = KeyEvent::new(*code, *mods);
        assert!(
            default_global_action(key, false).is_none(),
            "navigator row {code:?}+{mods:?} shadows a DEFAULT_GLOBAL_BINDINGS chord"
        );
        assert!(
            !DEFAULT_GLOBAL_BINDINGS.iter().any(|(m, c, _)| c == code && *m == *mods),
            "navigator row {code:?}+{mods:?} is bound in DEFAULT_GLOBAL_BINDINGS"
        );
    }
}

#[test]
fn drawer_nav_ops_have_colon_completion_and_help() {
    let drawer_row = bilterm::pane::completion::colon_command_table()
        .iter()
        .find(|(c, _)| *c == ":drawer")
        .expect(":drawer completion row")
        .1;
    let sampled = all_nav_ops();
    for op in &sampled {
        let form = drawer_nav_colon_form(op);
        let parsed = parse_colon_command(&form).unwrap().unwrap_or_else(|e| panic!("`{form}`: {e}"));
        assert_eq!(parsed, Action::DrawerNav(op.clone()), "`{form}` must parse to its op");
        let verb = form.split_whitespace().nth(1).unwrap();
        assert!(
            drawer_row.iter().any(|s| s.split_whitespace().next() == Some(verb)),
            "verb `{verb}` missing from the :drawer completion row"
        );
        let prefix = format!(":drawer {verb}");
        assert!(
            bilterm::ui::modals::HELP_LINES.iter().any(|(c, _)| c.starts_with(&prefix)),
            "no HELP_LINES row for `{prefix}`"
        );
    }
    for (_, _, _, action) in NAVIGATOR_DRAWER_BINDINGS {
        let Action::DrawerNav(op) = action else { panic!("{action:?} is not a DrawerNav action") };
        assert!(sampled.contains(op), "table op {op:?} is not covered by the sampled ops");
    }
}

// ---- Task 3: pin/scale with the navigator ----

fn gui_frame(wm: &mut WindowManager, area: Rect) -> bilterm::ui::renderer::GuiFrameSurface {
    use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
    wm.set_gui_surface(true);
    let mut renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    renderer.render_workspace_with_cell_size(wm, area, 8, 16, &mut surface);
    surface
}

#[test]
fn pin_scale_fixed_keeps_navigator_visible_and_reflows() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        open_drawer(&mut wm);
        run_colon(&mut wm, ":drawer pin");
        press(&mut wm, KeyCode::Esc);
        assert!(!wm.drawer().focused, "Esc hands the keyboard back");
        assert!(wm.is_drawer_effectively_visible(), "a pinned navigator drawer stays open");
        assert_eq!(wm.drawer_rect(area).unwrap().width, 28);
        assert_eq!(wm.effective_workspace_area(area), Rect::new(28, 0, 72, 30), "Push layout reflows the panes");
        let lines = buffer_lines(&render(&wm, area));
        let title = line_of(&lines, "[1] alpha");
        let col = title.char_indices().position(|(i, _)| title[i..].starts_with("[1] alpha")).unwrap();
        assert!(col > 28, "pane windows start right of the 28-col drawer, title at column {col}");
    });
}

#[test]
fn pin_scale_gui_scale_enlarges_navigator_glyphs() {
    with_temp_xdg(|| {
        let max_width = |s: &bilterm::ui::renderer::GuiFrameSurface| s.glyphs.iter().map(|g| g.cell_width_px).max().unwrap();
        let (mut wm, area) = navigator_wm(100, 30);
        wm.toggle_drawer();
        let base = max_width(&gui_frame(&mut wm, area));

        run_colon(&mut wm, ":drawer scale 150");
        let by_command = max_width(&gui_frame(&mut wm, area));
        assert!(by_command > base, ":drawer scale 150 widens the navigator glyphs");

        run_colon(&mut wm, ":drawer scale 100");
        wm.drawer_mut().focused = true;
        for _ in 0..5 {
            ch(&mut wm, ']'); // unclaimed by the navigator table: falls through to the classic handler
        }
        assert_eq!(wm.scale_setting(bilterm::ui::typography::PanelTarget::Drawer), 150);
        assert!(max_width(&gui_frame(&mut wm, area)) > base, "] steps the scale in navigator style too");
    });
}

// ---- Task 4: expand all / collapse all ----

/// Three workspaces: main > build > default (active, with panes) and a saved root "archive".
fn folded_forest(wm: &mut WindowManager) {
    nested_tree(wm);
    wm.session_tree_mut().upsert_session("archive", bilterm::wm::SessionNodeKind::Independent, None);
    wm.session_tree_mut().branch("archive", "old").unwrap();
    for name in ["main", "build", "archive"] {
        wm.session_tree_mut().node_mut(name).unwrap().collapsed = true;
    }
}

fn kinds(wm: &WindowManager) -> Vec<DrawerRowKind> {
    wm.drawer_rows().into_iter().map(|r| r.kind).collect()
}

#[test]
fn expand_all_unfolds_every_workspace_and_session() {
    with_temp_xdg(|| {
        let (mut wm, _a) = navigator_wm(100, 30);
        folded_forest(&mut wm);
        assert_eq!(kinds(&wm), vec![session("main"), session("archive")], "folded down to workspaces");
        wm.drawer_mut().selected_index = 1;
        wm.apply(Action::DrawerNav(DrawerNavOp::ExpandAll)).unwrap();
        let k = kinds(&wm);
        for want in [session("main"), session("build"), session("default"), DrawerRowKind::Pane(1), DrawerRowKind::Pane(2), session("archive"), session("old")] {
            assert!(k.contains(&want), "{want:?} missing after expand all: {k:?}");
        }
        assert_eq!(k[wm.drawer().selected_index], session("archive"), "selection stays on the same logical row");
    });
}

#[test]
fn collapse_all_folds_every_tree_to_workspace_rows() {
    with_temp_xdg(|| {
        let (mut wm, _a) = navigator_wm(100, 30);
        nested_tree(&mut wm);
        wm.session_tree_mut().upsert_session("archive", bilterm::wm::SessionNodeKind::Independent, None);
        wm.session_tree_mut().branch("archive", "old").unwrap();
        let k = kinds(&wm);
        let pane1 = k.iter().position(|x| *x == DrawerRowKind::Pane(1)).unwrap();
        wm.drawer_mut().selected_index = pane1;
        wm.apply(Action::DrawerNav(DrawerNavOp::CollapseAll)).unwrap();
        assert_eq!(kinds(&wm), vec![session("main"), session("archive")]);
        assert_eq!(wm.drawer().selected_index, 0, "selection moves to the pane's workspace (main)");
    });
}

#[test]
fn collapse_all_on_single_workspace_hides_panes_and_toggles_back() {
    with_temp_xdg(|| {
        let (mut wm, _a) = navigator_wm(100, 30);
        open_drawer(&mut wm);
        wm.drawer_mut().selected_index = 2;
        wm.apply(Action::DrawerNav(DrawerNavOp::CollapseAll)).unwrap();
        assert_eq!(kinds(&wm), vec![session("default")]);
        assert_eq!(wm.drawer().selected_index, 0);
        wm.apply(Action::DrawerNav(DrawerNavOp::CollapseAll)).unwrap();
        assert_eq!(pane_row_count(&wm), 2, "collapsing a fully collapsed tree expands it");
    });
}

#[test]
fn keys_e_upper_expands_all_and_c_upper_collapses_all() {
    with_temp_xdg(|| {
        let rows: Vec<_> = NAVIGATOR_DRAWER_BINDINGS
            .iter()
            .filter(|(_, m, c, _)| *m == KeyModifiers::NONE && matches!(c, KeyCode::Char('E' | 'C')))
            .collect();
        assert_eq!(rows.len(), 2, "E and C are rows of the navigator key table");
        let (mut wm, _a) = navigator_wm(100, 30);
        folded_forest(&mut wm);
        open_drawer(&mut wm);
        // Shifted letters arrive with SHIFT set; the table lookup normalises it away.
        press_mod(&mut wm, KeyCode::Char('E'), KeyModifiers::SHIFT);
        assert!(pane_row_count(&wm) == 2 && kinds(&wm).contains(&session("old")));
        press_mod(&mut wm, KeyCode::Char('C'), KeyModifiers::SHIFT);
        assert_eq!(kinds(&wm), vec![session("main"), session("archive")]);
        press_mod(&mut wm, KeyCode::Char('C'), KeyModifiers::SHIFT);
        assert!(pane_row_count(&wm) == 2, "second C on a fully collapsed tree expands all");
    });
}

#[test]
fn colon_navigator_and_drawer_expand_collapse_all_match() {
    for (cmd, op) in [
        (":navigator expand all", DrawerNavOp::ExpandAll),
        (":navigator collapse all", DrawerNavOp::CollapseAll),
        (":drawer expand all", DrawerNavOp::ExpandAll),
        (":drawer collapse all", DrawerNavOp::CollapseAll),
    ] {
        assert_eq!(parse_colon_command(cmd).unwrap().unwrap(), Action::DrawerNav(op), "{cmd}");
    }
    assert!(parse_colon_command(":drawer collapse").unwrap().is_err(), "collapse needs `all`");
    assert!(parse_colon_command(":navigator collapse now").unwrap().is_err());
    // `:navigator` forwards everything else to the :drawer parser, including `style`.
    assert_eq!(
        parse_colon_command(":navigator style classic").unwrap().unwrap(),
        parse_colon_command(":drawer style classic").unwrap().unwrap()
    );
    // The older `:drawer expand` (maximize) alias keeps its meaning.
    assert_eq!(parse_colon_command(":drawer expand").unwrap().unwrap(), Action::DrawerSize(u16::MAX));
}

#[test]
fn expand_collapse_all_have_completion_and_help() {
    let table = bilterm::pane::completion::colon_command_table();
    for head in [":drawer", ":navigator"] {
        let row = table.iter().find(|(c, _)| *c == head).unwrap_or_else(|| panic!("{head} completion row")).1;
        assert!(row.contains(&"expand all") && row.contains(&"collapse all"), "{head}: {row:?}");
    }
    let help = bilterm::ui::modals::HELP_LINES;
    let palette = bilterm::ui::modals::PALETTE_CATALOG;
    for cmd in [":drawer expand all", ":drawer collapse all", ":navigator expand all", ":navigator collapse all"] {
        assert!(help.iter().any(|(c, _)| *c == cmd), "help row for {cmd}");
        assert!(palette.iter().any(|e| e.0 == cmd), "palette row for {cmd}");
    }
}

#[test]
fn classic_unaffected_by_expand_collapse_all() {
    with_temp_xdg(|| {
        let (mut wm, _a) = fixture(100, 30);
        open_drawer(&mut wm);
        wm.drawer_mut().selected_index = 0;
        let before = observe(&wm);
        ch(&mut wm, 'E');
        ch(&mut wm, 'C');
        assert_eq!(observe(&wm), before);
    });
}

// ---- Task 5: add a window/pane to the selected session ----

/// Active session "default" (panes 1 and 2) plus a saved second session "other" (1 pane).
fn two_sessions() -> (WindowManager, Rect) {
    let (mut wm, area) = navigator_wm(100, 30);
    wm.create_new_session(Some("other"), false).unwrap();
    wm.switch_or_create_session("default").unwrap();
    wm.resize_all(area).unwrap();
    wm.focus_and_raise(1);
    open_drawer(&mut wm);
    (wm, area)
}

fn row_of(wm: &WindowManager, kind: &DrawerRowKind) -> usize {
    kinds(wm).iter().position(|k| k == kind).unwrap_or_else(|| panic!("{kind:?} not listed: {:?}", kinds(wm)))
}

#[test]
fn add_window_targets_selected_session() {
    with_temp_xdg(|| {
        let (mut wm, _a) = two_sessions();
        assert_eq!(wm.config().history_session, "default");
        assert_eq!(wm.pane_count(), 2);

        // Session row of another session: the window lands in that session, not the focused one.
        wm.drawer_mut().selected_index = row_of(&wm, &session("other"));
        wm.apply(Action::DrawerNav(DrawerNavOp::AddWindow(None))).unwrap();
        assert_eq!(wm.config().history_session, "other");
        assert_eq!(wm.pane_count(), 2, "other had one pane, now two");
        assert_eq!(wm.selected_drawer_pane_id(), Some(wm.active_pane_id()), "new window focused and selected");

        // Named session, given explicitly while the selection is elsewhere.
        wm.apply(Action::DrawerNav(DrawerNavOp::AddWindow(Some("default".into())))).unwrap();
        assert_eq!(wm.config().history_session, "default");
        assert_eq!(wm.pane_count(), 3);

        // Unknown session: refused with a status message, nothing created.
        wm.apply(Action::DrawerNav(DrawerNavOp::AddWindow(Some("nope".into())))).unwrap();
        assert_eq!((wm.config().history_session.as_str(), wm.pane_count()), ("default", 3));
        assert!(wm.active_pane().status_message().is_some_and(|m| m.contains("No such session")));

        // Pane row selected: that pane is split even though pane 1 holds the focus.
        wm.focus_and_raise(1);
        wm.drawer_mut().selected_index = row_of(&wm, &DrawerRowKind::Pane(2));
        let before = wm.ordered_pane_ids();
        wm.apply(Action::DrawerNav(DrawerNavOp::AddWindow(None))).unwrap();
        let after = wm.ordered_pane_ids();
        assert_eq!(after.len(), before.len() + 1);
        let new = wm.active_pane_id();
        let pos2 = after.iter().position(|&i| i == 2).unwrap();
        assert_eq!(after[pos2 + 1], new, "the new window sits next to the selected pane #2: {after:?}");
    });
}

#[test]
fn add_pane_splits_selected_row() {
    with_temp_xdg(|| {
        for dir in [SplitDirection::Horizontal, SplitDirection::Vertical] {
            let (mut wm, _a) = focused_navigator(2);
            wm.focus_and_raise(2);
            wm.drawer_mut().selected_index = row_of(&wm, &DrawerRowKind::Pane(1));
            wm.apply(Action::DrawerNav(DrawerNavOp::AddPane(dir))).unwrap();
            let ids = wm.ordered_pane_ids();
            assert_eq!(ids.len(), 3);
            let new = wm.active_pane_id();
            let p1 = ids.iter().position(|&i| i == 1).unwrap();
            assert_eq!(ids[p1 + 1], new, "split of pane #1 ({dir:?}): {ids:?}");
        }
    });
}

#[test]
fn key_a_and_colon_twins_match() {
    let observe_with = |f: &dyn Fn(&mut WindowManager)| {
        with_temp_xdg(|| {
            let (mut wm, _a) = two_sessions();
            wm.drawer_mut().selected_index = row_of(&wm, &session("other"));
            f(&mut wm);
            observe(&wm)
        })
    };
    let window_by_key = observe_with(&|wm| ch(wm, 'a'));
    assert_eq!(window_by_key, observe_with(&|wm| run_colon(wm, ":drawer add window")));
    assert_eq!(window_by_key, observe_with(&|wm| run_colon(wm, ":navigator add window")));
    assert_eq!(window_by_key, observe_with(&|wm| run_colon(wm, ":navigator add window other")));

    let pane_by_key = observe_with(&|wm| press_mod(wm, KeyCode::Char('A'), KeyModifiers::SHIFT));
    assert_eq!(pane_by_key, observe_with(&|wm| run_colon(wm, ":drawer add pane")));
    assert_eq!(pane_by_key, observe_with(&|wm| run_colon(wm, ":navigator add pane h")));

    for bad in [":drawer add", ":navigator add tab", ":drawer add pane x", ":drawer add window a b"] {
        assert!(parse_colon_command(bad).unwrap().is_err(), "{bad} must be a usage error");
    }
    let table = bilterm::pane::completion::colon_command_table();
    for head in [":drawer", ":navigator"] {
        let row = table.iter().find(|(c, _)| *c == head).unwrap().1;
        assert!(row.contains(&"add window") && row.contains(&"add pane"), "{head}: {row:?}");
    }
    for cmd in [":drawer add window", ":drawer add pane", ":navigator add window", ":navigator add pane"] {
        assert!(bilterm::ui::modals::HELP_LINES.iter().any(|(c, _)| c.starts_with(cmd)), "help row for {cmd}");
        assert!(bilterm::ui::modals::PALETTE_CATALOG.iter().any(|e| e.0.starts_with(cmd)), "palette row for {cmd}");
    }
}

#[test]
fn add_classic_unaffected() {
    with_temp_xdg(|| {
        let (mut wm, _a) = fixture(100, 30);
        open_drawer(&mut wm);
        let panes = wm.pane_count();
        ch(&mut wm, 'a');
        press_mod(&mut wm, KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(wm.pane_count(), panes, "classic a / A keep arranging windows and add none");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Classic);
    });
}

// ---- Phase 50-15: navigator mouse parity (FLT-01, success criteria 3 and 4) ----

use bilterm::input::handle_wm_mouse_event;
use bilterm::ui::drawer::navigator::navigator_row_at;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

fn mouse_ev(kind: MouseEventKind, col: u16, row: u16, mods: KeyModifiers) -> MouseEvent {
    MouseEvent { kind, column: col, row, modifiers: mods }
}

fn click_at(wm: &mut WindowManager, area: Rect, col: u16, row: u16) {
    for kind in [MouseEventKind::Down(MouseButton::Left), MouseEventKind::Up(MouseButton::Left)] {
        handle_wm_mouse_event(wm, mouse_ev(kind, col, row, KeyModifiers::NONE), area).unwrap();
    }
}

/// Screen cell `(x, y)` of the first buffer line containing `needle`.
fn find_cell(wm: &WindowManager, area: Rect, needle: &str) -> (u16, u16) {
    let lines = buffer_lines(&render(wm, area));
    for (y, l) in lines.iter().enumerate() {
        if let Some(byte) = l.find(needle) {
            return (l[..byte].chars().count() as u16, y as u16);
        }
    }
    panic!("`{needle}` not rendered:\n{}", lines.join("\n"));
}

#[test]
fn mouse_tracer_click_row_focuses_pane() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        wm.toggle_drawer();
        wm.focus_and_raise(1);
        let (x, y) = find_cell(&wm, area, "#2 beta");
        click_at(&mut wm, area, x, y);
        assert_eq!(wm.active_pane_id(), 2);
        let idx = navigator_row_at(&wm, area, x, y).expect("row under the click");
        assert_eq!(wm.drawer_rows()[idx].kind, DrawerRowKind::Pane(2));
        assert_eq!(wm.drawer().selected_index, idx);
        let buf = render(&wm, area);
        assert!(
            buf[(x, y)].modifier.contains(ratatui::style::Modifier::REVERSED)
                || buf[(x, y)].bg == ratatui::style::Color::Rgb(30, 41, 59),
            "the #2 row carries the selection style"
        );
        assert_eq!(navigator_row_at(&wm, area, 99, 29), None, "outside the drawer is no row");
    });
}

use bilterm::ui::drawer::navigator::{
    NavigatorButtonKind, navigator_fold_marker_cells, navigator_footer_buttons,
};
use bilterm::wm::DrawerVisibilityMode;

fn open_navigator(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let (mut wm, area) = navigator_wm(cols, rows);
    wm.toggle_drawer();
    wm.focus_and_raise(1);
    wm.drawer_mut().selected_index = 1;
    (wm, area)
}

fn click_button(wm: &mut WindowManager, area: Rect, kind: NavigatorButtonKind) {
    let b = navigator_footer_buttons(wm, area)
        .into_iter()
        .find(|b| b.kind == kind)
        .unwrap_or_else(|| panic!("{kind:?} button not laid out"));
    click_at(wm, area, b.x, b.y);
}

fn click_row(wm: &mut WindowManager, area: Rect, kind: &DrawerRowKind) {
    let idx = row_of(wm, kind);
    let (x, y) = (0..area.width)
        .flat_map(|x| (0..area.height).map(move |y| (x, y)))
        .find(|&(x, y)| navigator_row_at(wm, area, x, y) == Some(idx))
        .expect("row has a cell");
    click_at(wm, area, x + 2, y);
}

#[test]
fn mouse_fold_marker_click_folds_and_expands() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        assert_eq!(pane_row_count(&wm), 2);
        let cells = navigator_fold_marker_cells(&wm, area);
        assert_eq!(cells.len(), 1, "only the workspace row has children: {cells:?}");
        click_at(&mut wm, area, cells[0].x, cells[0].y);
        assert_eq!(pane_row_count(&wm), 0, "marker click hides the pane rows");
        click_at(&mut wm, area, cells[0].x, cells[0].y);
        assert_eq!(pane_row_count(&wm), 2, "second click expands");

        let (mut key_wm, _a) = open_navigator(100, 30);
        key_wm.drawer_mut().focused = true;
        key_wm.drawer_mut().selected_index = 0;
        ch(&mut key_wm, 'e');
        let (mut mouse_wm, area) = open_navigator(100, 30);
        let c = navigator_fold_marker_cells(&mouse_wm, area)[0];
        click_at(&mut mouse_wm, area, c.x, c.y);
        assert_eq!(kinds(&key_wm), kinds(&mouse_wm), "marker click equals `e` on that row");
    });
}

#[test]
fn mouse_footer_buttons_render_in_both_glyph_packs() {
    with_temp_xdg(|| {
        for pack in ["unicode", "ascii"] {
            let (mut wm, area) = navigator_wm(100, 30);
            run_colon(&mut wm, &format!(":set glyphs {pack}"));
            run_colon(&mut wm, ":drawer size 26");
            wm.toggle_drawer();
            let buttons = navigator_footer_buttons(&wm, area);
            assert_eq!(buttons.len(), 7, "{pack}: all seven buttons fit 26 columns");
            let buf = render(&wm, area);
            let labels: String = buttons.iter().map(|b| buf[(b.x, b.y)].symbol().to_string()).collect();
            assert_eq!(labels, "*!N+rx@", "{pack}");
            let y = buttons[0].y;
            assert!(buttons.iter().all(|b| b.y == y && b.width == 2));
            let dr = wm.drawer_rect(area).unwrap();
            assert_eq!(y, dr.y + dr.height - 2, "footer row is the last inner row");
        }
    });
}

#[test]
fn mouse_footer_attention_button_jumps() {
    with_temp_xdg(|| {
        let (mut wm, area) = three_pane_navigator(100, 30);
        wm.toggle_drawer();
        wm.focus_and_raise(1);
        with_states(&mut wm, &[(3, Some(AgentState::Blocked))]);
        click_button(&mut wm, area, NavigatorButtonKind::Attention);
        assert_eq!(wm.active_pane_id(), 3);
    });
}

#[test]
fn mouse_footer_new_session_and_new_pane_buttons_create() {
    with_temp_xdg(|| {
        let observe_with = |f: &dyn Fn(&mut WindowManager, Rect)| {
            let (mut wm, area) = open_navigator(100, 30);
            f(&mut wm, area);
            (observe(&wm), wm.active_pane().status_message().map(str::to_string))
        };
        let by_key = observe_with(&|wm, _| {
            wm.drawer_mut().focused = true;
            ch(wm, 'N');
        });
        let by_mouse = observe_with(&|wm, a| click_button(wm, a, NavigatorButtonKind::NewSession));
        assert_eq!(by_key.0.replace("focused=false", "focused=true"), by_mouse.0.replace("focused=false", "focused=true"));
        assert_eq!(by_key.1, by_mouse.1);
        assert!(by_mouse.1.is_some_and(|m| m.starts_with("Created new session")));

        let (mut wm, area) = open_navigator(100, 30);
        let before = wm.pane_count();
        click_button(&mut wm, area, NavigatorButtonKind::NewPane);
        assert_eq!(wm.pane_count(), before + 1);
        assert!(wm.active_pane().status_message().is_some_and(|m| m.starts_with("Created new window")));
    });
}

#[test]
fn mouse_footer_rename_button_opens_prompt() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        wm.drawer_mut().focused = true;
        click_button(&mut wm, area, NavigatorButtonKind::Rename);
        assert_eq!(wm.active_pane().prompt().text(), ":drawer rename ");
        assert!(!wm.drawer().focused, "the prompt takes the keyboard");
    });
}

#[test]
fn mouse_footer_close_button_closes_selected_pane() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        click_row(&mut wm, area, &DrawerRowKind::Pane(2));
        assert_eq!(wm.pane_count(), 2);
        click_button(&mut wm, area, NavigatorButtonKind::Close);
        assert_eq!(wm.pane_count(), 1);
        assert!(wm.pane(2).is_none());
    });
}

#[test]
fn mouse_footer_pin_button_toggles_fixed() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        assert_ne!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);
        click_button(&mut wm, area, NavigatorButtonKind::Pin);
        assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::Fixed);
        click_button(&mut wm, area, NavigatorButtonKind::Pin);
        assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
    });
}

#[test]
fn mouse_footer_zoom_button_opens_agents_window_on_selected_pane() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        click_row(&mut wm, area, &DrawerRowKind::Pane(2));
        assert!(!wm.agents_window().active);
        click_button(&mut wm, area, NavigatorButtonKind::Zoom);
        assert!(wm.agents_window().active);
        assert_eq!(wm.agents_window().selection.node.as_ref().map(|k| k.0.as_str()), Some("pane:2"));
    });
}

#[test]
fn mouse_footer_narrow_drops_buttons_from_the_right() {
    with_temp_xdg(|| {
        // A 24-column workspace caps the drawer at half the width (12 columns), an inner width of 10.
        let (mut wm, area) = navigator_wm(24, 30);
        wm.toggle_drawer();
        let dr = wm.drawer_rect(area).unwrap();
        assert_eq!(dr.width - 2, 10, "inner width 10");
        let buttons = navigator_footer_buttons(&wm, area);
        let kinds: Vec<_> = buttons.iter().map(|b| b.kind).collect();
        assert_eq!(
            kinds,
            [NavigatorButtonKind::Pin, NavigatorButtonKind::Attention, NavigatorButtonKind::NewSession, NavigatorButtonKind::NewPane]
        );
        let buf = render(&wm, area);
        let labels: String = buttons.iter().map(|b| buf[(b.x, b.y)].symbol().to_string()).collect();
        assert_eq!(labels, "*!N+");
        let footer = buttons[0].y;
        let row: String = (dr.x + 1..dr.x + dr.width - 1).map(|x| buf[(x, footer)].symbol().to_string()).collect();
        assert!(!row.contains('@') && !row.contains('x'), "dropped buttons are not rendered: {row:?}");
    });
}

#[test]
fn mouse_non_button_cells_create_nothing() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        wm.drawer_mut().focused = true;
        let dr = wm.drawer_rect(area).unwrap();
        let buttons = navigator_footer_buttons(&wm, area);
        let footer = buttons[0].y;
        let markers = navigator_fold_marker_cells(&wm, area);
        let plus = bilterm::ui::drawer::navigator::navigator_plus_cells(&wm, area);
        let before = observe(&wm);
        let vis = wm.drawer_config().visibility;
        for x in dr.x + 1..dr.x + dr.width - 1 {
            let on_button = buttons.iter().any(|b| x >= b.x && x < b.x + b.width);
            if !on_button {
                click_at(&mut wm, area, x, footer);
            }
            let on_marker = markers.iter().any(|m| m.x == x && m.y == dr.y + 1)
                || plus.iter().any(|c| x >= c.x && x < c.x + 3 && c.y == dr.y + 1);
            if !on_marker {
                // Header-ish first inner row: only the row label itself selects; nothing is created.
                let (panes, sessions) = (wm.pane_count(), wm.list_sessions().len());
                click_at(&mut wm, area, x, dr.y + 1);
                assert_eq!((wm.pane_count(), wm.list_sessions().len()), (panes, sessions));
            }
        }
        assert_eq!(wm.pane_count(), 2);
        assert_eq!(wm.drawer_config().visibility, vis);
        assert_eq!(wm.active_pane().prompt().text(), "");
        assert_eq!(pane_row_count(&wm), 2, "no fold happened");
        assert_eq!(wm.drawer_config().navigator_width_cols, 28);
        let _ = before;
    });
}

fn wheel(wm: &mut WindowManager, area: Rect, down: bool, mods: KeyModifiers, col: u16, row: u16) {
    let kind = if down { MouseEventKind::ScrollDown } else { MouseEventKind::ScrollUp };
    handle_wm_mouse_event(wm, mouse_ev(kind, col, row, mods), area).unwrap();
}

/// Title-bar badge columns `[minimize, zoom, close]` of a drawer rect (same formula as the hit-test).
fn badge_cols(wm: &WindowManager, dr: Rect) -> [u16; 3] {
    let g = wm.config().glyph_set();
    let (m, z, c) =
        (g.minimize_badge.chars().count() as u16, g.zoom_badge.chars().count() as u16, g.close_badge.chars().count() as u16);
    let start = (dr.x + dr.width).saturating_sub(m + z + c + 2);
    [start, start + m, start + m + z]
}

#[test]
fn mouse_title_badges_work_in_navigator() {
    with_temp_xdg(|| {
        for (badge, colon) in [(0usize, ":drawer close"), (1, ":drawer max")] {
            let (mut by_mouse, area) = open_navigator(100, 30);
            let dr = by_mouse.drawer_rect(area).unwrap();
            let x = badge_cols(&by_mouse, dr)[badge];
            click_at(&mut by_mouse, area, x, dr.y);
            let (mut by_colon, _a) = open_navigator(100, 30);
            run_colon(&mut by_colon, colon);
            assert_eq!(observe(&by_mouse), observe(&by_colon), "badge {badge} vs `{colon}`");
            assert_eq!(by_mouse.drawer().maximized, by_colon.drawer().maximized);
            assert_eq!(by_mouse.drawer().open, by_colon.drawer().open);
        }
        let (mut wm, area) = open_navigator(100, 30);
        run_colon(&mut wm, ":drawer fixed");
        let dr = wm.drawer_rect(area).unwrap();
        let close_x = badge_cols(&wm, dr)[2];
        click_at(&mut wm, area, close_x, dr.y);
        assert!(wm.drawer_rect(area).is_none(), "close badge hides the drawer");
        assert_eq!(wm.drawer_config().visibility, DrawerVisibilityMode::OnDemand);
    });
}

#[test]
fn mouse_wheel_scrolls_navigator_selection() {
    with_temp_xdg(|| {
        let by_wheel = |down: bool, mods: KeyModifiers| {
            let (mut wm, area) = n_pane_navigator(8, 100, 30);
            wm.toggle_drawer();
            wm.drawer_mut().selected_index = 4;
            wheel(&mut wm, area, down, mods, 3, 5);
            observe(&wm)
        };
        let by_colon = |cmd: &str| {
            let (mut wm, _a) = n_pane_navigator(8, 100, 30);
            wm.toggle_drawer();
            wm.drawer_mut().selected_index = 4;
            run_colon(&mut wm, cmd);
            observe(&wm)
        };
        assert_eq!(by_wheel(true, KeyModifiers::NONE), by_colon(":drawer scroll 3"));
        assert_eq!(by_wheel(false, KeyModifiers::NONE), by_colon(":drawer scroll -3"));
        assert_eq!(by_wheel(true, KeyModifiers::SHIFT), by_colon(":drawer scroll 3"), "shift scrolls rows too");
        assert!(by_wheel(true, KeyModifiers::NONE).starts_with("sel=7 "), "{}", by_wheel(true, KeyModifiers::NONE));
    });
}

#[test]
fn mouse_edge_drag_resizes_navigator_width() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        let w0 = wm.drawer_config().width_cols;
        let dr = wm.drawer_rect(area).unwrap();
        assert_eq!(dr.width, 28);
        let border = dr.x + dr.width - 1;
        handle_wm_mouse_event(&mut wm, mouse_ev(MouseEventKind::Down(MouseButton::Left), border, 10, KeyModifiers::NONE), area)
            .unwrap();
        assert!(wm.drawer().resizing_drag);
        handle_wm_mouse_event(&mut wm, mouse_ev(MouseEventKind::Drag(MouseButton::Left), 31, 10, KeyModifiers::NONE), area)
            .unwrap();
        handle_wm_mouse_event(&mut wm, mouse_ev(MouseEventKind::Up(MouseButton::Left), 31, 10, KeyModifiers::NONE), area)
            .unwrap();
        assert!(!wm.drawer().resizing_drag, "release ends the drag");
        assert_eq!(wm.drawer_config().navigator_width_cols, 32);
        assert_eq!(wm.drawer_config().width_cols, w0, "classic width untouched");
        let (mut colon, _a) = open_navigator(100, 30);
        run_colon(&mut colon, ":drawer size 32");
        assert_eq!(wm.drawer_config().navigator_width_cols, colon.drawer_config().navigator_width_cols);
        assert_eq!(wm.drawer_rect(area), colon.drawer_rect(area));
    });
}

#[test]
fn mouse_ctrl_wheel_scales_drawer_on_gui_surface() {
    use bilterm::ui::typography::PanelTarget;
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        wm.set_gui_surface(true);
        let before = wm.scale_pct(PanelTarget::Drawer);
        wheel(&mut wm, area, false, KeyModifiers::CONTROL, 3, 5);
        assert_ne!(wm.scale_pct(PanelTarget::Drawer), before, "Ctrl+wheel steps the drawer scale");
    });
}

/// Runs `key` on one fresh fixture and `mouse` on another; the observable state must match.
fn parity(name: &str, setup: &dyn Fn(&mut WindowManager), key: &dyn Fn(&mut WindowManager), mouse: &dyn Fn(&mut WindowManager, Rect)) {
    let by_key = with_temp_xdg(|| {
        let (mut wm, _a) = open_navigator(100, 30);
        setup(&mut wm);
        key(&mut wm);
        (observe(&wm), wm.drawer_config().visibility)
    });
    let by_mouse = with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        setup(&mut wm);
        mouse(&mut wm, area);
        (observe(&wm), wm.drawer_config().visibility)
    });
    assert_eq!(by_key, by_mouse, "parity `{name}`");
}

#[test]
fn parity_navigator_key_and_mouse_paths_match() {
    let nothing = |_: &mut WindowManager| {};
    // Row / Next + Activate: `j j` + Enter versus a click on the pane #2 row.
    parity(
        "row+activate",
        &nothing,
        &|wm| {
            ch(wm, 'j');
            press(wm, KeyCode::Enter);
        },
        &|wm, a| click_row(wm, a, &DrawerRowKind::Pane(2)),
    );
    // Fold: `k` to the workspace row then `e` versus a fold-marker click.
    parity(
        "fold",
        &nothing,
        &|wm| {
            ch(wm, 'k');
            ch(wm, 'e');
        },
        &|wm, a| {
            let c = navigator_fold_marker_cells(wm, a)[0];
            click_at(wm, a, c.x, c.y);
        },
    );
    // NextAttention.
    let blocked = |wm: &mut WindowManager| with_states(wm, &[(2, Some(AgentState::Blocked))]);
    parity("attention", &blocked, &|wm| ch(wm, '!'), &|wm, a| click_button(wm, a, NavigatorButtonKind::Attention));
    // Scroll: wheel versus the colon twin (PageUp/PageDown use the same op with another step).
    parity("scroll", &nothing, &|wm| run_colon(wm, ":drawer scroll 3"), &|wm, a| wheel(wm, a, true, KeyModifiers::NONE, 3, 5));
    // Widen: `}` (+2) versus dragging the edge two columns.
    parity(
        "widen",
        &nothing,
        &|wm| ch(wm, '}'),
        &|wm, a| {
            let dr = wm.drawer_rect(a).unwrap();
            let border = dr.x + dr.width - 1;
            for (kind, col) in [
                (MouseEventKind::Down(MouseButton::Left), border),
                (MouseEventKind::Drag(MouseButton::Left), border + 2),
                (MouseEventKind::Up(MouseButton::Left), border + 2),
            ] {
                handle_wm_mouse_event(wm, mouse_ev(kind, col, 10, KeyModifiers::NONE), a).unwrap();
            }
        },
    );
    parity("new-session", &nothing, &|wm| ch(wm, 'N'), &|wm, a| click_button(wm, a, NavigatorButtonKind::NewSession));
    parity("new-pane", &nothing, &|wm| ch(wm, 'n'), &|wm, a| click_button(wm, a, NavigatorButtonKind::NewPane));
    parity("rename", &nothing, &|wm| ch(wm, 'r'), &|wm, a| click_button(wm, a, NavigatorButtonKind::Rename));
    parity("close", &nothing, &|wm| ch(wm, 'x'), &|wm, a| click_button(wm, a, NavigatorButtonKind::Close));
    parity("pin", &nothing, &|wm| ch(wm, '*'), &|wm, a| click_button(wm, a, NavigatorButtonKind::Pin));
    // Key-only ops with no mouse requirement: E / C (expand / collapse all) have key and colon twins only.
}

// ---- Task 4: right-click context menu and [+] buttons ----

use bilterm::ui::drawer::navigator::{
    navigator_header_plus, navigator_menu_entry_at, navigator_menu_layout, navigator_plus_cells,
};
use bilterm::wm::drawer::NavigatorMenuEntry;

fn right_click_at(wm: &mut WindowManager, area: Rect, col: u16, row: u16) {
    handle_wm_mouse_event(wm, mouse_ev(MouseEventKind::Down(MouseButton::Right), col, row, KeyModifiers::NONE), area)
        .unwrap();
}

/// Any cell of the list row of `kind` (two columns in, clear of the fold marker).
fn cell_of_row(wm: &WindowManager, area: Rect, kind: &DrawerRowKind) -> (u16, u16) {
    let idx = row_of(wm, kind);
    let (x, y) = (0..area.width)
        .flat_map(|x| (0..area.height).map(move |y| (x, y)))
        .find(|&(x, y)| navigator_row_at(wm, area, x, y) == Some(idx))
        .expect("row has a cell");
    (x + 4, y)
}

fn menu_entry_cell(wm: &WindowManager, area: Rect, entry: NavigatorMenuEntry) -> (u16, u16) {
    let menu = navigator_menu_layout(wm, area).expect("menu open");
    let y = menu.entries.iter().find(|(e, _)| *e == entry).expect("entry shown").1;
    (menu.rect.x + 2, y)
}

#[test]
fn rclick_row_opens_menu_and_entries_match_key_twins() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        assert!(navigator_menu_layout(&wm, area).is_none());
        let (x, y) = cell_of_row(&wm, area, &DrawerRowKind::Pane(2));
        right_click_at(&mut wm, area, x, y);
        let menu = navigator_menu_layout(&wm, area).expect("right-click opens the menu");
        assert_eq!(wm.selected_drawer_pane_id(), Some(2), "right-click selects the row");
        assert_eq!(menu.entries.len(), 6);
        let dr = wm.drawer_rect(area).unwrap();
        assert!(menu.rect.x > dr.x && menu.rect.x + menu.rect.width < dr.x + dr.width);
        assert!(menu.rect.y > dr.y && menu.rect.y + menu.rect.height < dr.y + dr.height);
        let text = buffer_lines(&render(&wm, area)).join("\n");
        for e in NavigatorMenuEntry::ALL {
            assert!(text.contains(e.label()), "menu entry `{}` not rendered:\n{text}", e.label());
        }
    });
    type Key = fn(&mut WindowManager);
    let twins: [(NavigatorMenuEntry, Key); 5] = [
        (NavigatorMenuEntry::NewSession, |wm| ch(wm, 'N')),
        (NavigatorMenuEntry::NewWindow, |wm| ch(wm, 'a')),
        (NavigatorMenuEntry::SplitHorizontal, |wm| press_mod(wm, KeyCode::Char('A'), KeyModifiers::SHIFT)),
        (NavigatorMenuEntry::Rename, |wm| ch(wm, 'r')),
        (NavigatorMenuEntry::Close, |wm| ch(wm, 'x')),
    ];
    for (entry, key) in twins {
        let by_key = with_temp_xdg(|| {
            let (mut wm, area) = open_navigator(100, 30);
            wm.drawer_mut().selected_index = row_of(&wm, &DrawerRowKind::Pane(2));
            let _ = area;
            key(&mut wm);
            observe(&wm)
        });
        let by_menu = with_temp_xdg(|| {
            let (mut wm, area) = open_navigator(100, 30);
            let (x, y) = cell_of_row(&wm, area, &DrawerRowKind::Pane(2));
            right_click_at(&mut wm, area, x, y);
            let (ex, ey) = menu_entry_cell(&wm, area, entry);
            click_at(&mut wm, area, ex, ey);
            assert!(navigator_menu_layout(&wm, area).is_none(), "picking closes the menu");
            observe(&wm)
        });
        assert_eq!(by_key, by_menu, "menu entry {entry:?}");
    }
    // Split vertical has no single-key twin on `A`; it equals the colon twin.
    let by_colon = with_temp_xdg(|| {
        let (mut wm, _a) = open_navigator(100, 30);
        wm.drawer_mut().selected_index = row_of(&wm, &DrawerRowKind::Pane(2));
        run_colon(&mut wm, ":drawer add pane v");
        observe(&wm)
    });
    let by_menu = with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        let (x, y) = cell_of_row(&wm, area, &DrawerRowKind::Pane(2));
        right_click_at(&mut wm, area, x, y);
        let (ex, ey) = menu_entry_cell(&wm, area, NavigatorMenuEntry::SplitVertical);
        click_at(&mut wm, area, ex, ey);
        observe(&wm)
    });
    assert_eq!(by_colon, by_menu);
}

#[test]
fn rclick_menu_colon_twins_match_mouse() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        run_colon(&mut wm, ":drawer menu");
        assert!(navigator_menu_layout(&wm, area).is_some());
        run_colon(&mut wm, ":drawer menu close");
        assert!(navigator_menu_layout(&wm, area).is_none());
        run_colon(&mut wm, ":drawer menu 6 4");
        let menu = navigator_menu_layout(&wm, area).unwrap();
        assert_eq!(menu.rect.x, 6);
        wm.drawer_mut().selected_index = row_of(&wm, &DrawerRowKind::Pane(2));
        let before = wm.pane_count();
        run_colon(&mut wm, ":drawer menu pick 6");
        assert_eq!(wm.pane_count(), before - 1, "entry 6 is Close");
        assert!(navigator_menu_layout(&wm, area).is_none());
        for bad in [":drawer menu pick", ":drawer menu pick 0", ":drawer menu x", ":drawer menu 1 2 3"] {
            assert!(parse_colon_command(bad).unwrap().is_err(), "{bad} must be a usage error");
        }
    });
}

#[test]
fn rclick_esc_and_click_away_close_menu() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        let (x, y) = cell_of_row(&wm, area, &DrawerRowKind::Pane(1));
        right_click_at(&mut wm, area, x, y);
        assert!(navigator_menu_layout(&wm, area).is_some());
        press(&mut wm, KeyCode::Esc);
        assert!(navigator_menu_layout(&wm, area).is_none(), "Esc closes the menu");
        assert_eq!(wm.pane_count(), 2);

        right_click_at(&mut wm, area, x, y);
        click_at(&mut wm, area, 90, 20);
        assert!(navigator_menu_layout(&wm, area).is_none(), "click outside the drawer closes the menu");

        right_click_at(&mut wm, area, x, y);
        let menu = navigator_menu_layout(&wm, area).unwrap();
        assert!(navigator_menu_entry_at(&wm, area, menu.rect.x, menu.rect.y).is_none(), "the title line is no entry");
        let dr = wm.drawer_rect(area).unwrap();
        click_at(&mut wm, area, dr.x + 2, dr.y + dr.height - 3);
        assert!(navigator_menu_layout(&wm, area).is_none(), "click on the drawer away from the menu closes it");
        assert_eq!(wm.pane_count(), 2, "dismissing creates nothing");
    });
}

#[test]
fn rclick_elsewhere_still_pastes() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        // Pre-seed the yank buffer so a paste has content even without a system clipboard.
        wm.pane_mut(2).unwrap().record_yank("PASTED");
        wm.pane_mut(1).unwrap().record_yank("PASTED");
        let dr = wm.drawer_rect(area).unwrap();
        let (x, y) = (dr.x + dr.width + 10, 5);
        right_click_at(&mut wm, area, x, y);
        assert!(navigator_menu_layout(&wm, area).is_none(), "outside the drawer no menu opens");
        assert!(
            !wm.active_pane().prompt().text().is_empty(),
            "right-click outside the navigator pastes into the pane"
        );
    });
}

#[test]
fn rclick_menu_text_is_sanitized_and_clipped() {
    with_temp_xdg(|| {
        // A 24-column workspace caps the drawer at 12 columns: the menu must shrink to the drawer.
        let (mut wm, area) = navigator_wm(24, 30);
        wm.pane_mut(1).unwrap().set_title("ev\u{202E}il\u{009B}title");
        wm.toggle_drawer();
        let before = render(&wm, area);
        let dr = wm.drawer_rect(area).unwrap();
        // Anchor in the bottom-right corner of the drawer to force clamping on both axes.
        run_colon(&mut wm, &format!(":drawer menu {} {}", dr.x + dr.width - 2, dr.y + dr.height - 2));
        let menu = navigator_menu_layout(&wm, area).expect("menu fits the small drawer");
        assert!(menu.rect.x > dr.x && menu.rect.x + menu.rect.width < dr.x + dr.width, "{menu:?} vs {dr:?}");
        assert!(menu.rect.y > dr.y && menu.rect.y + menu.rect.height < dr.y + dr.height, "{menu:?} vs {dr:?}");
        let after = render(&wm, area);
        for yy in 0..area.height {
            for xx in 0..area.width {
                let inside = xx >= dr.x && xx < dr.x + dr.width && yy >= dr.y && yy < dr.y + dr.height;
                if !inside {
                    assert_eq!(before[(xx, yy)], after[(xx, yy)], "menu drew outside the drawer at ({xx},{yy})");
                }
            }
        }
        let text = buffer_lines(&after).join("\n");
        assert!(!text.contains('\u{202E}') && !text.contains('\u{009B}'), "control characters reached the menu");
    });
}

#[test]
fn rclick_classic_unaffected() {
    with_temp_xdg(|| {
        let (mut wm, area) = fixture(100, 30);
        wm.toggle_drawer();
        let dr = wm.drawer_rect(area).unwrap();
        right_click_at(&mut wm, area, dr.x + 4, dr.y + 4);
        assert!(navigator_menu_layout(&wm, area).is_none());
        assert!(wm.navigator_menu().is_none());
        assert!(navigator_plus_cells(&wm, area).is_empty(), "classic rows get no [+] cells");
        assert_eq!(wm.drawer_config().style, DrawerStyle::Classic);
    });
}

#[test]
fn plus_session_row_button_adds_window_like_key_a() {
    let by_key = with_temp_xdg(|| {
        let (mut wm, _a) = two_sessions();
        wm.drawer_mut().selected_index = row_of(&wm, &session("other"));
        ch(&mut wm, 'a');
        observe(&wm)
    });
    let by_mouse = with_temp_xdg(|| {
        let (mut wm, area) = two_sessions();
        let idx = row_of(&wm, &session("other"));
        let cell = navigator_plus_cells(&wm, area).into_iter().find(|c| c.row_index == idx).expect("[+] on the session row");
        let lines = buffer_lines(&render(&wm, area));
        assert_eq!(&lines[usize::from(cell.y)].chars().skip(usize::from(cell.x)).take(3).collect::<String>(), "[+]");
        click_at(&mut wm, area, cell.x + 1, cell.y);
        assert_eq!(wm.config().history_session, "other");
        observe(&wm)
    });
    assert_eq!(by_key, by_mouse);
}

#[test]
fn plus_header_button_creates_session_like_key_n() {
    let by_key = with_temp_xdg(|| {
        let (mut wm, _a) = open_navigator(100, 30);
        ch(&mut wm, 'N');
        observe(&wm)
    });
    let by_mouse = with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        let r = navigator_header_plus(&wm, area).expect("header [+] fits a 28-column drawer");
        let lines = buffer_lines(&render(&wm, area));
        assert_eq!(&lines[usize::from(r.y)].chars().skip(usize::from(r.x)).take(3).collect::<String>(), "[+]");
        click_at(&mut wm, area, r.x + 1, r.y);
        observe(&wm)
    });
    assert_eq!(by_key, by_mouse);
}

#[test]
fn plus_buttons_only_where_they_fit() {
    with_temp_xdg(|| {
        // Counts fill a narrow row: the button is dropped rather than overlapping them.
        let (mut wm, area) = navigator_wm(24, 30);
        wm.toggle_drawer();
        with_states(&mut wm, &[(1, Some(AgentState::Blocked)), (2, Some(AgentState::Working))]);
        let dr = wm.drawer_rect(area).unwrap();
        for c in navigator_plus_cells(&wm, area) {
            assert!(c.x + 3 < dr.x + dr.width, "[+] stays inside the drawer: {c:?}");
        }
        if let Some(r) = navigator_header_plus(&wm, area) {
            assert!(r.x + r.width < dr.x + dr.width);
        }
    });
}

// ---- Phase 50-12: comms indicator on navigator pane rows (D-06) ----

use std::time::{Duration, Instant};

use bilterm::ui::drawer::navigator::navigator_comms_cells;
use bilterm::wm::agents_window::NodeKey;
use bilterm::wm::drawer::CommsIndicator;
use ratatui::style::Modifier;

/// Drawer-interior line of pane `id` (`#<id> ` row).
fn comms_line(wm: &WindowManager, area: Rect, id: u32) -> String {
    let tag = format!("#{id} ");
    drawer_interior(wm, area)
        .into_iter()
        .find(|l| l.contains(&tag))
        .unwrap_or_else(|| panic!("no row for pane #{id}"))
        .trim_end()
        .to_string()
}

/// Records `from -> Pane(to)` at `at`.
fn talk(wm: &mut WindowManager, from: TrafficEndpoint, to: u32, at: Instant) {
    wm.record_traffic(from, TrafficEndpoint::Pane(to as _), TrafficKind::Message, "ping", at);
}

fn pane1_comms(wm: &WindowManager) -> Option<CommsIndicator> {
    wm.drawer_rows().into_iter().find(|r| r.kind == DrawerRowKind::Pane(1))?.navigator?.comms
}

fn pair_key() -> (NodeKey, NodeKey) {
    (NodeKey("pane:1".to_string()), NodeKey("user".to_string()))
}

#[test]
fn comms_tracer_pair_glyph_click_opens_edge() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        wm.broadcast_command("echo hi").unwrap();
        wm.update_agent_states(now());
        let l1 = comms_line(&wm, area, 1);
        assert!(l1.ends_with("⇄you"), "pane #1 row must end with the pairing glyph: {l1:?}");
        assert!(comms_line(&wm, area, 2).ends_with("⇄you"));

        let cell = navigator_comms_cells(&wm, area).into_iter().find(|c| c.row_index == 1).expect("comms cell for #1");
        assert_eq!(cell.width, 4);
        click_at(&mut wm, area, cell.x, cell.y);
        assert!(wm.agents_window().active, "clicking the glyph opens the zoom window");
        assert_eq!(wm.agents_window().selection.edge, Some(pair_key()), "the user-pane:1 edge is selected");
        assert_eq!(wm.agents_window().snoop.filter_pair, Some(pair_key()), "conversation filtered to the pair");
    });
}

#[test]
fn comms_fanout_shows_count() {
    with_temp_xdg(|| {
        let (mut wm, area) = n_pane_navigator(3, 100, 30);
        wm.toggle_drawer();
        let t = now();
        talk(&mut wm, TrafficEndpoint::User, 1, t);
        talk(&mut wm, TrafficEndpoint::Api, 1, t);
        talk(&mut wm, TrafficEndpoint::Pane(3), 1, t);
        wm.update_agent_states(t + Duration::from_millis(10));
        assert_eq!(pane1_comms(&wm), Some(CommsIndicator::FanOut { count: 3 }));
        let l1 = comms_line(&wm, area, 1);
        assert!(l1.ends_with("⋔3"), "{l1:?}");
        // Pane 3 talks to exactly one partner (#1): a pair labelled `#1`.
        assert!(comms_line(&wm, area, 3).ends_with("⇄#1"));
        // Fan-out click: pane selected on the graph, conversation filtered to that pane.
        let cell = navigator_comms_cells(&wm, area).into_iter().find(|c| c.row_index == 1).unwrap();
        click_at(&mut wm, area, cell.x, cell.y);
        assert_eq!(wm.agents_window().snoop.filter_agent, Some(NodeKey::pane(1)));
        assert_eq!(wm.agents_window().selection.node, Some(NodeKey::pane(1)));
    });
}

/// Focused navigator over `n` panes with `user -> #1` traffic recorded and applied.
fn focused_with_comms(n: u32) -> (WindowManager, Rect) {
    let (mut wm, area) = focused_navigator(n);
    let t = now();
    talk(&mut wm, TrafficEndpoint::User, 1, t);
    wm.update_agent_states(t + Duration::from_millis(10));
    (wm, area)
}

#[test]
fn comms_right_then_enter_opens_window() {
    with_temp_xdg(|| {
        let (mut wm, area) = focused_with_comms(3);
        press(&mut wm, KeyCode::Right);
        assert_eq!(wm.drawer().navigator.comms_focus, Some(1));
        let cell = navigator_comms_cells(&wm, area).into_iter().find(|c| c.row_index == 1).unwrap();
        let buf = render(&wm, area);
        assert!(buf[(cell.x, cell.y)].modifier.contains(Modifier::UNDERLINED), "focused glyph is highlighted");

        // Moving the selection makes the comms focus inert: Enter now activates the other row.
        ch(&mut wm, 'j');
        assert_ne!(wm.drawer().navigator.comms_focus, Some(wm.drawer().selected_index));
        press(&mut wm, KeyCode::Enter);
        assert!(!wm.agents_window().active);
        assert_eq!(wm.active_pane_id(), 2);

        wm.drawer_mut().focused = true;
        wm.drawer_mut().selected_index = 1;
        press(&mut wm, KeyCode::Right);
        press(&mut wm, KeyCode::Left);
        assert_eq!(wm.drawer().navigator.comms_focus, None, "Left releases the glyph");
        press(&mut wm, KeyCode::Right);
        press(&mut wm, KeyCode::Esc);
        assert_eq!(wm.drawer().navigator.comms_focus, None, "Esc releases the glyph first");
        assert!(wm.drawer().focused, "Esc does not leave the drawer while comms focus is on");

        press(&mut wm, KeyCode::Right);
        press(&mut wm, KeyCode::Enter);
        assert!(wm.agents_window().active);
        assert_eq!(wm.agents_window().selection.edge, Some(pair_key()));
        assert_eq!(wm.agents_window().snoop.filter_pair, Some(pair_key()));
    });
}

#[test]
fn comms_right_without_comms_falls_through() {
    with_temp_xdg(|| {
        let (mut wm, _a) = focused_navigator(2);
        press(&mut wm, KeyCode::Right);
        assert_eq!(wm.drawer().navigator.comms_focus, None);
        assert!(!wm.agents_window().active);
        assert!(!wm.drawer().focused || wm.drawer().h_scroll > 0, "classic Right behaviour (hscroll / leave drawer)");
    });
}

#[test]
fn comms_o_key_and_colon_comms_open_window() {
    let by_key = with_temp_xdg(|| {
        let (mut wm, _a) = focused_with_comms(2);
        ch(&mut wm, 'o');
        assert!(wm.agents_window().active);
        assert_eq!(wm.agents_window().selection.edge, Some(pair_key()));
        observe(&wm)
    });
    let by_colon = with_temp_xdg(|| {
        let (mut wm, _a) = focused_with_comms(2);
        run_colon(&mut wm, ":drawer comms");
        assert_eq!(wm.agents_window().snoop.filter_pair, Some(pair_key()));
        observe(&wm)
    });
    assert_eq!(by_key, by_colon);
}

#[test]
fn comms_colon_comms_focus_matches_keys() {
    let state = |wm: &WindowManager| (wm.drawer().navigator.comms_focus, wm.drawer().focused);
    let (by_key, by_colon) = with_temp_xdg(|| {
        let (mut a, _x) = focused_with_comms(2);
        let (mut b, _y) = focused_with_comms(2);
        let mut seen = Vec::new();
        press(&mut a, KeyCode::Right);
        run_colon(&mut b, ":drawer comms-focus on");
        seen.push((state(&a), state(&b)));
        press(&mut a, KeyCode::Left);
        run_colon(&mut b, ":drawer comms-focus off");
        seen.push((state(&a), state(&b)));
        (seen.iter().map(|p| p.0).collect::<Vec<_>>(), seen.iter().map(|p| p.1).collect::<Vec<_>>())
    });
    assert_eq!(by_key, by_colon);
    assert_eq!(by_key[0].0, Some(1));
    assert_eq!(by_key[1].0, None);
}

#[test]
fn comms_animates_only_while_active() {
    with_temp_xdg(|| {
        let (mut wm, area) = open_navigator(100, 30);
        let t = now();
        talk(&mut wm, TrafficEndpoint::User, 1, t);
        wm.update_agent_states(t + Duration::from_millis(100));
        let frame = |wm: &WindowManager| comms_line(wm, area, 1).chars().rev().nth(3).unwrap();
        let first = frame(&wm);
        wm.update_drawer_telemetry();
        let second = frame(&wm);
        assert_ne!(first, second, "frame alternates between ticks while traffic flows");
        assert!(["⇄", "⇆"].contains(&first.to_string().as_str()) && ["⇄", "⇆"].contains(&second.to_string().as_str()));
        // Past the 1.5 s window the glyph is still, whatever the tick parity.
        wm.update_agent_states(t + Duration::from_secs(3));
        let still = frame(&wm);
        wm.update_drawer_telemetry();
        assert_eq!(still, frame(&wm));
        assert_eq!(still, '⇄');
        assert!(!wm.drawer_rows()[1].navigator.as_ref().unwrap().comms_active);
    });
}

#[test]
fn comms_tick_redraws_while_active() {
    with_temp_xdg(|| {
        let (mut wm, _a) = open_navigator(100, 30);
        let t = now();
        talk(&mut wm, TrafficEndpoint::User, 1, t);
        assert!(wm.tick_agent_ui(t), "first tick records the states");
        assert!(!wm.tick_agent_ui(t + Duration::from_millis(50)), "inside the 120 ms cadence");
        assert!(wm.tick_agent_ui(t + Duration::from_millis(130)), "flowing traffic redraws at the cadence");
        assert!(!wm.tick_agent_ui(t + Duration::from_secs(3)), "no redraw once the traffic is stale");
        assert!(!wm.tick_agent_ui(t + Duration::from_secs(4)));
    });
}

#[test]
fn comms_ascii_glyphs() {
    with_temp_xdg(|| {
        let (mut wm, area) = n_pane_navigator(3, 100, 30);
        run_colon(&mut wm, ":set glyphs ascii");
        wm.toggle_drawer();
        let t = now();
        talk(&mut wm, TrafficEndpoint::User, 2, t);
        talk(&mut wm, TrafficEndpoint::User, 1, t);
        talk(&mut wm, TrafficEndpoint::Api, 1, t);
        wm.update_agent_states(t + Duration::from_secs(5));
        assert!(comms_line(&wm, area, 2).ends_with("<>you"), "{:?}", comms_line(&wm, area, 2));
        assert!(comms_line(&wm, area, 1).ends_with(">>2"), "{:?}", comms_line(&wm, area, 1));
    });
}

#[test]
fn comms_narrow_width_keeps_glyph() {
    with_temp_xdg(|| {
        let (mut wm, area) = navigator_wm(100, 30);
        run_colon(&mut wm, ":drawer size 24");
        wm.toggle_drawer();
        wm.pane_mut(1).unwrap().process_pty_output(b"\x1b]2;averyverylongpanetitleindeed\x07");
        let t = now();
        talk(&mut wm, TrafficEndpoint::Agent("ab\u{202e}cdefghij".to_string()), 1, t);
        wm.update_agent_states(t + Duration::from_secs(5));
        let line = comms_line(&wm, area, 1);
        assert!(line.ends_with("⇄abcde…"), "title truncates, label capped at 6 and sanitised: {line:?}");
        assert!(line.contains('…') && !line.contains('\u{202e}'));
        assert!(line.chars().count() <= 22, "row fits the 24-column drawer: {line:?}");
        assert!(line.contains("#1 "), "the pane prefix survives: {line:?}");

        // Narrower still: the partner label is cut before the glyph is dropped.
        run_colon(&mut wm, ":drawer size 16");
        let tight = comms_line(&wm, area, 1);
        assert!(tight.contains('⇄'), "glyph is never dropped: {tight:?}");
        assert!(tight.contains("#1"), "{tight:?}");
    });
}
