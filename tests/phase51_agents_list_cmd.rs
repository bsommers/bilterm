//! Phase 51-08: `:agents list` executor (status + structured data), grammar hardening / fuzz and the
//! menu-bar mouse path for the list, demo, stop demo and tile-all entries (UIT-03, UNI-R01).

use bilterm::action::Action;
use bilterm::api::JsonValue;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::modals::{menubar_category_item_count, menubar_dropdown_rect, menubar_item_command};
use bilterm::ui::shell::ShellLayout;
use bilterm::wm::agents_cmd::AgentsOp;
use bilterm::wm::agents_list_cmd::AgentsListOp;
use bilterm::wm::{SplitDirection, WindowManager, inset_window_frame};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const COLS: u16 = 120;
const ROWS: u16 = 40;

fn fixture() -> WindowManager {
    let mut wm = WindowManager::new_headless(ROWS, COLS);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(Rect::new(0, 0, COLS, ROWS)).unwrap();
    wm
}

fn parse(cmd: &str) -> Result<Action, String> {
    parse_colon_command(cmd).unwrap_or_else(|| panic!("not a colon command: {cmd}"))
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let action = parse(cmd).unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(action).unwrap();
}

fn status(wm: &WindowManager) -> Option<String> {
    wm.active_pane().status_message().map(str::to_string)
}

fn data(wm: &mut WindowManager) -> JsonValue {
    JsonValue::parse(&wm.take_command_data().expect("command data")).expect("valid json")
}

/// Every verb of the contract grammar (section 3), one canonical sample each.
const VERBS: &[&str] = &[
    "open", "close", "toggle", "dismiss", "blur", "select up", "select down", "select first", "select last",
    "select page-up", "select page-down", "select 1", "jump", "jump 1", "zoom", "zoom #1", "pin", "unpin",
    "pin toggle", "scale 100", "scale 150%", "scale +10", "scale -10", "filter all", "filter agents",
    "filter toggle", "dock", "dock toggle", "float", "move 1 -1", "resize 2 2", "rect 1 1 40 12", "size 40x12",
    "size auto", "scroll 3", "scroll -3",
];

fn assert_payload_matches_summary(wm: &mut WindowManager, label: &str) {
    let summary = wm.agents_list_summary();
    let d = data(wm);
    for key in ["active", "pinned", "focused", "docked", "filter", "selected", "row_count", "scale_pct"] {
        assert!(d.get(key).is_some(), "{label}: missing key {key}");
    }
    assert_eq!(d.get("active").and_then(JsonValue::as_bool), Some(summary.active), "{label}");
    assert_eq!(d.get("pinned").and_then(JsonValue::as_bool), Some(summary.pinned), "{label}");
    assert_eq!(d.get("focused").and_then(JsonValue::as_bool), Some(summary.focused), "{label}");
    assert_eq!(d.get("docked").and_then(JsonValue::as_bool), Some(summary.docked), "{label}");
    assert_eq!(d.get("row_count").and_then(JsonValue::as_usize), Some(summary.row_count), "{label}");
    assert_eq!(d.get("scale_pct").and_then(JsonValue::as_i64), Some(i64::from(summary.scale_pct)), "{label}");
    assert_eq!(
        d.get("selected").and_then(JsonValue::as_i64),
        summary.selected.map(|id| i64::try_from(id).unwrap()),
        "{label}"
    );
    assert!(matches!(d.get("filter").and_then(JsonValue::as_str), Some("all" | "agents")), "{label}");
}

