//! Phase 50.1-07: the Find window result list, tab chips, source tags, footer and masked count
//! (FND-R02, D-U2, D-U3), drawn on a `Buffer` shared by the TUI and the GUI surface.

use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
use bilterm::ui::{WorkspaceWidget, buffer_to_snapshot_string};
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::find_cmd::FindKind;
use bilterm::wm::registry::{
    AgentRow, FindResults, FindRow, HistoryRow, Origin, PaneRow, Recency, ResultSource, RowData, SessionRow,
    Transport,
};
use bilterm::wm::{SplitDirection, WindowManager};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::Widget;

fn fixture(cols: u16, rows: u16) -> WindowManager {
    let mut wm = WindowManager::new_headless(rows, cols);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, cols, rows)).unwrap();
    wm
}

fn source(origin: Origin, name: &str, path: Option<&str>) -> ResultSource {
    ResultSource { origin, name: name.to_string(), path: path.map(str::to_string) }
}

fn agent_row(pane: u64, name: &str, state: AgentState, since_ms: u64, matched: Vec<u32>) -> FindRow {
    let mut r = FindRow::empty(FindKind::Agent);
    r.id = format!("agent:work/{pane}");
    r.label = name.to_string();
    r.pane = Some(pane as _);
    r.session = "work".to_string();
    r.data = RowData::Agent(AgentRow {
        pane: pane as _,
        name: name.to_string(),
        state,
        since_ms,
        profile: None,
        title: name.to_string(),
    });
    r.matched = matched;
    r.source = ResultSource::live("work");
    r
}

fn session_row(name: &str, origin: Origin, path: Option<&str>) -> FindRow {
    let mut r = FindRow::empty(FindKind::Session);
    r.id = format!("session:{name}");
    r.label = name.to_string();
    r.session = name.to_string();
    r.source = source(origin, name, path);
    r.data = RowData::Session(SessionRow { name: name.to_string(), active: false, parent: None, pane_count: 2 });
    r
}

fn pane_row(title: &str, transport: Transport, host: Option<&str>, metadata_only: bool) -> FindRow {
    let mut r = FindRow::empty(FindKind::Session);
    r.id = format!("pane:work/{title}");
    r.label = title.to_string();
    r.session = "work".to_string();
    r.transport = transport;
    r.host = host.map(str::to_string);
    r.metadata_only = metadata_only;
    r.source = ResultSource::live("work");
    r.data = RowData::Pane(PaneRow {
        pane: 3,
        title: title.to_string(),
        cwd: None,
        cwd_trusted: false,
        running_command: None,
    });
    r
}

fn history_row(cmd: &str, exit: Option<i32>, origin: Origin, path: Option<&str>) -> FindRow {
    let mut r = FindRow::empty(FindKind::History);
    r.id = format!("history:work/{cmd}");
    r.label = cmd.to_string();
    r.session = "work".to_string();
    r.source = source(origin, "work", path);
    r.data = RowData::History(HistoryRow {
        seq: 1,
        pane: Some(2),
        cwd: None,
        command: cmd.to_string(),
        exit_code: exit,
        duration_ms: None,
        at: Recency::Saved,
    });
    r
}

fn results(kind: FindKind, rows: Vec<FindRow>, masked: usize) -> FindResults {
    let total = rows.len();
    FindResults { kind, query: String::new(), rows, total, truncated: false, masked }
}

fn open(wm: &mut WindowManager, kind: FindKind, query: &str, res: FindResults) {
    wm.open_find(kind, query);
    wm.set_find_results(res);
}

fn buffer(wm: &WindowManager, cols: u16, rows: u16) -> Buffer {
    let area = Rect::new(0, 0, cols, rows);
    let mut buf = Buffer::empty(area);
    WorkspaceWidget::new(wm).render(area, &mut buf);
    buf
}

fn text(buf: &Buffer) -> String {
    buffer_to_snapshot_string(buf)
}

