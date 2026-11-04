//! Phase 50-07: AGENT TRAFFIC zoom window (FLT-01, FLT-02). `window_` = tracer slice,
//! `layout_` = graph placement, `graph_` = selection model, `render_` = frame output.

use std::collections::BTreeMap;
use std::time::Instant;

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::handle_wm_key_event;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::design::DesignTokens;
use bilterm::wm::agents_window::{GraphDir, GraphLayout, NodeKey};
use bilterm::wm::traffic::{TrafficEndpoint, TrafficKind};
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agents_cmd::AgentsOp;
use bilterm::wm::{SplitDirection, WindowManager};
use std::sync::Mutex;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

/// Two-pane headless fixture.
fn fixture(cols: u16, rows: u16) -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(rows, cols);
    let area = Rect::new(0, 0, cols, rows);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
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

fn is_braille(sym: &str) -> bool {
    sym.chars().next().is_some_and(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
}

#[test]
fn window_tracer_render_open_close() {
    let (mut wm, area) = fixture(120, 40);
    wm.broadcast_command("echo hi").unwrap();
    wm.open_agents_window(None);
    let rect = wm.agents_window_rect(area).expect("window rect");
    let buf = render(&wm, area);
    let text = lines(&buf).join("\n");

    assert!(text.contains("AGENT TRAFFIC"), "missing window title");
    for label in ["you", "#1", "#2"] {
        assert!(text.contains(label), "missing node label {label}");
    }
    assert!(
        lines(&buf).iter().any(|l| l.contains("echo hi") && l.contains("broadcast")),
        "missing events line for the broadcast"
    );
    let graph_has_braille = (rect.y + 1..rect.y + rect.height - 1)
        .flat_map(|y| (rect.x + 1..rect.x + rect.width / 2).map(move |x| (x, y)))
        .any(|(x, y)| is_braille(buf[(x, y)].symbol()));
    assert!(graph_has_braille, "no braille cell inside the graph pane");

    wm.close_agents_window();
    assert!(!wm.agents_window().active && !wm.agents_window().focused);
    let after = lines(&render(&wm, area)).join("\n");
    assert!(!after.contains("AGENT TRAFFIC"), "window must disappear when closed");
}

fn ring(n: usize) -> (Vec<NodeKey>, Vec<(usize, usize)>) {
    let mut keys: Vec<NodeKey> = (0..n).map(|i| NodeKey(format!("session:s{i:02}"))).collect();
    keys.sort();
    let edges = (0..n).map(|i| (i, (i + 1) % n)).collect();
    (keys, edges)
}

fn by_key(keys: &[NodeKey], pos: &[(f32, f32)]) -> BTreeMap<NodeKey, (f32, f32)> {
    keys.iter().cloned().zip(pos.iter().copied()).collect()
}

#[test]
fn layout_deterministic_bit_for_bit() {
    let (keys, edges) = ring(12);
    let a = GraphLayout::default().place(&keys, &edges);
    let b = GraphLayout::default().place(&keys, &edges);
    assert_eq!(a.len(), keys.len());
    for (p, q) in a.iter().zip(&b) {
        assert_eq!(p.0.to_bits(), q.0.to_bits());
        assert_eq!(p.1.to_bits(), q.1.to_bits());
    }
}

#[test]
fn layout_stable_on_insertion() {
    let (keys, edges) = ring(10);
    let layout = GraphLayout::default();
    let before = by_key(&keys, &layout.place(&keys, &edges));

    let mut keys2 = keys.clone();
    keys2.push(NodeKey("session:s05x".to_string()));
    keys2.sort();
    let idx = |k: &str| keys2.iter().position(|n| n.as_str() == k).unwrap();
    let mut edges2: Vec<(usize, usize)> = (0..10)
        .map(|i| (idx(keys[i].as_str()), idx(keys[(i + 1) % 10].as_str())))
        .collect();
    edges2.push((idx("session:s05x"), idx("session:s03")));
    let after = by_key(&keys2, &layout.place(&keys2, &edges2));

    for (k, p) in &before {
        let q = after[k];
        let d = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
        assert!(d < 0.08, "{} moved {d}", k.as_str());
    }
}

#[test]
fn layout_respects_bounds_and_cap() {
    let (mut wm, _area) = fixture(120, 40);
    let now = Instant::now();
    for i in 0..80 {
        wm.record_traffic(
            TrafficEndpoint::User,
            TrafficEndpoint::Session(format!("s{i}")),
            TrafficKind::Broadcast,
            "x",
            now,
        );
    }
    let scene = wm.agents_graph_scene();
    assert!(scene.nodes.len() <= 64, "{} nodes", scene.nodes.len());
    for id in wm.panes().keys() {
        assert!(scene.nodes.iter().any(|n| n.key == NodeKey::pane(*id)), "pane {id} dropped");
    }
    for n in &scene.nodes {
        assert!((0.05..=0.95).contains(&n.pos.0) && (0.05..=0.95).contains(&n.pos.1), "{:?}", n.pos);
    }
    assert!(scene.edges.iter().all(|e| {
        scene.nodes.iter().any(|n| n.key == e.from) && scene.nodes.iter().any(|n| n.key == e.to)
    }));
}

fn traffic_fixture() -> (WindowManager, Rect) {
    let (mut wm, area) = fixture(120, 40);
    wm.broadcast_command("echo hi").unwrap();
    wm.open_agents_window(None);
    (wm, area)
}

#[test]
fn graph_select_nearest_in_direction() {
    let (mut wm, _) = traffic_fixture();
    let scene = wm.agents_graph_scene();
    assert_eq!(scene.nodes.len(), 3);
    let left = scene
        .nodes
        .iter()
        .min_by(|a, b| a.pos.0.total_cmp(&b.pos.0).then_with(|| a.key.cmp(&b.key)))
        .unwrap()
        .clone();
    assert!(wm.agents_select_node(&left.key));
    assert_eq!(wm.agents_window().selection.node.as_ref(), Some(&left.key));

    let expected = scene
        .nodes
        .iter()
        .filter(|n| n.pos.0 > left.pos.0)
        .min_by(|a, b| {
            let da = (a.pos.0 - left.pos.0).powi(2) + (a.pos.1 - left.pos.1).powi(2);
            let db = (b.pos.0 - left.pos.0).powi(2) + (b.pos.1 - left.pos.1).powi(2);
            da.total_cmp(&db).then_with(|| a.key.cmp(&b.key))
        })
        .map(|n| n.key.clone());
    assert!(expected.is_some(), "fixture has a node to the right");
    assert!(wm.agents_select_direction(GraphDir::Right));
    assert_eq!(wm.agents_window().selection.node, expected);

    // No node further left than the leftmost one: false, selection unchanged.
    assert!(wm.agents_select_node(&left.key));
    let before = wm.agents_window().selection.clone();
    assert!(!wm.agents_select_direction(GraphDir::Left));
    assert_eq!(wm.agents_window().selection, before);

    assert!(!wm.agents_select_node(&NodeKey("pane:999".to_string())));
}

#[test]
fn graph_cycle_incident_edges() {
    let (mut wm, _) = traffic_fixture();
    assert!(!wm.agents_cycle_edge(true), "no node selected yet");
    assert!(wm.agents_select_node(&NodeKey("user".to_string())));
    // user is connected to pane:1 and pane:2, sorted by the other endpoint's key.
    let edge = |wm: &WindowManager| wm.agents_window().selection.edge.clone();
    assert!(wm.agents_cycle_edge(true));
    assert_eq!(edge(&wm).map(|(a, b)| (a.0, b.0)), Some(("pane:1".into(), "user".into())));
    assert!(wm.agents_cycle_edge(true));
    assert_eq!(edge(&wm).map(|(a, b)| (a.0, b.0)), Some(("pane:2".into(), "user".into())));
    assert!(wm.agents_cycle_edge(true));
    assert_eq!(edge(&wm).map(|(a, b)| (a.0, b.0)), Some(("pane:1".into(), "user".into())), "forward wraps");
    assert!(wm.agents_cycle_edge(false));
    assert_eq!(edge(&wm).map(|(a, b)| (a.0, b.0)), Some(("pane:2".into(), "user".into())), "backward wraps");
    assert!(wm.agents_select_node(&NodeKey::pane(1)));
    assert!(wm.agents_window().selection.edge.is_none(), "moving the node clears the edge");
}

fn rgb_of(c: ratatui::style::Color) -> (i32, i32, i32) {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => (i32::from(r), i32::from(g), i32::from(b)),
        other => panic!("expected an rgb colour, got {other:?}"),
    }
}

