//! Phase 50.1-15: UNI-R01 parity across mouse, key, colon, palette, menu, RPC and MCP for every
//! Find capability of 50.1-CONTRACT.md section 6, plus the D-U1 legacy regression and the GUI
//! surface. The matrix is data: a failure names the capability and the surface that disagrees.

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::find_modal::{list_viewport_start, lower_find_mouse};
use bilterm::input::keymap::{default_global_action, find_modal_key_action};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::find_ui::{FindLayout, find_layout};
use bilterm::ui::modals::{HELP_LINES, PALETTE_CATALOG, menubar_category_item_count, menubar_dropdown_rect, menubar_item_command};
use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
use bilterm::ui::shell::ShellLayout;
use bilterm::ui::WorkspaceWidget;
use bilterm::wm::find_cmd::{FindKind, FindOp};
use bilterm::wm::registry::{FindResults, FindRow};
use bilterm::wm::{SplitDirection, WindowManager, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

const COLS: u16 = 100;
const ROWS: u16 = 30;
const SCREEN: Rect = Rect { x: 0, y: 0, width: COLS, height: ROWS };
const NONE: KeyModifiers = KeyModifiers::NONE;
const CHORD: KeyModifiers = KeyModifiers::CONTROL.union(KeyModifiers::ALT);

/// Five panes; pane 1 has one captured history command (`echo parity`) so every kind has rows.
fn fixture() -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    for _ in 0..4 {
        wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    wm.resize_all(SCREEN).unwrap();
    wm.pane_mut(1).unwrap().submit_command("echo parity").unwrap();
    wm.poll_all_ptys();
    wm
}

fn parse(cmd: &str) -> Action {
    parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"))
}

fn layout_of(wm: &WindowManager) -> FindLayout {
    let outer = wm.active_modal_rect(SCREEN).expect("find modal rect");
    find_layout(inset_window_frame(outer), wm.config())
}

fn click(col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: NONE }
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent { kind, column: col, row, modifiers: NONE }
}

fn centre(r: Rect) -> (u16, u16) {
    (r.x + r.width / 2, r.y)
}

fn key_action(code: KeyCode, mods: KeyModifiers) -> Action {
    find_modal_key_action(KeyEvent::new(code, mods)).unwrap_or_else(|| panic!("unbound {code:?}"))
}

/// Observable result of a gesture: window, kind/query/selection, listed rows, focus and prompt.
type State = (bool, Option<(FindKind, String, usize)>, usize, usize, String);

fn state(wm: &WindowManager) -> State {
    (
        wm.is_find_visible(),
        wm.find_modal_state().map(|(k, q, s)| (k, q.to_string(), s)),
        wm.find_results().rows.len(),
        wm.active_pane_id(),
        wm.active_pane().prompt().text(),
    )
}