fn line_with<'a>(frame: &'a str, needle: &str) -> &'a str {
    frame.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("no line with {needle:?} in:\n{frame}"))
}

#[test]
fn find_tracer_row_shows_label_highlight_and_source_tag() {
    let mut wm = fixture(100, 30);
    // Row 0 is selected (emphasised as a whole); the matched cells are checked on the second row.
    let first = agent_row(2, "zeta-other", AgentState::Idle, 0, vec![]);
    let row = agent_row(1, "claude-main", AgentState::Working, 4000, vec![0, 1]);
    open(&mut wm, FindKind::Agent, "cl", results(FindKind::Agent, vec![first, row], 0));
    let buf = buffer(&wm, 100, 30);
    let frame = text(&buf);
    let line = line_with(&frame, "claude-main");
    assert!(line.contains("[live]"), "{line}");
    let after_tag = &line[line.find("[live]").unwrap() + "[live]".len()..];
    assert!(after_tag.chars().all(|c| matches!(c, ' ' | '║' | '│' | '|')), "tag must be right-aligned: {line}");
    // The first two label cells carry the bold modifier; the third does not.
    let (y, x) = (0..30u16)
        .find_map(|y| {
            (0..97u16)
                .find(|x| ["c", "l", "a", "u"].iter().enumerate().all(|(i, s)| buf[(*x + i as u16, y)].symbol() == *s))
                .map(|x| (y, x))
        })
        .expect("label cells");
    assert!(buf[(x, y)].modifier.contains(Modifier::BOLD));
    assert!(buf[(x + 1, y)].modifier.contains(Modifier::BOLD));
    assert!(!buf[(x + 2, y)].modifier.contains(Modifier::BOLD), "unmatched cell must not be bold");
}

#[test]
fn every_row_shows_its_source_tag() {
    let cases = [
        (
            FindKind::Agent,
            vec![agent_row(1, "alpha", AgentState::Idle, 0, vec![])],
            "alpha",
            "[live]",
        ),
        (
            FindKind::Session,
            vec![
                session_row("work", Origin::Live, None),
                session_row("old", Origin::SavedSession, Some("/state/sessions/old.toml")),
                pane_row("build-box", Transport::Ssh, Some("prod"), false),
            ],
            "old",
            "[saved:old]",
        ),
        (
            FindKind::History,
            vec![history_row("git status", Some(0), Origin::HistoryLog, Some("/state/history.log"))],
            "git status",
            "[log:work]",
        ),
    ];
    for (kind, rows, probe, tag) in cases {
        let n = rows.len();
        let mut wm = fixture(100, 30);
        open(&mut wm, kind, "", results(kind, rows, 0));
        let frame = text(&buffer(&wm, 100, 30));
        assert!(line_with(&frame, probe).contains(tag), "kind {kind:?}:\n{frame}");
        let tags = frame.matches("[live]").count() + frame.matches("[saved:").count() + frame.matches("[log:").count();
        assert!(tags >= n, "expected {n} tags, got {tags}:\n{frame}");
    }
}

#[test]
fn source_tag_survives_when_the_label_is_truncated() {
    let mut wm = fixture(100, 30);
    let long = "x".repeat(60);
    let mut row = history_row(&long, Some(1), Origin::HistoryLog, Some("/state/history.log"));
    row.source.name = "a-rather-long-session-name".to_string();
    open(&mut wm, FindKind::History, "", results(FindKind::History, vec![row], 0));
    let narrow = text(&buffer(&wm, 60, 30));
    let line = line_with(&narrow, "xxxx");
    assert!(line.contains("[log:a-rather-long-session-name]"), "{line}");
    assert!(line.contains('…') || line.contains('~'), "label should be truncated: {line}");
}