#[test]
fn list_data_payload() {
    for verb in VERBS {
        if verb.starts_with("scale ") && !verb.contains(['+', '-']) {
            continue; // absolute scale lowers to the generic `PanelScaleOf` executor (51-01), no list payload
        }
        let mut wm = fixture();
        run_colon(&mut wm, ":agents list open");
        let _ = wm.take_command_data();
        run_colon(&mut wm, &format!(":agents list {verb}"));
        assert_payload_matches_summary(&mut wm, verb);
    }
    // Open reflects in the payload; names never appear in it (T-51-21).
    let mut wm = fixture();
    run_colon(&mut wm, ":agents list open");
    let raw = wm.take_command_data().unwrap();
    let d = JsonValue::parse(&raw).unwrap();
    assert_eq!(d.get("active").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(d.get("focused").and_then(JsonValue::as_bool), Some(true));
    assert_eq!(d.get("row_count").and_then(JsonValue::as_usize), Some(2));
    assert!(!raw.contains("name"), "summary carries scalars only: {raw}");
}

#[test]
fn list_status_text_shown() {
    let mut wm = fixture();
    // A verb whose apply returns None leaves the status untouched.
    wm.active_pane_mut().set_status_message(Some("keep me".to_string()));
    run_colon(&mut wm, ":agents list open");
    assert_eq!(status(&wm).as_deref(), Some("keep me"));
    // The executor shows exactly the message `agents_list_apply` returns.
    for verb in VERBS {
        let op = match parse(&format!(":agents list {verb}")) {
            Ok(Action::AgentsList(op)) => op,
            _ => continue,
        };
        let mut probe = fixture();
        probe.open_agents_list();
        let expect = probe.agents_list_apply(&op);
        let mut wm = fixture();
        wm.open_agents_list();
        wm.active_pane_mut().set_status_message(Some("before".to_string()));
        run_colon(&mut wm, &format!(":agents list {verb}"));
        match expect {
            Some(msg) => assert_eq!(status(&wm), Some(msg), "{verb}"),
            None => assert_eq!(status(&wm).as_deref(), Some("before"), "{verb}"),
        }
    }
}

#[test]
fn bare_list_payload_unchanged_plus_window_key() {
    for cmd in [":agents list", ":agents", ":agents ls", ":agents status"] {
        let mut wm = fixture();
        run_colon(&mut wm, cmd);
        let list = wm.agents_list();
        assert!(list.active && list.focused, "{cmd} opens and focuses the window (D-03)");
        let d = data(&mut wm);
        let agents = d.get("agents").and_then(JsonValue::as_array).expect("agents array");
        assert_eq!(agents.len(), 2, "{cmd}");
        for key in ["pane_id", "name", "state", "since_ms", "profile", "reported", "partners"] {
            assert!(agents[0].get(key).is_some(), "{cmd}: agent key {key}");
        }
        let counts = d.get("counts").expect("counts");
        for key in ["working", "blocked", "idle", "done", "errored"] {
            assert!(counts.get(key).and_then(JsonValue::as_i64).is_some(), "{cmd}: counts.{key}");
        }
        let traffic = d.get("traffic").expect("traffic");
        for key in ["latest_seq", "len", "dropped"] {
            assert!(traffic.get(key).and_then(JsonValue::as_i64).is_some(), "{cmd}: traffic.{key}");
        }
        let window = d.get("agents_list").expect("additive agents_list key");
        assert_eq!(window.get("active").and_then(JsonValue::as_bool), Some(true), "{cmd}");
        assert_eq!(window.get("focused").and_then(JsonValue::as_bool), Some(true), "{cmd}");
        assert_eq!(window.get("row_count").and_then(JsonValue::as_usize), Some(2), "{cmd}");
    }
}

/// Deterministic xorshift so the fuzz is reproducible without a dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick<'a>(&mut self, items: &'a [String]) -> &'a str {
        &items[(self.next() % items.len() as u64) as usize]
    }
}

#[test]
fn grammar_fuzz_never_panics() {
    let mut tokens: Vec<String> = [
        "open", "close", "toggle", "dismiss", "blur", "select", "up", "down", "first", "last", "page-up",
        "page-down", "jump", "zoom", "pin", "unpin", "scale", "filter", "all", "agents", "dock", "float",
        "move", "resize", "rect", "size", "auto", "scroll", "stop", "list", "demo", "0", "1", "75", "200",
        "65535", "65536", "-1", "#3", "3x4", "+10", "20x6", "400x200", "401x200", "%", "100%", "", "٣", "１２",
        "\u{7}", "\t", "\u{0}", "-", "+", "x", "##3", "4294967295", "4294967296", "-32768", "32768",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tokens.push("9".repeat(4096));
    tokens.push("x".repeat(4096));
    tokens.push("é".repeat(2048));
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut ok, mut err) = (0usize, 0usize);
    for _ in 0..20_000 {
        let head = if rng.next().is_multiple_of(4) { ":agents demo" } else { ":agents list" };
        let mut line = head.to_string();
        for _ in 0..(rng.next() % 6) {
            line.push(' ');
            line.push_str(rng.pick(&tokens));
        }
        let Some(parsed) = parse_colon_command(&line) else { continue };
        match parsed {
            Ok(Action::AgentsList(op)) => {
                ok += 1;
                let form = op.colon_form();
                assert_eq!(parse(&form), Ok(Action::AgentsList(op)), "round trip of {line:?} via {form:?}");
            }
            Ok(_) => ok += 1,
            Err(_) => err += 1,
        }
    }
    assert!(ok > 50 && err > 50, "fuzz must reach both outcomes (ok {ok}, err {err})");
}