fn dist(a: (i32, i32, i32), b: (i32, i32, i32)) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs()
}

#[test]
fn render_selection_highlight() {
    let (mut wm, area) = traffic_fixture();
    let rect = wm.agents_window_rect(area).unwrap();
    let tokens = DesignTokens::from_palette(&wm.config().palette());
    let accent = rgb_of(tokens.accent);

    // Selected node label renders reversed.
    wm.agents_select_node(&NodeKey::pane(1));
    let buf = render(&wm, area);
    let reversed_hash = (rect.y..rect.y + rect.height).any(|y| {
        (rect.x..rect.x + rect.width).any(|x| {
            let c = &buf[(x, y)];
            c.symbol() == "#"
                && c.modifier.contains(ratatui::style::Modifier::REVERSED)
                && buf[(x + 1, y)].symbol() == "1"
        })
    });
    assert!(reversed_hash, "selected node label must be reversed");

    // Selecting an edge colours braille cells on it with the accent.
    let accent_cells = |buf: &Buffer| {
        (rect.y + 1..rect.y + rect.height - 1)
            .flat_map(|y| (rect.x + 1..rect.x + rect.width / 2).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let c = &buf[(x, y)];
                is_braille(c.symbol()) && dist(rgb_of(c.fg), accent) <= 40
            })
            .count()
    };
    let without = accent_cells(&render(&wm, area));
    wm.agents_select_node(&NodeKey::pane(1));
    assert!(wm.agents_cycle_edge(true));
    let with = accent_cells(&render(&wm, area));
    assert!(with > without, "selected edge needs accent braille cells ({with} vs {without})");
}