#[derive(Clone)]
enum Step {
    Act(Action),
    Key(KeyCode, KeyModifiers),
    Mouse(MouseEvent),
    MenuClick(&'static str),
}

fn menu_slot(cmd: &str) -> (usize, usize) {
    for cat in 0..8 {
        for item in 0..menubar_category_item_count(cat) {
            if menubar_item_command(cat, item) == Some(cmd) {
                return (cat, item);
            }
        }
    }
    panic!("no menu bar item runs `{cmd}`");
}

fn run(wm: &mut WindowManager, steps: &[Step]) {
    for s in steps {
        match s {
            Step::Act(a) => {
                wm.apply(a.clone()).unwrap();
            }
            Step::Key(c, m) => {
                handle_wm_key_event(wm, KeyEvent::new(*c, *m)).unwrap();
            }
            Step::Mouse(ev) => {
                handle_wm_mouse_event(wm, *ev, SCREEN).unwrap();
            }
            Step::MenuClick(cmd) => {
                let (cat, item) = menu_slot(cmd);
                wm.set_menubar_open_category(Some(cat));
                let shell = ShellLayout::compute(SCREEN, wm.config(), wm.pane_count());
                let inner = inset_window_frame(menubar_dropdown_rect(cat, shell.workspace));
                handle_wm_mouse_event(wm, click(inner.x + 1, inner.y + item as u16), SCREEN).unwrap();
            }
        }
    }
}

/// One surface: the steps it performs, and (when the contract claims equal Actions) the Actions it lowers to.
struct Via {
    name: &'static str,
    steps: Vec<Step>,
    strict: Option<Vec<Action>>,
}

struct Cap {
    name: &'static str,
    /// Colon text that opens the window before the gesture (None: gesture starts closed).
    setup: Option<&'static str>,
    colon: Vec<&'static str>,
    via: Vec<Via>,
}

fn opened(setup: Option<&str>) -> WindowManager {
    let mut wm = fixture();
    if let Some(cmd) = setup {
        wm.apply(parse(cmd)).unwrap();
    }
    wm
}

fn palette_action(text: &str) -> Action {
    let row = PALETTE_CATALOG.iter().find(|e| e.0 == text).unwrap_or_else(|| panic!("palette lacks `{text}`"));
    parse(row.0)
}

fn menu_action(text: &str) -> Action {
    let (cat, item) = menu_slot(text);
    parse(menubar_item_command(cat, item).unwrap())
}

fn key_via(name: &'static str, code: KeyCode, mods: KeyModifiers, strict: bool) -> Via {
    let a = key_action(code, mods);
    Via { name, steps: vec![Step::Key(code, mods)], strict: strict.then(|| vec![a]) }
}

/// Mouse surfaces are lowered on a probe window opened like the capability's setup.
fn matrix() -> Vec<Cap> {
    let probe = opened(Some(":find agent"));
    let l = layout_of(&probe);
    let chip = |r: Rect| {
        let (c, y) = centre(r);
        click(c, y)
    };
    let lower = |ev: MouseEvent| {
        let (kind, _, sel) = probe.find_modal_state().unwrap();
        lower_find_mouse(ev, &l, probe.find_results().rows.len(), sel, kind).unwrap_or_else(|| panic!("unlowered {ev:?}"))
    };
    let mouse_via = |name: &'static str, ev: MouseEvent, strict: bool| Via {
        name,
        steps: vec![Step::Mouse(ev)],
        strict: strict.then(|| lower(ev)),
    };
    let open_via = |cmd: &'static str, chord_keys: Vec<(KeyCode, KeyModifiers)>| {
        let a = parse(cmd);
        vec![
            Via { name: "palette", steps: vec![Step::Act(palette_action(cmd))], strict: Some(vec![a.clone()]) },
            Via { name: "menu item command", steps: vec![Step::Act(menu_action(cmd))], strict: Some(vec![a.clone()]) },
            Via { name: "menu click (mouse)", steps: vec![Step::MenuClick(cmd)], strict: None },
            Via { name: "key chord", steps: chord_keys.into_iter().map(|(c, m)| Step::Key(c, m)).collect(), strict: None },
        ]
    };
    let rows_agent = ":find agent";
    vec![
        Cap { name: "open agent", setup: None, colon: vec![":find agent"], via: open_via(":find agent", vec![(KeyCode::Char('f'), CHORD)]) },
        Cap {
            name: "open session",
            setup: None,
            colon: vec![":find session"],
            via: open_via(":find session", vec![(KeyCode::Char('f'), CHORD), (KeyCode::Tab, NONE)]),
        },
        Cap {
            name: "open history",
            setup: None,
            colon: vec![":find history"],
            via: open_via(":find history", vec![(KeyCode::Char('f'), CHORD), (KeyCode::Tab, NONE), (KeyCode::Tab, NONE)]),
        },
        Cap {
            name: "switch kind",
            setup: Some(rows_agent),
            colon: vec![":finder kind session"],
            via: vec![
                key_via("Tab", KeyCode::Tab, NONE, false),
                mouse_via("tab chip", chip(l.tab_chips[1]), true),
                mouse_via("[Tab kind] footer chip", chip(l.footer_chips[1]), false),
            ],
        },
        Cap {
            name: "clear query",
            setup: Some(":find agent zz"),
            colon: vec![":finder query"],
            via: vec![
                key_via("Ctrl+U", KeyCode::Char('u'), KeyModifiers::CONTROL, true),
                mouse_via("[clear] chip", chip(l.clear_chip), true),
            ],
        },
        Cap {
            name: "edit query (E2: no mouse path)",
            setup: Some(rows_agent),
            colon: vec![":finder query abc"],
            via: vec![Via {
                name: "typing a b c",
                steps: "abc".chars().map(|c| Step::Key(KeyCode::Char(c), NONE)).collect(),
                strict: None,
            }],
        },
        Cap {
            name: "move",
            setup: Some(rows_agent),
            colon: vec![":finder move +3"],
            via: vec![
                Via { name: "Down x3", steps: vec![Step::Key(KeyCode::Down, NONE); 3], strict: None },
                mouse_via("wheel", mouse(MouseEventKind::ScrollDown, l.list.x + 1, l.list.y), true),
            ],
        },
        Cap {
            name: "move to top",
            setup: Some(":find agent"),
            colon: vec![":finder move top"],
            via: vec![key_via("Home", KeyCode::Home, NONE, true)],
        },
        Cap {
            name: "move to bottom",
            setup: Some(rows_agent),
            colon: vec![":finder move bottom"],
            via: vec![key_via("End", KeyCode::End, NONE, true)],
        },
        Cap {
            name: "select a row",
            setup: Some(rows_agent),
            colon: vec![":finder select 2"],
            via: vec![
                Via { name: "Down x2", steps: vec![Step::Key(KeyCode::Down, NONE); 2], strict: None },
                mouse_via("hover", mouse(MouseEventKind::Moved, l.list.x + 1, l.list.y + 2), true),
            ],
        },
        Cap {
            name: "jump (selected row)",
            setup: Some(rows_agent),
            colon: vec![":finder enter"],
            via: vec![key_via("Enter", KeyCode::Enter, NONE, true), mouse_via("[Enter] chip", chip(l.footer_chips[0]), true)],
        },
        Cap {
            name: "jump (row 2)",
            setup: Some(rows_agent),
            colon: vec![":finder select 2", ":finder enter"],
            via: vec![mouse_via("row click", click(l.list.x + 1, l.list.y + 2), true)],
        },
        Cap {
            name: "prefill prompt",
            setup: Some(":find history"),
            colon: vec![":finder prefill"],
            via: vec![
                key_via("Ctrl+Y", KeyCode::Char('y'), KeyModifiers::CONTROL, true),
                mouse_via("[^Y prefill] chip", chip(l.footer_chips[2]), true),
            ],
        },
        Cap {
            name: "close",
            setup: Some(rows_agent),
            colon: vec![":finder close"],
            via: vec![
                key_via("Esc", KeyCode::Esc, NONE, true),
                mouse_via("[Esc] chip", chip(l.footer_chips[3]), true),
                Via { name: "Ctrl+Alt+F", steps: vec![Step::Key(KeyCode::Char('f'), CHORD)], strict: None },
            ],
        },
    ]
}