#[test]
fn row_text_and_transport_words() {
    let mut wm = fixture(120, 30);
    let mut ssh = pane_row("build-box", Transport::Ssh, Some("prod"), false);
    ssh.matched = vec![];
    let tmux = pane_row("editor", Transport::Tmux, None, true);
    let both = pane_row("deploy", Transport::SshTmux, Some("edge"), false);
    let mut saved = session_row("work", Origin::Live, None);
    saved.also_in.push(source(Origin::SavedSession, "work", Some("/state/sessions/work.toml")));
    open(
        &mut wm,
        FindKind::Session,
        "",
        results(FindKind::Session, vec![ssh, tmux, both, saved], 0),
    );
    let frame = text(&buffer(&wm, 120, 30));
    assert!(line_with(&frame, "build-box").contains("ssh"), "{frame}");
    assert!(line_with(&frame, "editor").contains("tmux"), "{frame}");
    assert!(line_with(&frame, "editor").contains("label-only"), "{frame}");
    assert!(line_with(&frame, "deploy").contains("ssh-tmux"), "{frame}");
    assert!(line_with(&frame, " work ").contains("(also saved:work)"), "{frame}");
}

#[test]
fn history_rows_show_exit_code_when_known() {
    let mut wm = fixture(100, 30);
    let rows = vec![
        history_row("make test", Some(2), Origin::Live, None),
        history_row("ls", None, Origin::Live, None),
    ];
    open(&mut wm, FindKind::History, "", results(FindKind::History, rows, 0));
    let frame = text(&buffer(&wm, 100, 30));
    assert!(line_with(&frame, "make test").contains("exit 2"), "{frame}");
    assert!(!line_with(&frame, " ls ").contains("exit"), "{frame}");
}

#[test]
fn tabs_query_and_footer_chips_are_drawn() {
    let mut wm = fixture(100, 30);
    open(&mut wm, FindKind::Session, "wo", results(FindKind::Session, vec![session_row("work", Origin::Live, None)], 0));
    let frame = text(&buffer(&wm, 100, 30));
    for needle in [
        "[Agents]",
        "[Sessions]",
        "[History]",
        "[clear]",
        "[Enter jump]",
        "[Tab kind]",
        "[^Y prefill]",
        "[Esc close]",
        "> wo_",
        "1 results",
        "1/1",
        "source: live work",
    ] {
        assert!(frame.contains(needle), "missing {needle:?}:\n{frame}");
    }
}

#[test]
fn source_line_shows_origin_name_and_path_of_the_selected_row() {
    let mut wm = fixture(100, 30);
    let rows = vec![
        session_row("work", Origin::Live, None),
        session_row("work", Origin::SavedSession, Some("/state/sessions/work.toml")),
    ];
    open(&mut wm, FindKind::Session, "", results(FindKind::Session, rows, 0));
    let frame = text(&buffer(&wm, 100, 30));
    assert!(frame.contains("source: live work"), "{frame}");
    assert!(!frame.contains("/state/sessions/work.toml"), "unselected path leaked:\n{frame}");
    wm.apply(bilterm::action::Action::Find(bilterm::wm::find_cmd::FindOp::Move(1))).ok();
    let frame = text(&buffer(&wm, 100, 30));
    assert!(frame.contains("source: saved-session work /state/sessions/work.toml"), "{frame}");
}

#[test]
fn masked_count_is_shown_without_text() {
    let secret = "hunter2-SECRET-token";
    let mut wm = fixture(100, 30);
    let rows = vec![history_row("echo ok", Some(0), Origin::HistoryLog, Some("/state/history.log"))];
    open(&mut wm, FindKind::History, "", results(FindKind::History, rows, 3));
    let frame = text(&buffer(&wm, 100, 30));
    assert!(frame.contains("3 masked"), "{frame}");
    assert!(!frame.contains(secret));
    // Empty list: the empty-state line carries the masked count too.
    open(&mut wm, FindKind::History, "zzz", results(FindKind::History, vec![], 3));
    let frame = text(&buffer(&wm, 100, 30));
    assert!(frame.contains("No history match, 3 masked"), "{frame}");
    // No masked entries: no masked text at all.
    open(&mut wm, FindKind::History, "", results(FindKind::History, vec![], 0));
    let frame = text(&buffer(&wm, 100, 30));
    assert!(!frame.contains("masked"), "{frame}");
    // Narrow window: the count still shows (it moves off the chip row if it must).
    let rows = vec![history_row("echo ok", Some(0), Origin::Live, None)];
    open(&mut wm, FindKind::History, "", results(FindKind::History, rows, 7));
    let frame = text(&buffer(&wm, 44, 12));
    assert!(frame.contains("7 masked"), "{frame}");
}