#[test]
fn render_tui_graph_snapshot_static() {
    let (wm, area) = traffic_fixture();
    let rect = wm.agents_window_rect(area).unwrap();
    let buf = render(&wm, area);
    let text: Vec<String> = (rect.y..rect.y + rect.height)
        .map(|y| {
            (rect.x..rect.x + rect.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    insta::assert_snapshot!("render_tui_graph_snapshot_static", text.join("\n"));
}

// ---- Plan 50-08: keyboard and command entry points ----

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn press_mod(wm: &mut WindowManager, code: KeyCode, mods: KeyModifiers) {
    handle_wm_key_event(wm, KeyEvent::new(code, mods)).unwrap();
}

fn press(wm: &mut WindowManager, code: KeyCode) {
    press_mod(wm, code, KeyModifiers::NONE);
}

#[test]
fn window_tracer_zoom_command_and_esc() {
    let (mut wm, area) = fixture(120, 40);
    wm.broadcast_command("echo hi").unwrap();
    assert!(!wm.agents_window().active);

    run_colon(&mut wm, ":agents zoom");
    assert!(wm.agents_window().active);
    let text = lines(&render(&wm, area)).join("\n");
    assert!(text.contains("AGENT TRAFFIC"), "zoom renders the window");

    press(&mut wm, KeyCode::Esc);
    assert!(!wm.agents_window().active, "Esc closes the unpinned window");

    run_colon(&mut wm, ":agents zoom");
    run_colon(&mut wm, ":agents dismiss");
    assert!(!wm.agents_window().active);

    run_colon(&mut wm, ":agents zoom");
    run_colon(&mut wm, ":agents close");
    assert!(!wm.agents_window().active);

    // Pinned: Esc / dismiss only release the keyboard.
    run_colon(&mut wm, ":agents zoom");
    wm.agents_window_mut().pinned = true;
    press(&mut wm, KeyCode::Esc);
    assert!(wm.agents_window().active && !wm.agents_window().focused);

    let err = parse_colon_command(":agents zoom x").unwrap().unwrap_err();
    assert_eq!(err, "Usage: :agents zoom [pane-id]");
    assert_eq!(
        parse_colon_command(":agents zoom 2").unwrap().unwrap(),
        Action::Agents(bilterm::wm::agents_cmd::AgentsOp::Zoom(Some(2)))
    );
}

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Isolates config/state directories for tests that drive real key dispatch.
fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-agents-window-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY-equivalent: serialized by ENV_LOCK; every test using key dispatch goes through here.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

type Observed = (bool, bool, Option<NodeKey>, Option<(NodeKey, NodeKey)>, usize);

fn observe(wm: &WindowManager) -> Observed {
    let w = wm.agents_window();
    (w.active, w.focused, w.selection.node.clone(), w.selection.edge.clone(), wm.active_pane_id())
}

fn leftmost(wm: &WindowManager) -> NodeKey {
    wm.agents_graph_scene()
        .nodes
        .iter()
        .min_by(|a, b| a.pos.0.total_cmp(&b.pos.0).then_with(|| a.key.cmp(&b.key)))
        .unwrap()
        .key
        .clone()
}

#[test]
fn window_arrows_move_to_nearest_node() {
    with_temp_xdg(|| {
        let by_key = {
            let (mut wm, _) = traffic_fixture();
            let l = leftmost(&wm);
            assert!(wm.agents_select_node(&l));
            press(&mut wm, KeyCode::Right);
            wm.agents_window().selection.node.clone()
        };
        let by_colon = {
            let (mut wm, _) = traffic_fixture();
            let l = leftmost(&wm);
            assert!(wm.agents_select_node(&l));
            run_colon(&mut wm, ":agents select right");
            wm.agents_window().selection.node.clone()
        };
        assert!(by_key.is_some());
        assert_eq!(by_key, by_colon);
    });
}

#[test]
fn window_tab_cycles_incident_edges() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        assert!(wm.agents_select_node(&NodeKey("user".to_string())));
        press(&mut wm, KeyCode::Tab);
        let first = wm.agents_window().selection.edge.clone().expect("Tab selects an edge");
        press(&mut wm, KeyCode::Tab);
        let second = wm.agents_window().selection.edge.clone().unwrap();
        assert_ne!(first, second);
        press_mod(&mut wm, KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(wm.agents_window().selection.edge, Some(first.clone()), "Shift+Tab goes back");
        press(&mut wm, KeyCode::BackTab);
        assert_eq!(wm.agents_window().selection.edge, Some(second), "BackTab without SHIFT also goes back");
        // The selected edge is drawn in the accent colour (see render_selection_highlight).
        assert!(lines(&render(&wm, area)).join("\n").contains("AGENT TRAFFIC"));
    });
}

#[test]
fn window_enter_on_pane_node_focuses_pane() {
    with_temp_xdg(|| {
        for pinned in [false, true] {
            let (mut wm, _) = traffic_fixture();
            wm.focus_and_raise(1);
            wm.agents_window_mut().pinned = pinned;
            assert!(wm.agents_select_node(&NodeKey::pane(2)));
            press(&mut wm, KeyCode::Enter);
            assert_eq!(wm.active_pane_id(), 2);
            if pinned {
                assert!(wm.agents_window().active && !wm.agents_window().focused, "pinned stays open, unfocused");
            } else {
                assert!(!wm.agents_window().active, "unpinned window closes");
            }
        }
        // Not a pane node: stays open, status explains.
        let (mut wm, _) = traffic_fixture();
        assert!(wm.agents_select_node(&NodeKey("user".to_string())));
        press(&mut wm, KeyCode::Enter);
        assert!(wm.agents_window().active);
    });
}

#[test]
fn window_leader_a_opens_window() {
    with_temp_xdg(|| {
        let (mut wm, _) = fixture(120, 40);
        press_mod(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending());
        press(&mut wm, KeyCode::Char('a'));
        assert!(wm.agents_window().active);
    });
}

#[test]
fn window_zoom_with_pane_selects_node() {
    let (mut wm, _) = fixture(120, 40);
    run_colon(&mut wm, ":agents zoom 2");
    assert_eq!(wm.agents_window().selection.node, Some(NodeKey::pane(2)));
    let (mut wm, _) = fixture(120, 40);
    run_colon(&mut wm, ":agents zoom 99");
    assert!(wm.agents_window().active);
    assert!(wm.agents_window().selection.node.is_none(), "unknown pane: no selection");
}

#[test]
fn window_leader_passes_through_while_focused() {
    with_temp_xdg(|| {
        let (mut wm, _) = fixture(120, 40);
        wm.focus_and_raise(1);
        wm.report_agent_state(2, Some(AgentState::Blocked));
        wm.update_agent_states(Instant::now());
        wm.open_agents_window(None);
        assert!(wm.agents_window().captures_input());
        press_mod(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending(), "the leader key is not consumed by the window");
        press(&mut wm, KeyCode::Char('!'));
        assert_eq!(wm.active_pane_id(), 2, "attention jump ran");
    });
}

fn agents_op_colon_form(op: &AgentsOp) -> Option<String> {
    Some(match op {
        AgentsOp::List => ":agents list".to_string(),
        AgentsOp::Zoom(None) => ":agents zoom".to_string(),
        AgentsOp::Zoom(Some(id)) => format!(":agents zoom {id}"),
        AgentsOp::Close => ":agents close".to_string(),
        AgentsOp::Dismiss => ":agents dismiss".to_string(),
        AgentsOp::Select(dir) => format!(
            ":agents select {}",
            match dir {
                GraphDir::Left => "left",
                GraphDir::Right => "right",
                GraphDir::Up => "up",
                GraphDir::Down => "down",
            }
        ),
        AgentsOp::CycleEdge(true) => ":agents edge next".to_string(),
        AgentsOp::CycleEdge(false) => ":agents edge prev".to_string(),
        AgentsOp::Activate => ":agents enter".to_string(),
    })
}

fn sample_ops() -> Vec<AgentsOp> {
    vec![
        AgentsOp::List,
        AgentsOp::Zoom(None),
        AgentsOp::Zoom(Some(2)),
        AgentsOp::Close,
        AgentsOp::Dismiss,
        AgentsOp::Select(GraphDir::Left),
        AgentsOp::Select(GraphDir::Right),
        AgentsOp::Select(GraphDir::Up),
        AgentsOp::Select(GraphDir::Down),
        AgentsOp::CycleEdge(true),
        AgentsOp::CycleEdge(false),
        AgentsOp::Activate,
    ]
}

#[test]
fn window_keys_match_colon_forms() {
    use bilterm::input::keymap::AGENTS_WINDOW_BINDINGS;
    for (_, mods, code, action) in AGENTS_WINDOW_BINDINGS {
        let Action::Agents(op) = action else { panic!("{action:?} is not an Agents action") };
        let form = agents_op_colon_form(op).expect("every table op has a colon form");
        let setup = |wm: &mut WindowManager| {
            assert!(wm.agents_select_node(&NodeKey("user".to_string())));
            wm.agents_cycle_edge(true);
            wm.focus_and_raise(1);
        };
        let by_key = with_temp_xdg(|| {
            let (mut wm, _) = traffic_fixture();
            setup(&mut wm);
            press_mod(&mut wm, *code, *mods);
            observe(&wm)
        });
        let by_colon = with_temp_xdg(|| {
            let (mut wm, _) = traffic_fixture();
            setup(&mut wm);
            run_colon(&mut wm, &form);
            observe(&wm)
        });
        assert_eq!(by_key, by_colon, "key {code:?} vs `{form}`");
    }
}

#[test]
fn keymap_agents_window_keys_unique() {
    use bilterm::input::keymap::{AGENTS_WINDOW_BINDINGS, AgentsKeyScope};
    let mut seen = Vec::new();
    for (scope, mods, code, _) in AGENTS_WINDOW_BINDINGS {
        let triple = (*scope, *mods, *code);
        assert!(!seen.contains(&triple), "duplicate agents window chord {triple:?}");
        seen.push(triple);
    }
    for (scope, mods, code, _) in AGENTS_WINDOW_BINDINGS {
        if *scope == AgentsKeyScope::Window {
            assert!(
                !AGENTS_WINDOW_BINDINGS.iter().any(|(s, m, c, _)| s != scope && m == mods && c == code),
                "Window-scope chord {code:?}+{mods:?} is also bound in another scope"
            );
        }
    }
}

#[test]
fn keymap_agents_window_keys_do_not_shadow_globals() {
    use bilterm::input::keymap::{AGENTS_WINDOW_BINDINGS, DEFAULT_GLOBAL_BINDINGS, default_global_action};
    for (_, mods, code, _) in AGENTS_WINDOW_BINDINGS {
        let norm = match code {
            KeyCode::Char(_) | KeyCode::BackTab => *mods - KeyModifiers::SHIFT,
            _ => *mods,
        };
        assert!(
            default_global_action(KeyEvent::new(*code, *mods), false).is_none(),
            "agents row {code:?}+{mods:?} shadows a global chord"
        );
        assert!(
            !DEFAULT_GLOBAL_BINDINGS.iter().any(|(m, c, _)| c == code && (*m == *mods || *m == norm)),
            "agents row {code:?}+{mods:?} is bound in DEFAULT_GLOBAL_BINDINGS"
        );
    }
}

#[test]
fn agents_ops_have_colon_completion_and_help() {
    use bilterm::input::keymap::{AGENTS_WINDOW_BINDINGS, DEFAULT_LEADER_BINDINGS};
    let row = bilterm::pane::completion::colon_command_table()
        .iter()
        .find(|(c, _)| *c == ":agents")
        .expect(":agents completion row")
        .1;
    let samples = sample_ops();
    for op in &samples {
        let form = agents_op_colon_form(op).expect("colon form");
        let parsed = parse_colon_command(&form).unwrap().unwrap_or_else(|e| panic!("`{form}`: {e}"));
        assert_eq!(parsed, Action::Agents(op.clone()), "`{form}` must parse to its op");
        let verb = form.split_whitespace().nth(1).unwrap();
        assert!(
            row.iter().any(|s| s.split_whitespace().next() == Some(verb)),
            "verb `{verb}` missing from the :agents completion row"
        );
        let prefix = format!(":agents {verb}");
        assert!(
            bilterm::ui::modals::HELP_LINES
                .iter()
                .any(|(c, _)| c.starts_with(&prefix) || (verb == "list" && c.starts_with(":agents [list]"))),
            "no HELP_LINES row for `{prefix}`"
        );
    }
    for (_, _, _, action) in AGENTS_WINDOW_BINDINGS {
        let Action::Agents(op) = action else { panic!("{action:?} is not an Agents action") };
        assert!(samples.contains(op), "table op {op:?} is not covered by the sampled ops");
    }
    let leader = DEFAULT_LEADER_BINDINGS
        .iter()
        .find(|(k, _)| *k == KeyCode::Char('a'))
        .expect("leader a row");
    assert!(matches!(&leader.1, Action::Agents(op) if samples.contains(op)));
}