#[test]
fn grammar_edge_numbers() {
    let must_err = [
        "scale 74", "scale 201", "scale 0", "scale 65536", "scale -", "scale +", "scale 99999",
        "rect 0 0 0", "rect 1 2 3 65536", "rect -1 0 10 10", "size 0x0", "size 19x6", "size 20x5", "size 401x6",
        "size 65535x65535", "size 65536x6", "size 20x6x1", "move 40000 0", "move 1", "resize 0 40000",
        "scroll 99999", "scroll -99999", "scroll x", "select 4294967296", "select -1", "jump 4294967296",
        "jump ##1", "zoom 1 2", "scale ٣٠٠", "scale １００", "filter", "filter nope", "dock now", "pin maybe",
        "bogus",
    ];
    for tail in must_err {
        let line = format!(":agents list {tail}");
        let err = parse(&line).expect_err(&line);
        assert!(err.starts_with("Usage: :agents list"), "{line}: {err}");
    }
    for (tail, expect) in [
        ("scale 75", ":panel scale"),
        ("scale 200%", ":panel scale"),
        ("rect 0 0 0 0", "rect 0 0 0 0"),
        ("size 65535x65535", ""),
    ] {
        let line = format!(":agents list {tail}");
        match (parse(&line), expect) {
            (Ok(Action::AgentsList(op)), e) if !e.is_empty() => assert_eq!(op.colon_form(), format!(":agents list {tail}")),
            (Ok(Action::PanelScaleOf(..)), ":panel scale") => {}
            (Err(_), "") => {}
            other => panic!("{line}: {other:?}"),
        }
    }
    // Usage names every verb.
    let usage = parse(":agents list bogus").unwrap_err();
    for verb in [
        "open", "close", "toggle", "dismiss", "blur", "select", "jump", "zoom", "pin", "unpin", "scale",
        "filter", "dock", "float", "move", "resize", "rect", "size", "auto", "scroll",
    ] {
        assert!(usage.contains(verb), "usage misses `{verb}`: {usage}");
    }
    // `:agents demo` stays strict.
    assert!(parse(":agents demo now").is_err());
    assert_eq!(parse(":agents demo stop"), Ok(Action::Agents(AgentsOp::DemoStop)));
}

#[test]
fn list_ops_on_closed_window() {
    for verb in VERBS {
        let mut wm = fixture();
        run_colon(&mut wm, &format!(":agents list {verb}"));
        let _ = wm.take_command_data();
    }
    // Scroll beyond everything on a closed window is a clean no-op.
    let mut wm = fixture();
    run_colon(&mut wm, ":agents list scroll 32767");
    assert!(!wm.agents_list().active);
    assert_payload_matches_summary(&mut wm, "scroll on closed");
}

// ---- menu entries (Task 2) ----

const VIEW: usize = 3;
const WINDOW: usize = 1;

fn find_entry(cmd: &str) -> (usize, usize) {
    (0..8)
        .flat_map(|c| (0..menubar_category_item_count(c).min(32)).map(move |i| (c, i)))
        .find(|&(c, i)| menubar_item_command(c, i) == Some(cmd))
        .unwrap_or_else(|| panic!("no menu entry for {cmd}"))
}