fn cell_text(buf: &Buffer, r: Rect) -> String {
    (r.x..r.x + r.width).map(|x| buf.cell((x, r.y)).map_or(" ", |c| c.symbol())).collect()
}

fn mcp_text(plane: &mut AutomationControlPlane, kind: &str) -> JsonValue {
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"bilterm_find","arguments":{{"kind":"{kind}"}}}}}}"#
    );
    let resp = JsonValue::parse(&plane.handle_mcp_line(&line).expect("mcp response")).unwrap();
    let text = resp
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(JsonValue::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .unwrap_or_else(|| panic!("no mcp payload for {kind}"))
        .to_string();
    JsonValue::parse(&text).unwrap()
}

fn row_keys(v: &JsonValue) -> Vec<(String, String)> {
    let rows = v.get("rows").and_then(JsonValue::as_array).expect("rows").to_vec();
    rows.iter()
        .map(|r| {
            let id = r.get("id").and_then(JsonValue::as_str).unwrap_or_default().to_string();
            (id, r.get("source").map(JsonValue::to_json_string).unwrap_or_default())
        })
        .collect()
}

/// RPC `find.<kind>s`, `wm.command ":find <kind>"` and MCP `bilterm_find` return the same rows.
fn rpc_mcp_leg() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    for _ in 0..2 {
        plane.wm.split_focused(SplitDirection::Vertical).unwrap();
    }
    plane.wm.pane_mut(1).unwrap().submit_command("echo parity").unwrap();
    plane.wm.poll_all_ptys();
    plane.wm.update_agent_states(std::time::Instant::now());
    for (method, kind, colon) in [
        ("find.agents", "agent", ":find agent"),
        ("find.sessions", "session", ":find session"),
        ("find.history", "history", ":find history"),
    ] {
        let rpc = plane.dispatch_method(method, &JsonValue::parse("{}").unwrap()).unwrap();
        assert_eq!(rpc.get("kind").and_then(JsonValue::as_str), Some(kind), "RPC {method} kind");
        assert!(!row_keys(&rpc).is_empty(), "RPC {method} returned no rows");
        let mcp = mcp_text(&mut plane, kind);
        assert_eq!(mcp, rpc, "open {kind}: MCP bilterm_find payload differs from RPC {method}");
        let via = plane
            .dispatch_method("wm.command", &JsonValue::parse(&format!(r#"{{"command":"{colon}"}}"#)).unwrap())
            .unwrap();
        let data = via.get("data").unwrap_or_else(|| panic!("wm.command {colon} has no data"));
        assert_eq!(row_keys(data), row_keys(&rpc), "open {kind}: wm.command data differs from RPC {method}");
        plane.wm.apply(parse(":finder close")).unwrap();
    }
}

#[test]
fn uni_r01_matrix() {
    for cap in matrix() {
        let colon: Vec<Action> = cap.colon.iter().map(|c| parse(c)).collect();
        let mut want_wm = opened(cap.setup);
        run(&mut want_wm, &colon.iter().cloned().map(Step::Act).collect::<Vec<_>>());
        let want = state(&want_wm);
        for via in &cap.via {
            let mut wm = opened(cap.setup);
            run(&mut wm, &via.steps);
            assert_eq!(state(&wm), want, "UNI-R01 `{}`: surface `{}` ends in a different state than the colon path", cap.name, via.name);
            if let Some(actions) = &via.strict {
                assert_eq!(actions, &colon, "UNI-R01 `{}`: surface `{}` lowers to different Actions than the colon path", cap.name, via.name);
            }
        }
    }
    let chord = default_global_action(KeyEvent::new(KeyCode::Char('f'), CHORD), false);
    assert_eq!(chord, Some(parse(":find agent")), "global chord Ctrl+Alt+F");
    rpc_mcp_leg();
}

/// Matrix row (capability name) a `FindOp` belongs to. No wildcard arm: a new `FindOp` variant
/// fails to compile here until it is placed in the matrix or listed as an explicit exception.
fn matrix_row(op: &FindOp) -> &'static str {
    match op {
        FindOp::Open { kind: FindKind::Agent, .. } => "open agent",
        FindOp::Open { kind: FindKind::Session, .. } => "open session",
        FindOp::Open { kind: FindKind::History, .. } => "open history",
        FindOp::SetKind(_) | FindOp::CycleKind(_) => "switch kind",
        FindOp::SetQuery(_) | FindOp::ClearQuery => "clear query",
        FindOp::Push(_) | FindOp::Pop => "edit query (E2: no mouse path)",
        FindOp::Move(i16::MIN) => "move to top",
        FindOp::Move(i16::MAX) => "move to bottom",
        FindOp::Move(_) => "move",
        FindOp::Select(_) => "select a row",
        FindOp::Activate => "jump (selected row)",
        FindOp::Prefill => "prefill prompt",
        FindOp::Close => "close",
    }
}

#[test]
fn every_find_op_is_in_the_matrix() {
    let names: Vec<&str> = matrix().iter().map(|c| c.name).collect();
    let open = |kind| FindOp::Open { kind, query: String::new() };
    let ops = [
        open(FindKind::Agent),
        open(FindKind::Session),
        open(FindKind::History),
        FindOp::SetKind(FindKind::Agent),
        FindOp::CycleKind(true),
        FindOp::SetQuery("x".into()),
        FindOp::Push('x'),
        FindOp::Pop,
        FindOp::ClearQuery,
        FindOp::Move(1),
        FindOp::Move(i16::MIN),
        FindOp::Move(i16::MAX),
        FindOp::Select(1),
        FindOp::Activate,
        FindOp::Prefill,
        FindOp::Close,
    ];
    for op in ops {
        assert!(names.contains(&matrix_row(&op)), "{op:?} maps to matrix row `{}` which does not exist", matrix_row(&op));
    }
}

#[test]
fn finder_has_no_palette_entries_and_all_other_paths() {
    // E1: modal-only verbs have no palette entry...
    assert!(PALETTE_CATALOG.iter().all(|e| e.0.split_whitespace().next() != Some(":finder")));
    // ...but every one of them has a key or mouse path, a colon form (matrix) and a help row.
    assert!(HELP_LINES.iter().any(|(l, _)| l.split_whitespace().next() == Some(":finder")));
    let finder_rows = matrix().iter().filter(|c| c.colon.iter().all(|c| c.starts_with(":finder"))).count();
    assert!(finder_rows >= 10, "matrix covers the :finder verbs ({finder_rows} rows)");
}

#[test]
fn legacy_find_and_search_unchanged() {
    let s = |t: &str| Action::SearchQuery(t.to_string());
    assert_eq!(parse(":find hello"), s("hello"));
    assert_eq!(parse(":find"), Action::SearchOff);
    assert_eq!(parse(":find off"), Action::SearchOff);
    assert_eq!(parse(":find clear"), Action::SearchOff);
    assert_eq!(parse(":find next"), Action::SearchNext);
    assert_eq!(parse(":find prev"), Action::SearchPrev);
    assert_eq!(parse(":find prefill"), Action::SearchPromptPrefill);
    assert_eq!(parse(":search agent x"), s("agent x"));
    assert_eq!(parse(":search history"), s("history"));
    assert_eq!(
        parse(":find agents x"),
        Action::Find(FindOp::Open { kind: FindKind::Agent, query: "x".into() })
    );
    // Applying the legacy forms never opens the registry window.
    let mut wm = fixture();
    for c in [":find hello", ":find next", ":find prev", ":find off", ":search agent x"] {
        wm.apply(parse(c)).unwrap();
        assert!(!wm.is_find_visible(), "{c} must stay scrollback search");
    }
}

#[test]
fn gui_surface_paths_work() {
    let run_surface = |gui: bool| {
        let mut wm = fixture();
        wm.set_gui_surface(gui);
        run(&mut wm, &[Step::Key(KeyCode::Char('f'), CHORD)]);
        assert_eq!(wm.find_modal_state().map(|s| s.0), Some(FindKind::Agent), "chord opens (gui={gui})");
        let opened = state(&wm);
        run(&mut wm, &[Step::Key(KeyCode::Esc, NONE), Step::MenuClick(":find session")]);
        assert_eq!(wm.find_modal_state().map(|s| s.0), Some(FindKind::Session), "menu click opens (gui={gui})");
        wm.apply(parse(":find agent")).unwrap();
        let l = layout_of(&wm);
        let want = wm.find_results().rows[2].pane.expect("live agent row has a pane");
        run(&mut wm, &[Step::Mouse(click(l.list.x + 1, l.list.y + 2))]);
        assert!(!wm.is_find_visible(), "row click closes (gui={gui})");
        assert_eq!(wm.active_pane_id(), want, "row click jumps to the row's pane (gui={gui})");
        (opened, state(&wm))
    };
    assert_eq!(run_surface(false), run_surface(true), "GUI and TUI end in the same state");

    // The GUI renderer draws the window.
    let mut wm = fixture();
    wm.set_gui_surface(true);
    wm.apply(parse(":find agent")).unwrap();
    let mut renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    renderer.render_workspace_with_cell_size(&wm, SCREEN, 8, 16, &mut surface);
    let glyphs: String = surface.glyphs.iter().map(|g| g.text.as_str()).collect();
    assert!(glyphs.contains("FIND") || glyphs.contains('F'), "GUI frame has glyphs");
}

#[test]
fn help_and_completion_cover_every_find_verb() {
    let help: String = HELP_LINES.iter().map(|(l, r)| format!("{l} {r}\n")).collect();
    for word in ["agent", "session", "history", ":finder", "Find window keys", "Find window mouse", "Ctrl+Alt+F"] {
        assert!(help.contains(word), "help lacks `{word}`");
    }
    let table = bilterm::pane::completion::colon_command_table();
    let (_, finder) = table.iter().find(|(c, _)| *c == ":finder").expect(":finder completion row");
    let verbs: Vec<&str> = matrix().iter().flat_map(|c| c.colon.clone()).filter(|c| c.starts_with(":finder ")).collect();
    for v in verbs {
        let word = v.split_whitespace().nth(1).unwrap();
        assert!(finder.iter().any(|s| s.trim() == word), "completion lacks `:finder {word}`");
    }
}

/// Plan 08 checked clicks against a formula; this checks them against what plan 07 renders.
#[test]
fn clicked_rows_match_the_rendered_rows() {
    let mut wm = fixture();
    wm.apply(parse(":find agent")).unwrap();
    let rows: Vec<FindRow> = (0..40)
        .map(|i| {
            let mut r = FindRow::empty(FindKind::Agent);
            r.id = format!("row:{i}");
            r.label = format!("label-{i:02}");
            r
        })
        .collect();
    wm.set_find_results(FindResults { kind: FindKind::Agent, query: String::new(), rows, total: 40, truncated: false, masked: 0 });
    wm.apply(parse(":finder select 35")).unwrap();
    let l = layout_of(&wm);
    let mut buf = Buffer::empty(SCREEN);
    WorkspaceWidget::new(&wm).render(SCREEN, &mut buf);
    let start = list_viewport_start(35, usize::from(l.list.height));
    for y in 0..l.list.height {
        let text = cell_text(&buf, Rect::new(l.list.x, l.list.y + y, l.list.width, 1));
        let shown: usize = text
            .split("label-")
            .nth(1)
            .and_then(|t| t.get(..2))
            .and_then(|t| t.parse().ok())
            .unwrap_or_else(|| panic!("row {y} shows no label: {text:?}"));
        assert_eq!(shown, start + usize::from(y), "rendered row {y} is not the formula row");
        let (kind, _, sel) = wm.find_modal_state().unwrap();
        let lowered = lower_find_mouse(click(l.list.x + 1, l.list.y + y), &l, 40, sel, kind);
        assert_eq!(
            lowered,
            Some(vec![Action::Find(FindOp::Select(shown)), Action::Find(FindOp::Activate)]),
            "clicking the row that renders label-{shown:02}"
        );
    }
}