#[test]
fn empty_states_name_the_kind() {
    for (kind, msg) in [
        (FindKind::Agent, "No agents match"),
        (FindKind::Session, "No sessions match"),
        (FindKind::History, "No history match"),
    ] {
        let mut wm = fixture(100, 30);
        open(&mut wm, kind, "q", results(kind, vec![], 0));
        let frame = text(&buffer(&wm, 100, 30));
        assert!(frame.contains(msg), "{frame}");
        assert!(frame.contains("0 results"), "{frame}");
    }
}

#[test]
fn control_characters_in_rows_are_replaced() {
    let mut wm = fixture(100, 30);
    let mut row = agent_row(1, "evil\u{1b}[31mname", AgentState::Idle, 0, vec![]);
    row.label = "evil\u{1b}[31mname\u{202E}x".to_string();
    open(&mut wm, FindKind::Agent, "", results(FindKind::Agent, vec![row], 0));
    let buf = buffer(&wm, 100, 30);
    for y in 0..30 {
        for x in 0..100 {
            let sym = buf[(x, y)].symbol();
            assert!(!sym.contains('\u{1b}') && !sym.contains('\u{202E}'), "control cell at {x},{y}");
        }
    }
    assert!(text(&buf).contains("evil?[31mname?x"));
}

#[test]
fn footer_chips_sit_in_their_layout_rects() {
    let mut wm = fixture(100, 30);
    open(&mut wm, FindKind::Agent, "", results(FindKind::Agent, vec![], 0));
    let area = Rect::new(0, 0, 100, 30);
    let outer = wm.active_modal_rect(area).expect("find rect");
    let inner = bilterm::wm::inset_window_frame(outer);
    let layout = bilterm::ui::find_ui::find_layout(inner, &bilterm::config::BiltermConfig::default());
    let buf = buffer(&wm, 100, 30);
    let read = |r: Rect| -> String { (r.x..r.x + r.width).map(|x| buf[(x, r.y)].symbol().to_string()).collect() };
    for (chip, label) in layout.footer_chips.iter().zip(bilterm::ui::find_ui::FOOTER_LABELS) {
        assert_eq!(read(*chip), label);
    }
    for (chip, label) in layout.tab_chips.iter().zip(bilterm::ui::find_ui::TAB_LABELS) {
        assert_eq!(read(*chip), label);
    }
    assert_eq!(read(layout.clear_chip), bilterm::ui::find_ui::CLEAR_LABEL);
    // The active tab chip is reversed; the others are not.
    assert!(buf[(layout.tab_chips[0].x, layout.tab_chips[0].y)].modifier.contains(Modifier::REVERSED));
    assert!(!buf[(layout.tab_chips[1].x, layout.tab_chips[1].y)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn degenerate_sizes_do_not_panic() {
    for (cols, rows) in [(40u16, 8u16), (24, 6), (200, 60), (10, 4), (3, 3)] {
        let mut wm = fixture(cols.max(10), rows.max(6));
        let rs = vec![
            session_row("work", Origin::SavedSession, Some("/state/sessions/work.toml")),
            pane_row("a-very-long-pane-title-that-needs-truncation-to-fit", Transport::SshTmux, Some("edge"), true),
        ];
        open(&mut wm, FindKind::Session, "q", results(FindKind::Session, rs, 5));
        let _ = buffer(&wm, cols, rows);
    }
}

#[test]
fn long_lists_scroll_so_the_selection_stays_visible() {
    let mut wm = fixture(100, 20);
    let rows: Vec<FindRow> = (0..40).map(|i| agent_row(i, &format!("agent-{i:02}"), AgentState::Idle, 0, vec![])).collect();
    open(&mut wm, FindKind::Agent, "", results(FindKind::Agent, rows, 0));
    for _ in 0..30 {
        wm.apply(bilterm::action::Action::Find(bilterm::wm::find_cmd::FindOp::Move(1))).unwrap();
    }
    let frame = text(&buffer(&wm, 100, 20));
    assert!(frame.contains("agent-30"), "{frame}");
    assert!(!frame.contains("agent-00"), "{frame}");
    assert!(frame.contains("31/40"), "{frame}");
}

#[test]
fn gui_surface_draws_same_cells() {
    let mut wm = fixture(100, 30);
    let rows = vec![session_row("old", Origin::SavedSession, Some("/state/sessions/old.toml"))];
    open(&mut wm, FindKind::Session, "", results(FindKind::Session, rows, 2));
    wm.set_gui_surface(true);
    let area = Rect::new(0, 0, 100, 30);
    let mut renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    renderer.render_workspace_with_cell_size(&wm, area, 8, 16, &mut surface);
    // Glyph runs are one per cell and skip blanks; rebuild each row by column.
    let mut grid = vec![vec![' '; 100]; 30];
    for g in &surface.glyphs {
        if let (Some(c), true) = (g.text.chars().next(), g.row < 30 && g.col < 100) {
            grid[usize::from(g.row)][usize::from(g.col)] = c;
        }
    }
    let all: String = grid.iter().map(|r| r.iter().collect::<String>()).collect::<Vec<_>>().join("\n");
    for needle in ["FIND", "[saved:old]", "2 masked"] {
        assert!(all.contains(needle), "GUI glyphs miss {needle:?}:\n{all}");
    }
}

#[test]
fn snapshot_agents_tui() {
    let mut wm = fixture(100, 30);
    let rows = vec![
        agent_row(1, "claude-main", AgentState::Working, 4_000, vec![0, 1, 2]),
        agent_row(2, "claude-review", AgentState::Blocked, 125_000, vec![0, 1, 2]),
        agent_row(3, "codex-fix", AgentState::Idle, 7_300_000, vec![]),
    ];
    open(&mut wm, FindKind::Agent, "cla", results(FindKind::Agent, rows, 0));
    insta::assert_snapshot!("agents_tui", text(&buffer(&wm, 100, 30)));
}

#[test]
fn snapshot_sessions_sources_tui() {
    let mut wm = fixture(100, 30);
    let mut live = session_row("work", Origin::Live, None);
    live.also_in.push(source(Origin::SavedSession, "work", Some("/state/sessions/work.toml")));
    let rows = vec![
        live,
        pane_row("build-box", Transport::Ssh, Some("prod"), false),
        pane_row("editor", Transport::Tmux, None, true),
        session_row("old", Origin::SavedSession, Some("/state/sessions/old.toml")),
    ];
    open(&mut wm, FindKind::Session, "", results(FindKind::Session, rows, 0));
    insta::assert_snapshot!("sessions_sources_tui", text(&buffer(&wm, 100, 30)));
}

#[test]
fn snapshot_history_masked_tui() {
    let mut wm = fixture(100, 30);
    let rows = vec![
        history_row("cargo test", Some(0), Origin::Live, None),
        history_row("git push origin main", Some(1), Origin::HistoryLog, Some("/state/history.log")),
        history_row("ls -la", None, Origin::HistoryLog, Some("/state/history.log")),
    ];
    wm.open_find(FindKind::History, "");
    wm.set_find_results(results(FindKind::History, rows, 3));
    wm.apply(bilterm::action::Action::Find(bilterm::wm::find_cmd::FindOp::Move(1))).ok();
    insta::assert_snapshot!("history_masked_tui", text(&buffer(&wm, 100, 30)));
}