fn key_pick(wm: &mut WindowManager, (cat, item): (usize, usize)) {
    wm.set_menubar_open_category(Some(cat));
    for _ in 0..item {
        handle_wm_key_event(wm, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap();
    }
    handle_wm_key_event(wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(wm.menubar_state().is_none(), "the menu closes after the pick");
}

fn mouse_pick(wm: &mut WindowManager, (cat, item): (usize, usize)) {
    let screen = Rect::new(0, 0, COLS, ROWS);
    let workspace = ShellLayout::compute(screen, wm.config(), wm.pane_count()).workspace;
    wm.set_menubar_open_category(Some(cat));
    let inner = inset_window_frame(menubar_dropdown_rect(cat, workspace));
    let ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: inner.x + 2,
        row: inner.y + item as u16,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(wm, ev, screen).unwrap();
    assert!(wm.menubar_state().is_none(), "mouse pick closes the menu: {cat}/{item} click {:?} in {:?} ws {workspace:?} state {:?}", (ev.column, ev.row), inner, wm.menubar_state());
}

#[test]
fn menu_entries_lower_to_actions() {
    let want: [(&str, usize, Action); 4] = [
        (":agents list toggle", VIEW, Action::AgentsList(AgentsListOp::Toggle)),
        (":agents demo", VIEW, Action::Agents(AgentsOp::Demo)),
        (":agents demo stop", VIEW, Action::Agents(AgentsOp::DemoStop)),
        (":arrange grid", WINDOW, Action::LayoutProfileCommand("arrange grid".to_string())),
    ];
    for (cmd, cat, action) in want {
        let (c, i) = find_entry(cmd);
        assert_eq!(c, cat, "{cmd} lives in the expected category");
        assert_eq!(menubar_item_command(c, i), Some(cmd));
        assert_eq!(parse(cmd), Ok(action), "{cmd}");
    }
}

#[test]
fn menu_pick_by_keyboard() {
    let mut wm = fixture();
    let list = find_entry(":agents list toggle");
    key_pick(&mut wm, list);
    assert!(wm.agents_list().active, "first pick opens the list");
    key_pick(&mut wm, list);
    assert!(!wm.agents_list().active, "second pick closes it");

    let before = wm.pane_count();
    key_pick(&mut wm, find_entry(":agents demo"));
    assert!(wm.pane_count() >= before.max(3), "demo adds panes up to three");
    key_pick(&mut wm, find_entry(":agents demo stop"));

    let mut wm = fixture();
    key_pick(&mut wm, find_entry(":arrange grid"));
    assert_eq!(wm.pane_count(), 2, "tiling keeps every pane");
}

#[test]
fn menu_pick_by_mouse() {
    let mut wm = fixture();
    let list = find_entry(":agents list toggle");
    mouse_pick(&mut wm, list);
    assert!(wm.agents_list().active, "click opens the list");
    mouse_pick(&mut wm, list);
    assert!(!wm.agents_list().active, "click again closes it");

    let before = wm.pane_count();
    mouse_pick(&mut wm, find_entry(":agents demo"));
    assert!(wm.pane_count() >= before.max(3), "demo adds panes up to three");
    // The demo opens the traffic window over the dropdown; an open menu still gets the click first.
    assert!(wm.agents_window().active, "demo opened the traffic window");
    mouse_pick(&mut wm, find_entry(":agents demo stop"));

    let mut wm = fixture();
    mouse_pick(&mut wm, find_entry(":arrange grid"));
    assert_eq!(wm.pane_count(), 2);
}

#[test]
fn menu_existing_entries_unchanged() {
    // Literal copy of the table at the start of 51-08; new entries are appended only.
    let table: [&[&str]; 7] = [
        &[":inspector", ":session new", ":session save", ":session restore", ":session info", ":session processes", ":quit"],
        &[
            ":split h", ":split v", ":zoom", ":layout stack", ":window theme list", ":window menu", ":window theme next",
            ":window outline next", ":window bold toggle", ":window reset",
        ],
        &[":prompt top", ":prompt bottom", ":prompt split", ":mode cycle"],
        &[
            ":hints", ":copy", ":rect", ":rect mode", ":crosshair", ":fkeybar", ":agents toggle", ":find agent",
            ":find session", ":find history",
        ],
        &[":theme turbo-blue", ":theme turbo-cyan", ":theme 3278", ":theme 3279"],
        &[":broadcast", ":lock", ":quake", ":raw"],
        &[":help", ":inspector", ":session list", ":help"],
    ];
    let appended = [0usize, 1, 0, 9, 0, 4, 0];
    for (cat, cmds) in table.iter().enumerate() {
        assert_eq!(menubar_category_item_count(cat), cmds.len() + appended[cat], "category {cat} count");
        for (i, cmd) in cmds.iter().enumerate() {
            assert_eq!(menubar_item_command(cat, i), Some(*cmd), "category {cat} item {i}");
        }
    }
    assert_eq!(menubar_category_item_count(2), 4, "Prompt keeps 4 items");
    // The tallest dropdown still fits a 24-row terminal.
    let screen = Rect::new(0, 0, 80, 24);
    let workspace = ShellLayout::compute(screen, &bilterm::config::BiltermConfig::default(), 1).workspace;
    for cat in 0..7 {
        let r = menubar_dropdown_rect(cat, workspace);
        assert!(r.y + r.height <= screen.height, "category {cat} dropdown {r:?} fits 24 rows");
        assert_eq!(r.height, menubar_category_item_count(cat) as u16 + 2, "category {cat} is not clipped");
    }
}
