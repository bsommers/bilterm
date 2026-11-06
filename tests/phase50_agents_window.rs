//! Phase 50-07: AGENT TRAFFIC zoom window (FLT-01, FLT-02). `window_` = tracer slice,
//! `layout_` = graph placement, `graph_` = selection model, `render_` = frame output.

use std::collections::BTreeMap;
use std::time::Instant;

use bilterm::action::Action;
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::design::DesignTokens;
use bilterm::ui::typography::PanelTarget;
use bilterm::wm::agents_window::{GraphDir, GraphLayout, NodeKey, SnoopState, WindowPane};
use bilterm::wm::traffic::{TrafficEndpoint, TrafficKind};
use bilterm::wm::agent_state::AgentState;
use bilterm::wm::agents_cmd::{AgentsOp, SnoopFilterKind, SnoopFilterOp};
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

#[derive(Debug, PartialEq)]
struct Observed {
    active: bool,
    focused: bool,
    node: Option<NodeKey>,
    edge: Option<(NodeKey, NodeKey)>,
    active_pane: usize,
    rect: Option<Rect>,
    maximized: bool,
    splitter_pct: u16,
    grab: bool,
    pinned: Vec<(NodeKey, u32, u32)>,
    pane_focus: WindowPane,
    snoop: SnoopState,
}

fn observe(wm: &WindowManager) -> Observed {
    let w = wm.agents_window();
    Observed {
        active: w.active,
        focused: w.focused,
        node: w.selection.node.clone(),
        edge: w.selection.edge.clone(),
        active_pane: wm.active_pane_id(),
        rect: w.rect,
        maximized: w.maximized,
        splitter_pct: w.splitter_pct,
        grab: w.grab_mode,
        pinned: w
            .layout
            .pinned
            .iter()
            .map(|(k, p)| (k.clone(), p.0.to_bits(), p.1.to_bits()))
            .collect(),
        pane_focus: w.pane_focus,
        snoop: w.snoop.clone(),
    }
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
            wm.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
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
        wm.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
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
        AgentsOp::Toggle(None) => ":agents toggle".to_string(),
        AgentsOp::Toggle(Some(id)) => format!(":agents toggle {id}"),
        AgentsOp::Demo => ":agents demo".to_string(),
        AgentsOp::DemoStop => ":agents demo stop".to_string(),
        AgentsOp::Close => ":agents close".to_string(),
        AgentsOp::Dismiss => ":agents dismiss".to_string(),
        AgentsOp::Blur => ":agents blur".to_string(),
        AgentsOp::Fade(None) => ":agents fade".to_string(),
        AgentsOp::Fade(Some(n)) => format!(":agents fade {n}"),
        AgentsOp::Topology(bilterm::wm::agents_cmd::TopologyOp::Show) => ":agents topology".to_string(),
        AgentsOp::Topology(bilterm::wm::agents_cmd::TopologyOp::Export(n)) => format!(":agents topology export {n}"),
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
        AgentsOp::Move { dx, dy } => format!(":agents move {dx} {dy}"),
        AgentsOp::Resize { dw, dh } => format!(":agents resize {dw} {dh}"),
        AgentsOp::SetRect { x, y, w, h } => format!(":agents rect {x} {y} {w} {h}"),
        AgentsOp::Size(None) => ":agents size auto".to_string(),
        AgentsOp::Size(Some((c, r))) => format!(":agents size {c}x{r}"),
        AgentsOp::Maximize => ":agents max".to_string(),
        AgentsOp::Split(n) => format!(":agents split {n}"),
        AgentsOp::SplitStep(n) if *n >= 0 => format!(":agents split +{n}"),
        AgentsOp::SplitStep(n) => format!(":agents split {n}"),
        AgentsOp::SelectNode(k) => format!(":agents select {}", k.as_str()),
        AgentsOp::SelectEdge(a, b) => format!(":agents edge {} {}", a.as_str(), b.as_str()),
        AgentsOp::Grab(None) => ":agents grab".to_string(),
        AgentsOp::Grab(Some(true)) => ":agents grab on".to_string(),
        AgentsOp::Grab(Some(false)) => ":agents grab off".to_string(),
        AgentsOp::NudgeNode(dir) => format!(
            ":agents nudge {}",
            match dir {
                GraphDir::Left => "left",
                GraphDir::Right => "right",
                GraphDir::Up => "up",
                GraphDir::Down => "down",
            }
        ),
        AgentsOp::PinNode { key, x_pm, y_pm } => format!(":agents node-pin {} {x_pm} {y_pm}", key.as_str()),
        AgentsOp::UnpinNode(None) => ":agents node-unpin".to_string(),
        AgentsOp::UnpinNode(Some(k)) => format!(":agents node-unpin {}", k.as_str()),
        AgentsOp::Pin(None) => ":agents pin toggle".to_string(),
        AgentsOp::Pin(Some(true)) => ":agents pin".to_string(),
        AgentsOp::Pin(Some(false)) => ":agents unpin".to_string(),
        AgentsOp::ScaleStep(n) if *n >= 0 => format!(":agents scale +{n}"),
        AgentsOp::ScaleStep(n) => format!(":agents scale {n}"),
        AgentsOp::FocusPane(None) => ":agents focus".to_string(),
        AgentsOp::FocusPane(Some(WindowPane::Graph)) => ":agents focus graph".to_string(),
        AgentsOp::FocusPane(Some(WindowPane::Snoop)) => ":agents focus snoop".to_string(),
        AgentsOp::FocusPane(Some(WindowPane::Compose)) => ":agents focus compose".to_string(),
        AgentsOp::SnoopStep(n) if *n >= 0 => format!(":agents line +{n}"),
        AgentsOp::SnoopStep(n) => format!(":agents line {n}"),
        AgentsOp::SnoopSelect(seq) => format!(":agents line {seq}"),
        AgentsOp::Copy => ":agents copy".to_string(),
        AgentsOp::SnoopScroll(i16::MIN) => ":agents scroll top".to_string(),
        AgentsOp::SnoopScroll(n) => format!(":agents scroll {n}"),
        AgentsOp::Follow(None) => ":agents follow".to_string(),
        AgentsOp::Follow(Some(true)) => ":agents follow on".to_string(),
        AgentsOp::Follow(Some(false)) => ":agents follow off".to_string(),
        AgentsOp::Filter(SnoopFilterOp::Agent(l)) => format!(":agents filter agent {l}"),
        AgentsOp::Filter(SnoopFilterOp::Pair(a, b)) => format!(":agents filter pair {a} {b}"),
        AgentsOp::Filter(SnoopFilterOp::Text(t)) => format!(":agents filter text {t}"),
        AgentsOp::Filter(SnoopFilterOp::Clear) => ":agents filter clear".to_string(),
        AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Agent)) => ":agents filter remove agent".to_string(),
        AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Pair)) => ":agents filter remove pair".to_string(),
        AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Text)) => ":agents filter remove text".to_string(),
        AgentsOp::Filter(SnoopFilterOp::AgentFromSelection) => ":agents filter agent".to_string(),
        AgentsOp::Filter(SnoopFilterOp::PairFromSelection) => ":agents filter pair".to_string(),
        // Documented exemption: keyword entry is text-field editing; its committed form is
        // `:agents filter text <kw>` (covered by `Filter(Text)`).
        AgentsOp::KeywordEdit(_) => return None,
        // Documented exemption: Phase 51 decides the command surface of the outward write path
        // behind its permission gate (D-09); Phase 50 offers no way to submit over RPC/MCP.
        AgentsOp::ComposeSubmit => return None,
    })
}

fn sample_ops() -> Vec<AgentsOp> {
    vec![
        AgentsOp::List,
        AgentsOp::Zoom(None),
        AgentsOp::Zoom(Some(2)),
        AgentsOp::Toggle(None),
        AgentsOp::Toggle(Some(2)),
        AgentsOp::Close,
        AgentsOp::Dismiss,
        AgentsOp::Blur,
        AgentsOp::Fade(None),
        AgentsOp::Fade(Some(90)),
        AgentsOp::Topology(bilterm::wm::agents_cmd::TopologyOp::Show),
        AgentsOp::Topology(bilterm::wm::agents_cmd::TopologyOp::Export("run1".to_string())),
        AgentsOp::Select(GraphDir::Left),
        AgentsOp::Select(GraphDir::Right),
        AgentsOp::Select(GraphDir::Up),
        AgentsOp::Select(GraphDir::Down),
        AgentsOp::CycleEdge(true),
        AgentsOp::CycleEdge(false),
        AgentsOp::Activate,
        AgentsOp::Move { dx: -1, dy: 0 },
        AgentsOp::Move { dx: 1, dy: 0 },
        AgentsOp::Move { dx: 0, dy: -1 },
        AgentsOp::Move { dx: 0, dy: 1 },
        AgentsOp::Resize { dw: 1, dh: 0 },
        AgentsOp::Resize { dw: -1, dh: 0 },
        AgentsOp::Resize { dw: 0, dh: 1 },
        AgentsOp::Resize { dw: 0, dh: -1 },
        AgentsOp::SetRect { x: 10, y: 5, w: 80, h: 24 },
        AgentsOp::Size(Some((90, 30))),
        AgentsOp::Size(None),
        AgentsOp::Maximize,
        AgentsOp::Split(60),
        AgentsOp::SplitStep(-5),
        AgentsOp::SplitStep(5),
        AgentsOp::SelectNode(NodeKey::pane(2)),
        AgentsOp::SelectEdge(NodeKey::pane(1), NodeKey("user".to_string())),
        AgentsOp::Grab(None),
        AgentsOp::Grab(Some(true)),
        AgentsOp::Grab(Some(false)),
        AgentsOp::NudgeNode(GraphDir::Left),
        AgentsOp::NudgeNode(GraphDir::Right),
        AgentsOp::NudgeNode(GraphDir::Up),
        AgentsOp::NudgeNode(GraphDir::Down),
        AgentsOp::PinNode { key: NodeKey::pane(1), x_pm: 300, y_pm: 700 },
        AgentsOp::UnpinNode(None),
        AgentsOp::UnpinNode(Some(NodeKey::pane(1))),
        AgentsOp::Pin(None),
        AgentsOp::Pin(Some(true)),
        AgentsOp::Pin(Some(false)),
        AgentsOp::ScaleStep(-10),
        AgentsOp::ScaleStep(10),
        AgentsOp::FocusPane(None),
        AgentsOp::FocusPane(Some(WindowPane::Graph)),
        AgentsOp::FocusPane(Some(WindowPane::Snoop)),
        AgentsOp::FocusPane(Some(WindowPane::Compose)),
        AgentsOp::SnoopStep(1),
        AgentsOp::SnoopStep(-1),
        AgentsOp::SnoopSelect(2),
        AgentsOp::Copy,
        AgentsOp::SnoopScroll(-10),
        AgentsOp::SnoopScroll(10),
        AgentsOp::SnoopScroll(i16::MIN),
        AgentsOp::Follow(None),
        AgentsOp::Follow(Some(true)),
        AgentsOp::Follow(Some(false)),
        AgentsOp::Filter(SnoopFilterOp::Agent("you".to_string())),
        AgentsOp::Filter(SnoopFilterOp::Pair("you".to_string(), "#2".to_string())),
        AgentsOp::Filter(SnoopFilterOp::Text("echo".to_string())),
        AgentsOp::Filter(SnoopFilterOp::Clear),
        AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Agent)),
        AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Pair)),
        AgentsOp::Filter(SnoopFilterOp::Remove(SnoopFilterKind::Text)),
        AgentsOp::Filter(SnoopFilterOp::AgentFromSelection),
        AgentsOp::Filter(SnoopFilterOp::PairFromSelection),
    ]
}

#[test]
fn window_keys_match_colon_forms() {
    use bilterm::input::keymap::AGENTS_WINDOW_BINDINGS;
    for (scope, mods, code, action) in AGENTS_WINDOW_BINDINGS {
        let Action::Agents(op) = action else { panic!("{action:?} is not an Agents action") };
        let Some(form) = agents_op_colon_form(op) else {
            assert!(
                matches!(op, AgentsOp::KeywordEdit(_) | AgentsOp::ComposeSubmit),
                "{op:?} has no colon form and no exemption"
            );
            continue;
        };
        let in_grab = *scope == bilterm::input::keymap::AgentsKeyScope::Grab;
        let in_snoop = *scope == bilterm::input::keymap::AgentsKeyScope::Snoop;
        let setup = |wm: &mut WindowManager| {
            assert!(wm.agents_select_node(&NodeKey("user".to_string())));
            wm.agents_cycle_edge(true);
            wm.focus_and_raise(1);
            wm.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
            if in_grab {
                run_colon(wm, ":agents grab on");
            }
            if in_snoop {
                run_colon(wm, ":agents focus snoop");
            }
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
                !AGENTS_WINDOW_BINDINGS.iter().any(|(s, m, c, _)| {
                    // Documented overrides: Esc in grab mode drops the grab, and Esc in keyword
                    // entry cancels the entry, instead of dismissing the window.
                    s != scope
                        && m == mods
                        && c == code
                        && !(matches!(*s, AgentsKeyScope::Grab | AgentsKeyScope::KeywordEntry)
                            && *c == KeyCode::Esc)
                }),
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
        if matches!(op, AgentsOp::KeywordEdit(_) | AgentsOp::ComposeSubmit) {
            continue; // exempt: text-field editing / the closed Phase 51 write path
        }
        assert!(samples.contains(op), "table op {op:?} is not covered by the sampled ops");
    }
    let leader = DEFAULT_LEADER_BINDINGS
        .iter()
        .find(|(k, _)| *k == KeyCode::Char('a'))
        .expect("leader a row");
    assert!(matches!(&leader.1, Action::Agents(op) if samples.contains(op)));
}

// ---- Plan 50-09: window chrome (move, resize, maximize, splitter) and node/edge gestures ----

fn ws_of(wm: &WindowManager, area: Rect) -> Rect {
    bilterm::ui::shell::ShellLayout::compute(area, wm.config(), wm.pane_count()).workspace
}

fn mouse(wm: &mut WindowManager, area: Rect, kind: MouseEventKind, col: u16, row: u16) {
    let ev = MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE };
    handle_wm_mouse_event(wm, ev, area).unwrap();
}

fn click(wm: &mut WindowManager, area: Rect, col: u16, row: u16) {
    mouse(wm, area, MouseEventKind::Down(MouseButton::Left), col, row);
    mouse(wm, area, MouseEventKind::Up(MouseButton::Left), col, row);
}

fn drag(wm: &mut WindowManager, area: Rect, from: (u16, u16), to: (u16, u16)) {
    mouse(wm, area, MouseEventKind::Down(MouseButton::Left), from.0, from.1);
    mouse(wm, area, MouseEventKind::Drag(MouseButton::Left), to.0, to.1);
    mouse(wm, area, MouseEventKind::Up(MouseButton::Left), to.0, to.1);
}

fn win_rect(wm: &WindowManager, area: Rect) -> Rect {
    wm.agents_window_rect(ws_of(wm, area)).expect("window rect")
}

/// First cell of the label of node `key`.
fn node_cell(wm: &WindowManager, area: Rect, key: &str) -> (u16, u16) {
    let rects = wm.agents_pane_rects(ws_of(wm, area)).unwrap();
    let scene = wm.agents_graph_scene();
    let node = scene.nodes.iter().find(|n| n.key.as_str() == key).expect("node in scene");
    let (lx, ly, _) = bilterm::ui::graph_raster::label_cell(node, rects.graph).unwrap();
    (rects.graph.x + lx, rects.graph.y + ly)
}

fn user() -> NodeKey {
    NodeKey("user".to_string())
}

#[test]
fn window_tracer_drag_title_and_shift_arrows_move() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        assert_eq!(ws_of(&wm, area), area_workspace(&wm), "mouse and action paths share one workspace");
        let r0 = win_rect(&wm, area);

        drag(&mut wm, area, (r0.x + 5, r0.y), (r0.x + 12, r0.y + 3));
        let r1 = win_rect(&wm, area);
        assert_eq!((r1.x, r1.y, r1.width, r1.height), (r0.x + 7, r0.y + 3, r0.width, r0.height));
        assert!(wm.agents_window().drag.is_none(), "drag bookkeeping cleared on release");

        let (mut other, oarea) = traffic_fixture();
        run_colon(&mut other, &format!(":agents rect {} {} {} {}", r1.x, r1.y, r1.width, r1.height));
        assert_eq!(win_rect(&other, oarea), r1, ":agents rect gives the dragged rect");

        press_mod(&mut wm, KeyCode::Left, KeyModifiers::SHIFT);
        press_mod(&mut wm, KeyCode::Left, KeyModifiers::SHIFT);
        let r2 = win_rect(&wm, area);
        assert_eq!(r2.x, r1.x - 2);
        run_colon(&mut other, ":agents move -2 0");
        assert_eq!(win_rect(&other, oarea), r2, "Shift+Left x2 equals :agents move -2 0");

        // Clicks inside the window never reach the drawer or panes beneath it.
        let (mut wm, area) = traffic_fixture();
        wm.toggle_drawer();
        let drawer = wm.drawer_rect(ws_of(&wm, area)).expect("drawer rect");
        run_colon(&mut wm, ":agents max");
        let before = (wm.drawer().selected_index, wm.active_pane_id());
        let wr = win_rect(&wm, area);
        let (c, r) = (drawer.x + 2, drawer.y + 3);
        assert!(c > wr.x && c < wr.x + wr.width - 1 && r > wr.y && r < wr.y + wr.height - 1, "probe cell lies inside the window");
        click(&mut wm, area, c, r);
        assert_eq!((wm.drawer().selected_index, wm.active_pane_id()), before);

        // Pinned window: a click outside releases the keyboard.
        run_colon(&mut wm, ":agents max");
        assert!(!wm.agents_window().maximized);
        wm.agents_window_mut().pinned = true;
        wm.agents_window_mut().focused = true;
        assert!(wm.agents_window().captures_input());
        mouse(&mut wm, area, MouseEventKind::Down(MouseButton::Left), area.width - 1, area.height - 3);
        assert!(!wm.agents_window().captures_input());
        assert!(wm.agents_window().active, "pinned window stays open");
    });
}

/// The workspace the `Action` paths clamp against (same formula the shell uses).
fn area_workspace(wm: &WindowManager) -> Rect {
    let (cols, rows) = (120u16, 40u16);
    ws_of(wm, Rect::new(0, 0, cols, rows))
}

#[test]
fn window_edge_drag_resizes() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let r0 = win_rect(&wm, area);
        let right = r0.x + r0.width - 1;
        let bottom = r0.y + r0.height - 1;
        drag(&mut wm, area, (right, r0.y + 5), (right + 10, r0.y + 5));
        let r1 = win_rect(&wm, area);
        assert_eq!((r1.x, r1.y, r1.width, r1.height), (r0.x, r0.y, r0.width + 10, r0.height));

        let (mut wm, area) = traffic_fixture();
        drag(&mut wm, area, (right, bottom), (right + 5, bottom + 3));
        let r2 = win_rect(&wm, area);
        assert_eq!((r2.width, r2.height), (r0.width + 5, r0.height + 3));
        assert!(wm.agents_window().rect.is_some(), "the drag stores an explicit rect");
    });
}

#[test]
fn window_alt_arrows_resize() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let (mut other, oarea) = traffic_fixture();
        let r0 = win_rect(&wm, area);
        press_mod(&mut wm, KeyCode::Right, KeyModifiers::ALT);
        run_colon(&mut other, ":agents resize 1 0");
        assert_eq!(win_rect(&wm, area), win_rect(&other, oarea));
        assert_eq!(win_rect(&wm, area).width, r0.width + 1);
        press_mod(&mut wm, KeyCode::Down, KeyModifiers::ALT);
        run_colon(&mut other, ":agents resize 0 1");
        assert_eq!(win_rect(&wm, area), win_rect(&other, oarea));
        for _ in 0..200 {
            press_mod(&mut wm, KeyCode::Left, KeyModifiers::ALT);
            press_mod(&mut wm, KeyCode::Up, KeyModifiers::ALT);
        }
        let r = win_rect(&wm, area);
        assert_eq!((r.width, r.height), (40, 12), "never below 40x12");
    });
}

#[test]
fn window_rect_clamped_to_workspace() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let ws = ws_of(&wm, area);
        run_colon(&mut wm, ":agents rect 0 0 500 500");
        let r = win_rect(&wm, area);
        assert_eq!((r.width, r.height), (ws.width, ws.height));
        run_colon(&mut wm, ":agents rect 1000 1000 50 20");
        let r = win_rect(&wm, area);
        assert!(r.x + r.width <= ws.x + ws.width && r.y + r.height <= ws.y + ws.height, "{r:?} outside {ws:?}");
        run_colon(&mut wm, ":agents rect 5 5 3 3");
        let r = win_rect(&wm, area);
        assert_eq!((r.width, r.height), (40, 12), "tiny rects grow to the minimum");

        let r0 = win_rect(&wm, area);
        drag(&mut wm, area, (r0.x + 5, r0.y), (area.width - 1, area.height - 1));
        let r = win_rect(&wm, area);
        assert!(r.x + r.width <= ws.x + ws.width && r.y + r.height <= ws.y + ws.height, "{r:?} outside {ws:?}");
    });
}

/// Title badge columns `(minimize, zoom, close)` of a window rect.
fn badge_cols(wm: &WindowManager, r: Rect) -> (u16, u16, u16) {
    let g = wm.config().glyph_set();
    let len = |s: &str| s.chars().count() as u16;
    let start = (r.x + r.width).saturating_sub(len(g.minimize_badge) + len(g.zoom_badge) + len(g.close_badge) + 2);
    (start, start + len(g.minimize_badge), start + len(g.minimize_badge) + len(g.zoom_badge))
}

#[test]
fn window_title_badges_close_and_maximize() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let r = win_rect(&wm, area);
        let (_, zoom, _) = badge_cols(&wm, r);
        click(&mut wm, area, zoom, r.y);
        assert!(wm.agents_window().maximized, "badge 1 maximizes");
        assert!(wm.agents_window().drag.is_none(), "a badge click does not start a drag");
        assert_eq!(win_rect(&wm, area), ws_of(&wm, area));
        let rm = win_rect(&wm, area);
        let (_, zoom, _) = badge_cols(&wm, rm);
        click(&mut wm, area, zoom, rm.y);
        assert!(!wm.agents_window().maximized, "badge 1 toggles back");

        for badge in [0usize, 2] {
            let (mut wm, area) = traffic_fixture();
            let r = win_rect(&wm, area);
            let (min, _, close) = badge_cols(&wm, r);
            click(&mut wm, area, [min, 0, close][badge], r.y);
            assert!(!wm.agents_window().active, "badge {badge} closes");
        }
    });
}

#[test]
fn window_m_key_and_colon_max_toggle_maximize() {
    with_temp_xdg(|| {
        let (mut a, aarea) = traffic_fixture();
        let (mut b, barea) = traffic_fixture();
        press(&mut a, KeyCode::Char('m'));
        run_colon(&mut b, ":agents max");
        assert!(a.agents_window().maximized);
        assert_eq!(observe(&a), observe(&b));
        assert_eq!(win_rect(&a, aarea), win_rect(&b, barea));
        press(&mut a, KeyCode::Char('m'));
        run_colon(&mut b, ":agents max");
        assert!(!a.agents_window().maximized);
        assert_eq!(observe(&a), observe(&b));
    });
}

#[test]
fn window_splitter_drag_and_keys() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let rects = wm.agents_pane_rects(ws_of(&wm, area)).unwrap();
        let from = (rects.splitter_col, rects.inner.y + 2);
        drag(&mut wm, area, from, (from.0 + 10, from.1));
        let pct = wm.agents_window().splitter_pct;
        assert!(pct > 55 && (20..=80).contains(&pct), "splitter moved right: {pct}");
        let after = wm.agents_pane_rects(ws_of(&wm, area)).unwrap().splitter_col;
        assert!(after.abs_diff(from.0 + 10) <= 1, "divider follows the pointer ({after} vs {})", from.0 + 10);

        let (mut other, oarea) = traffic_fixture();
        run_colon(&mut other, &format!(":agents split {pct}"));
        assert_eq!(
            other.agents_pane_rects(ws_of(&other, oarea)).unwrap().splitter_col,
            after,
            "drag equals :agents split <pct>"
        );

        let (mut a, _) = traffic_fixture();
        let (mut b, _) = traffic_fixture();
        press(&mut a, KeyCode::Char('<'));
        run_colon(&mut b, ":agents split -5");
        assert_eq!(a.agents_window().splitter_pct, 50);
        assert_eq!(observe(&a), observe(&b));
        press(&mut a, KeyCode::Char('>'));
        press(&mut a, KeyCode::Char('>'));
        run_colon(&mut b, ":agents split +5");
        run_colon(&mut b, ":agents split +5");
        assert_eq!(a.agents_window().splitter_pct, 60);
        assert_eq!(observe(&a), observe(&b));
        run_colon(&mut b, ":agents split +100");
        assert_eq!(b.agents_window().splitter_pct, 80);
        run_colon(&mut b, ":agents split -100");
        assert_eq!(b.agents_window().splitter_pct, 20);
    });
}

#[test]
fn window_colon_size_forms() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let default = win_rect(&wm, area);
        run_colon(&mut wm, ":agents size 90x30");
        let r = win_rect(&wm, area);
        assert_eq!((r.width, r.height), (90, 30));
        run_colon(&mut wm, ":agents size auto");
        assert_eq!(win_rect(&wm, area), default);
        for bad in [":agents size 5x5", ":agents size 500x30", ":agents size 90", ":agents split 95", ":agents split 10", ":agents resize x 1", ":agents move 1", ":agents rect 1 2 3"] {
            let err = parse_colon_command(bad).unwrap();
            assert!(err.is_err(), "`{bad}` must be a usage error, got {err:?}");
            assert!(err.unwrap_err().starts_with("Usage:"), "{bad}");
        }
    });
}

#[test]
fn window_click_node_selects_and_second_click_activates() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        wm.focus_and_raise(1);
        wm.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
        let (c, r) = node_cell(&wm, area, "pane:2");
        click(&mut wm, area, c, r);
        assert_eq!(wm.agents_window().selection.node, Some(NodeKey::pane(2)));
        let (mut other, _) = traffic_fixture();
        other.focus_and_raise(1);
        other.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
        run_colon(&mut other, ":agents select pane:2");
        assert_eq!(observe(&wm), observe(&other));
        assert_eq!(wm.active_pane_id(), 1, "the first click only selects");

        click(&mut wm, area, c, r);
        assert_eq!(wm.active_pane_id(), 2, "the second click activates like Enter");
        assert!(!wm.agents_window().active);
    });
}

#[test]
fn window_click_edge_selects_edge() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let rects = wm.agents_pane_rects(ws_of(&wm, area)).unwrap();
        let scene = wm.agents_graph_scene();
        let find = |k: &str| scene.nodes.iter().find(|n| n.key.as_str() == k).unwrap().clone();
        let (a, b) = (find("pane:1"), find("user"));
        let pair = (a.key.clone(), b.key.clone());
        let cells = bilterm::wm::agents_geometry::edge_curve_cells(&a, &b, rects.graph);
        let on_edge = cells
            .iter()
            .map(|&(x, y)| (rects.graph.x + x as u16, rects.graph.y + y as u16))
            .find(|&(x, y)| wm.agents_hit(ws_of(&wm, area), x, y) == bilterm::wm::agents_geometry::AgentsHit::Edge(pair.0.clone(), pair.1.clone()))
            .expect("a cell of the user - #1 edge that is not a node");
        click(&mut wm, area, on_edge.0, on_edge.1);
        assert_eq!(wm.agents_window().selection.edge, Some(pair));

        let (mut other, _) = traffic_fixture();
        run_colon(&mut other, ":agents edge user pane:1");
        assert_eq!(observe(&wm), observe(&other));
    });
}

#[test]
fn window_node_drag_pins_node() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let rects = wm.agents_pane_rects(ws_of(&wm, area)).unwrap();
        let from = node_cell(&wm, area, "pane:1");
        let to = (rects.graph.x + rects.graph.width / 2, rects.graph.y + rects.graph.height / 2);
        drag(&mut wm, area, from, to);
        let pinned = wm.agents_window().layout.pinned.get(&NodeKey::pane(1)).copied().expect("pinned");
        assert!(wm.agents_window().active, "a drag never activates");
        assert_eq!(wm.agents_window().selection.node, Some(NodeKey::pane(1)));
        let scene = wm.agents_graph_scene();
        let node = scene.nodes.iter().find(|n| n.key == NodeKey::pane(1)).unwrap();
        assert_eq!(node.pos, pinned, "the scene shows the pinned position");

        let (x_pm, y_pm) = ((pinned.0 * 1000.0).round() as u16, (pinned.1 * 1000.0).round() as u16);
        let (mut other, _) = traffic_fixture();
        run_colon(&mut other, ":agents select pane:1");
        run_colon(&mut other, &format!(":agents node-pin pane:1 {x_pm} {y_pm}"));
        assert_eq!(observe(&wm), observe(&other), "drag equals :agents node-pin with its permille");

        // A later layout pass (new traffic, new node) leaves the node where it is.
        wm.record_traffic(
            TrafficEndpoint::User,
            TrafficEndpoint::Session("later".into()),
            TrafficKind::Broadcast,
            "x",
            Instant::now(),
        );
        let scene = wm.agents_graph_scene();
        assert_eq!(scene.nodes.iter().find(|n| n.key == NodeKey::pane(1)).unwrap().pos, pinned);
    });
}

#[test]
fn window_grab_mode_moves_node_with_keys() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        assert!(wm.agents_select_node(&NodeKey::pane(1)));
        let x0 = wm.agents_graph_scene().nodes.iter().find(|n| n.key == NodeKey::pane(1)).unwrap().pos.0;
        press(&mut wm, KeyCode::Char('g'));
        assert!(wm.agents_window().grab_mode);
        for _ in 0..3 {
            press(&mut wm, KeyCode::Right);
        }
        press(&mut wm, KeyCode::Char('g'));
        assert!(!wm.agents_window().grab_mode);
        let pos = wm.agents_window().layout.pinned.get(&NodeKey::pane(1)).copied().expect("pinned by nudging");
        assert!(pos.0 > x0, "moved right ({} vs {x0})", pos.0);

        let (mut other, _) = traffic_fixture();
        assert!(other.agents_select_node(&NodeKey::pane(1)));
        run_colon(&mut other, ":agents grab on");
        for _ in 0..3 {
            run_colon(&mut other, ":agents nudge right");
        }
        run_colon(&mut other, ":agents grab off");
        assert_eq!(observe(&wm), observe(&other));

        // Esc in grab mode drops the grab without dismissing the window.
        press(&mut wm, KeyCode::Char('g'));
        press(&mut wm, KeyCode::Esc);
        assert!(!wm.agents_window().grab_mode && wm.agents_window().active);
        // Enter drops it too, and grabbing needs a selected node.
        press(&mut wm, KeyCode::Char('g'));
        press(&mut wm, KeyCode::Enter);
        assert!(!wm.agents_window().grab_mode);
        let (mut bare, _) = traffic_fixture();
        run_colon(&mut bare, ":agents grab on");
        assert!(!bare.agents_window().grab_mode, "no node selected: grab refused");
    });
}

#[test]
fn window_right_click_and_u_unpin_node() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        run_colon(&mut wm, ":agents node-pin pane:1 300 300");
        assert!(wm.agents_window().layout.pinned.contains_key(&NodeKey::pane(1)));
        let (c, r) = node_cell(&wm, area, "pane:1");
        mouse(&mut wm, area, MouseEventKind::Down(MouseButton::Right), c, r);
        assert!(wm.agents_window().layout.pinned.is_empty(), "right-click unpins");

        run_colon(&mut wm, ":agents node-pin pane:1 300 300");
        run_colon(&mut wm, ":agents select pane:1");
        press(&mut wm, KeyCode::Char('u'));
        assert!(wm.agents_window().layout.pinned.is_empty(), "u unpins the selected node");

        let (mut other, _) = traffic_fixture();
        run_colon(&mut other, ":agents node-pin pane:1 300 300");
        run_colon(&mut other, ":agents select pane:1");
        run_colon(&mut other, ":agents node-unpin");
        assert_eq!(observe(&wm), observe(&other));
    });
}

#[test]
fn window_colon_node_forms() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        let before = observe(&wm);
        run_colon(&mut wm, ":agents select pane:99");
        run_colon(&mut wm, ":agents node-pin pane:99 10 10");
        run_colon(&mut wm, ":agents node-unpin pane:1");
        run_colon(&mut wm, ":agents edge pane:1 pane:2");
        assert_eq!(observe(&wm), before, "unknown nodes and edges change nothing");
        for bad in [
            ":agents node-pin pane:1 2000 5",
            ":agents node-pin pane:1 5",
            ":agents node-pin nonsense 5 5",
            ":agents select nonsense",
            ":agents edge user user",
            ":agents nudge sideways",
            ":agents grab maybe",
        ] {
            let res = parse_colon_command(bad).unwrap();
            assert!(res.is_err(), "`{bad}` must be a usage error, got {res:?}");
        }
    });
}

/// Observable window state for the cross-path parity test.
fn state_of(wm: &WindowManager, area: Rect) -> (Option<Rect>, Observed) {
    (wm.agents_window_rect(ws_of(wm, area)), observe(wm))
}

#[test]
fn parity_window_geometry_key_mouse_colon_match() {
    use bilterm::wm::agents_geometry::AgentsHit;
    type Path = Box<dyn Fn(&mut WindowManager, Rect)>;
    struct Case {
        name: &'static str,
        setup: fn(&mut WindowManager),
        key: Option<Path>,
        mouse: Option<Path>,
        colon: &'static str,
    }
    fn select_user(wm: &mut WindowManager) {
        assert!(wm.agents_select_node(&user()));
    }
    fn select_pane2_focus1(wm: &mut WindowManager) {
        wm.focus_and_raise(1);
        wm.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
        assert!(wm.agents_select_node(&NodeKey::pane(2)));
    }
    fn select_pane1(wm: &mut WindowManager) {
        assert!(wm.agents_select_node(&NodeKey::pane(1)));
    }
    fn pinned_pane1(wm: &mut WindowManager) {
        assert!(wm.pin_agents_node(&NodeKey::pane(1), 300, 300));
        assert!(wm.agents_select_node(&NodeKey::pane(1)));
    }
    fn nothing(_: &mut WindowManager) {}

    let cases: Vec<Case> = vec![
        Case {
            name: "Move",
            setup: nothing,
            key: Some(Box::new(|wm, _| {
                for _ in 0..3 {
                    press_mod(wm, KeyCode::Right, KeyModifiers::SHIFT);
                }
                for _ in 0..2 {
                    press_mod(wm, KeyCode::Down, KeyModifiers::SHIFT);
                }
            })),
            mouse: Some(Box::new(|wm, area| {
                let r = win_rect(wm, area);
                drag(wm, area, (r.x + 5, r.y), (r.x + 8, r.y + 2));
            })),
            colon: ":agents move 3 2",
        },
        Case {
            name: "Resize",
            setup: nothing,
            key: Some(Box::new(|wm, _| {
                for _ in 0..5 {
                    press_mod(wm, KeyCode::Right, KeyModifiers::ALT);
                }
                for _ in 0..3 {
                    press_mod(wm, KeyCode::Down, KeyModifiers::ALT);
                }
            })),
            mouse: Some(Box::new(|wm, area| {
                let r = win_rect(wm, area);
                let (right, bottom) = (r.x + r.width - 1, r.y + r.height - 1);
                drag(wm, area, (right, bottom), (right + 5, bottom + 3));
            })),
            colon: ":agents resize 5 3",
        },
        Case {
            name: "Maximize",
            setup: nothing,
            key: Some(Box::new(|wm, _| press(wm, KeyCode::Char('m')))),
            mouse: Some(Box::new(|wm, area| {
                let r = win_rect(wm, area);
                click(wm, area, badge_cols(wm, r).1, r.y);
            })),
            colon: ":agents max",
        },
        Case {
            name: "Split",
            setup: nothing,
            key: Some(Box::new(|wm, _| press(wm, KeyCode::Char('>')))),
            mouse: Some(Box::new(|wm, area| {
                let rects = wm.agents_pane_rects(ws_of(wm, area)).unwrap();
                let w = u32::from(rects.inner.width);
                let d = (0..rects.inner.width)
                    .find(|d| (u32::from(*d) * 100 + w / 2) / w == 60)
                    .expect("a column giving 60%");
                drag(wm, area, (rects.splitter_col, rects.inner.y + 1), (rects.inner.x + d, rects.inner.y + 1));
            })),
            colon: ":agents split 60",
        },
        Case {
            name: "SelectNode",
            setup: nothing,
            key: None,
            mouse: Some(Box::new(|wm, area| {
                let (c, r) = node_cell(wm, area, "pane:2");
                click(wm, area, c, r);
            })),
            colon: ":agents select pane:2",
        },
        Case {
            name: "SelectEdge",
            setup: select_user,
            key: Some(Box::new(|wm, _| press(wm, KeyCode::Tab))),
            mouse: Some(Box::new(|wm, area| {
                let rects = wm.agents_pane_rects(ws_of(wm, area)).unwrap();
                let scene = wm.agents_graph_scene();
                let find = |k: &str| scene.nodes.iter().find(|n| n.key.as_str() == k).unwrap().clone();
                let (a, b) = (find("pane:1"), find("user"));
                let want = AgentsHit::Edge(a.key.clone(), b.key.clone());
                let (x, y) = bilterm::wm::agents_geometry::edge_curve_cells(&a, &b, rects.graph)
                    .into_iter()
                    .map(|(x, y)| (rects.graph.x + x as u16, rects.graph.y + y as u16))
                    .find(|&(x, y)| wm.agents_hit(ws_of(wm, area), x, y) == want)
                    .expect("edge cell");
                click(wm, area, x, y);
            })),
            colon: ":agents edge user pane:1",
        },
        Case {
            name: "Activate",
            setup: select_pane2_focus1,
            key: Some(Box::new(|wm, _| press(wm, KeyCode::Enter))),
            mouse: Some(Box::new(|wm, area| {
                let (c, r) = node_cell(wm, area, "pane:2");
                click(wm, area, c, r);
            })),
            colon: ":agents enter",
        },
        Case {
            name: "PinNode",
            setup: select_pane1,
            key: None,
            mouse: Some(Box::new(|wm, area| {
                let rects = wm.agents_pane_rects(ws_of(wm, area)).unwrap();
                let from = node_cell(wm, area, "pane:1");
                let to = (rects.graph.x + rects.graph.width / 3, rects.graph.y + rects.graph.height / 2);
                drag(wm, area, from, to);
            })),
            // Permille of the cell the drag ends on, computed independently of the handler.
            colon: ":agents node-pin pane:1 PM",
        },
        Case {
            name: "UnpinNode",
            setup: pinned_pane1,
            key: Some(Box::new(|wm, _| press(wm, KeyCode::Char('u')))),
            mouse: Some(Box::new(|wm, area| {
                let (c, r) = node_cell(wm, area, "pane:1");
                mouse(wm, area, MouseEventKind::Down(MouseButton::Right), c, r);
            })),
            colon: ":agents node-unpin",
        },
    ];

    for case in cases {
        let run_path = |path: &Path| {
            let (mut wm, area) = traffic_fixture();
            (case.setup)(&mut wm);
            path(&mut wm, area);
            state_of(&wm, area)
        };
        // PinNode: derive the colon form from what the mouse path produced.
        let colon = if case.colon.ends_with(" PM") {
            let (wm, _) = {
                let (mut wm, area) = traffic_fixture();
                (case.mouse.as_ref().unwrap())(&mut wm, area);
                (wm, area)
            };
            let p = wm.agents_window().layout.pinned[&NodeKey::pane(1)];
            case.colon.replace("PM", &format!("{} {}", (p.0 * 1000.0).round() as u16, (p.1 * 1000.0).round() as u16))
        } else {
            case.colon.to_string()
        };
        let by_colon = {
            let (mut wm, area) = traffic_fixture();
            (case.setup)(&mut wm);
            run_colon(&mut wm, &colon);
            state_of(&wm, area)
        };
        if let Some(path) = &case.key {
            assert_eq!(run_path(path), by_colon, "{}: key path vs `{colon}`", case.name);
        }
        if let Some(path) = &case.mouse {
            assert_eq!(run_path(path), by_colon, "{}: mouse path vs `{colon}`", case.name);
        }
    }
}

#[test]
fn window_swallows_pointer_events_and_ignores_stale_drags() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        wm.focus_and_raise(1);
        wm.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
        let r = win_rect(&wm, area);
        let ws = ws_of(&wm, area);
        // Wheel and right-click inside the window are swallowed, whatever lies beneath.
        for kind in [MouseEventKind::ScrollUp, MouseEventKind::Down(MouseButton::Right), MouseEventKind::Moved] {
            mouse(&mut wm, area, kind, r.x + 3, r.y + 3);
        }
        assert_eq!(wm.active_pane_id(), 1);
        // The window is not modal: a click elsewhere reaches the pane under it, blurs the window and
        // does not close it.
        let outside = (ws.x + ws.width - 1, ws.y + ws.height - 1);
        assert!(!bilterm::wm::rect_contains(r, outside.0, outside.1));
        click(&mut wm, area, outside.0, outside.1);
        assert_eq!(wm.active_pane_id(), 2, "the click reached the pane under the pointer");
        assert!(wm.agents_window().active, "a click elsewhere does not close the window");
        assert!(!wm.agents_window().focused, "the click released the keyboard");
        // ... a drag release without a press is a no-op.
        mouse(&mut wm, area, MouseEventKind::Up(MouseButton::Left), r.x + 3, r.y + 3);
        assert!(wm.agents_window().drag.is_none());
    });
}

// ---- Plan 50-16: pin badge and independent panel scale ----

fn title_text(wm: &WindowManager, area: Rect) -> String {
    let r = win_rect(wm, area);
    let buf = render(wm, area);
    lines(&buf)[usize::from(r.y)].clone()
}

#[test]
fn window_tracer_pin_badge_click_pins() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        assert!(title_text(&wm, area).contains('◇'), "unpinned title shows the hollow pin");
        let r = win_rect(&wm, area);
        let x = bilterm::ui::agents_ui::agents_pin_badge_x(r).expect("pin badge fits at 120 cols");
        click(&mut wm, area, x, r.y);
        assert!(wm.agents_window().pinned, "badge click pins");
        assert!(wm.agents_window().drag.is_none(), "a badge click does not start a drag");
        let title = title_text(&wm, area);
        assert!(title.contains('◆') && !title.contains('◇'), "pinned title shows the filled pin: {title}");

        press(&mut wm, KeyCode::Esc);
        assert!(wm.agents_window().active, "Esc keeps a pinned window open");
        assert!(!wm.agents_window().captures_input(), "Esc releases the keyboard");

        let r = win_rect(&wm, area);
        click(&mut wm, area, r.x + 3, r.y + 3);
        assert!(wm.agents_window().captures_input(), "a click inside takes the keyboard back");

        press(&mut wm, KeyCode::Char('*'));
        assert!(!wm.agents_window().pinned, "`*` unpins");
        assert!(title_text(&wm, area).contains('◇'));
    });
}

#[test]
fn window_star_and_colon_pin_match_badge() {
    with_temp_xdg(|| {
        let by_badge = {
            let (mut wm, area) = traffic_fixture();
            let r = win_rect(&wm, area);
            click(&mut wm, area, bilterm::ui::agents_ui::agents_pin_badge_x(r).unwrap(), r.y);
            observe(&wm)
        };
        let by_key = {
            let (mut wm, _) = traffic_fixture();
            press(&mut wm, KeyCode::Char('*'));
            observe(&wm)
        };
        let by_colon = {
            let (mut wm, area) = traffic_fixture();
            run_colon(&mut wm, ":agents pin toggle");
            let _ = area;
            observe(&wm)
        };
        assert_eq!(by_badge, by_key);
        assert_eq!(by_key, by_colon);
        assert!(wm_pinned(":agents pin") && !wm_pinned(":agents unpin"));
    });
}

fn wm_pinned(cmd: &str) -> bool {
    let (mut wm, _) = traffic_fixture();
    run_colon(&mut wm, ":agents pin");
    run_colon(&mut wm, cmd);
    wm.agents_window().pinned
}

#[test]
fn window_pin_esc_releases_keyboard() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        run_colon(&mut wm, ":agents pin");
        press(&mut wm, KeyCode::Esc);
        assert!(wm.agents_window().active);
        assert!(!wm.agents_window().captures_input());
        run_colon(&mut wm, ":agents unpin");
        assert!(!wm.agents_window().pinned);
        run_colon(&mut wm, ":agents zoom");
        assert!(wm.agents_window().captures_input());
    });
}

fn scale_of(wm: &WindowManager) -> u16 {
    wm.scale_pct(PanelTarget::Agents)
}

fn ctrl_wheel(wm: &mut WindowManager, area: Rect, up: bool, col: u16, row: u16) {
    let kind = if up { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown };
    let ev = MouseEvent { kind, column: col, row, modifiers: KeyModifiers::CONTROL };
    handle_wm_mouse_event(wm, ev, area).unwrap();
}

#[test]
fn window_ctrl_wheel_scales_agents_panel_on_gui_surface() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        wm.set_gui_surface(true);
        let r = win_rect(&wm, area);
        ctrl_wheel(&mut wm, area, true, r.x + 3, r.y + 3);
        assert_eq!(scale_of(&wm), 110);
        ctrl_wheel(&mut wm, area, false, r.x + 3, r.y + 3);
        ctrl_wheel(&mut wm, area, false, r.x + 3, r.y + 3);
        assert_eq!(scale_of(&wm), 100 - 10);
        assert_eq!(wm.scale_pct(PanelTarget::Drawer), 100, "drawer scale untouched");
        assert_eq!(wm.scale_pct(PanelTarget::Mcp), 100, "MCP scale untouched");
        assert!(wm.agents_window().active);
    });
}

#[test]
fn window_bracket_keys_step_scale() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        press(&mut wm, KeyCode::Char(']'));
        assert_eq!(wm.scale_setting(PanelTarget::Agents), 110);
        let msg = wm.active_pane().status_message().unwrap_or_default().to_string();
        assert!(msg.contains("110%") && msg.contains("(applies in the GUI only)"), "{msg}");
        press(&mut wm, KeyCode::Char('['));
        press(&mut wm, KeyCode::Char('['));
        assert_eq!(wm.scale_setting(PanelTarget::Agents), 90);
        for _ in 0..5 {
            press(&mut wm, KeyCode::Char('['));
        }
        assert_eq!(wm.scale_setting(PanelTarget::Agents), 75, "clamped at 75");
        for _ in 0..20 {
            press(&mut wm, KeyCode::Char(']'));
        }
        assert_eq!(wm.scale_setting(PanelTarget::Agents), 200, "clamped at 200");
        wm.set_gui_surface(true);
        press(&mut wm, KeyCode::Char('['));
        let msg = wm.active_pane().status_message().unwrap_or_default().to_string();
        assert!(msg.contains("190%") && !msg.contains("GUI only"), "{msg}");
    });
}

#[test]
fn window_colon_scale_absolute_and_relative() {
    let parsed = |s: &str| parse_colon_command(s).unwrap();
    assert_eq!(parsed(":agents scale 150"), Ok(Action::PanelScaleOf(PanelTarget::Agents, 150)));
    assert_eq!(parsed(":agents scale +10"), Ok(Action::Agents(AgentsOp::ScaleStep(10))));
    assert_eq!(parsed(":agents scale -10"), Ok(Action::Agents(AgentsOp::ScaleStep(-10))));
    for bad in [":agents scale 300", ":agents scale 74", ":agents scale x", ":agents scale +x", ":agents scale", ":agents scale 1 2"] {
        assert!(parsed(bad).is_err(), "`{bad}` must be a usage error");
    }
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        run_colon(&mut wm, ":agents scale 150");
        assert_eq!(wm.scale_setting(PanelTarget::Agents), 150);
        assert_eq!(wm.scale_setting(PanelTarget::Drawer), 100);
        run_colon(&mut wm, ":agents scale -10");
        assert_eq!(wm.scale_setting(PanelTarget::Agents), 140);
    });
}

#[test]
fn window_panel_scale_sets_all_panels() {
    let mut wm = WindowManager::new_headless(30, 100);
    run_colon(&mut wm, ":panel scale 125");
    for t in [PanelTarget::Drawer, PanelTarget::Mcp, PanelTarget::Agents] {
        assert_eq!(wm.scale_setting(t), 125, "{t:?}");
    }
}

fn gui_max_cell_width(wm: &mut WindowManager, area: Rect) -> u16 {
    use bilterm::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
    wm.set_gui_surface(true);
    let mut renderer = GuiWorkspaceRenderer::new(8, 16);
    let mut surface = GuiFrameSurface::new(8, 16);
    renderer.render_workspace_with_cell_size(wm, area, 8, 16, &mut surface);
    let r = wm.agents_window_rect(ws_of(wm, area)).expect("window rect");
    let (cw, ch) = (8.0f32, 16.0f32);
    // Glyphs whose cell origin lies inside the (unscaled) window rect.
    surface
        .glyphs
        .iter()
        .filter(|g| g.x_px >= f32::from(r.x) * cw && g.x_px < f32::from(r.x + r.width) * cw && g.y_px >= f32::from(r.y) * ch && g.y_px < f32::from(r.y + r.height) * ch)
        .map(|g| g.cell_width_px)
        .max()
        .unwrap_or(0)
}

#[test]
fn window_gui_scaled_agents_glyphs_wider() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let base = gui_max_cell_width(&mut wm, area);
        run_colon(&mut wm, ":agents scale 150");
        let big = gui_max_cell_width(&mut wm, area);
        assert!(base > 0 && big > base, "agents glyph cells should be wider at 150% ({big} vs {base})");
    });
}

#[test]
fn parity_pin_scale_key_mouse_colon_match() {
    with_temp_xdg(|| {
        let by_key = {
            let (mut wm, _) = traffic_fixture();
            wm.set_gui_surface(true);
            press(&mut wm, KeyCode::Char(']'));
            (scale_of(&wm), observe(&wm))
        };
        let by_mouse = {
            let (mut wm, area) = traffic_fixture();
            wm.set_gui_surface(true);
            let r = win_rect(&wm, area);
            ctrl_wheel(&mut wm, area, true, r.x + 3, r.y + 3);
            (scale_of(&wm), observe(&wm))
        };
        let by_colon = {
            let (mut wm, _) = traffic_fixture();
            wm.set_gui_surface(true);
            run_colon(&mut wm, ":agents scale +10");
            (scale_of(&wm), observe(&wm))
        };
        assert_eq!(by_key.0, 110);
        assert_eq!(by_key, by_mouse);
        assert_eq!(by_key, by_colon);
    });
}

// ---- Plan 50-10: conversation (snoop) pane ----

fn snoop_rects(wm: &WindowManager, area: Rect) -> bilterm::wm::agents_geometry::AgentsPaneRects {
    wm.agents_pane_rects(ws_of(wm, area)).expect("pane rects")
}

fn snoop_lay(wm: &WindowManager, area: Rect) -> bilterm::ui::agents_ui::SnoopLayout {
    bilterm::ui::agents_ui::snoop_layout(wm, &snoop_rects(wm, area))
}

fn ctrl_o(wm: &mut WindowManager) {
    press_mod(wm, KeyCode::Char('o'), KeyModifiers::CONTROL);
}

fn last_copy(wm: &WindowManager) -> Option<String> {
    wm.agents_window().snoop.last_copy.clone()
}

fn line_cell(wm: &WindowManager, area: Rect, seq: u64) -> (u16, u16) {
    let lay = snoop_lay(wm, area);
    let y = lay.lines.iter().find(|(_, s)| *s == seq).unwrap_or_else(|| panic!("line {seq} not visible")).0;
    (lay.lines_area.x + 2, y)
}

#[test]
fn window_tracer_snoop_line_select_and_copy() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        assert_eq!(wm.traffic().latest_seq(), 2, "one event per broadcast target");
        ctrl_o(&mut wm);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Snoop);
        let text = lines(&render(&wm, area));
        assert!(
            text.iter().any(|l| l.contains("you→#1") && l.contains("echo hi")),
            "missing conversation line for the first broadcast target"
        );
        assert!(text.iter().any(|l| l.contains("you→#2") && l.contains("echo hi")));

        // The newest line is selected by default; `k` moves to the previous one.
        press(&mut wm, KeyCode::Char('k'));
        assert_eq!(wm.agents_window().snoop.selected_seq, Some(1));
        press(&mut wm, KeyCode::Char('y'));
        let copied = last_copy(&wm).expect("y copies the selected line");
        assert!(copied.contains("you -> #1") && copied.contains("echo hi"), "{copied}");

        // Mouse: click the other line, then the [copy] badge.
        let (cx, cy) = line_cell(&wm, area, 2);
        click(&mut wm, area, cx, cy);
        assert_eq!(wm.agents_window().snoop.selected_seq, Some(2));
        let badge = snoop_lay(&wm, area).copy;
        assert!(badge.width > 0, "the [copy] badge fits at 120 columns");
        click(&mut wm, area, badge.x + 1, badge.y);
        let by_mouse = last_copy(&wm).expect("badge copies");
        assert!(by_mouse.contains("you -> #2") && by_mouse.contains("echo hi"), "{by_mouse}");

        // Colon: same result as `y` on the same selection.
        let (mut other, _) = traffic_fixture();
        run_colon(&mut other, ":agents focus snoop");
        run_colon(&mut other, ":agents line 2");
        run_colon(&mut other, ":agents copy");
        assert_eq!(last_copy(&other), Some(by_mouse));
        assert_eq!(observe(&wm).snoop, observe(&other).snoop);
    });
}

// ---- Plan 50-10 Task 2: filters, follow, scroll ----

/// Broadcast `echo hi` (seq 1: you->#1, seq 2: you->#2) plus an agent chatter pair and an API exec.
fn snoop_fixture() -> (WindowManager, Rect) {
    let (mut wm, area) = traffic_fixture();
    let now = Instant::now();
    wm.record_traffic(TrafficEndpoint::Pane(1), TrafficEndpoint::Pane(2), TrafficKind::Message, "pong ECHO back", now);
    wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(2), TrafficKind::Exec, "ls -la", now);
    (wm, area)
}

fn seqs(wm: &WindowManager) -> Vec<u64> {
    wm.snoop_lines().iter().map(|l| l.seq).collect()
}

fn focus_snoop(wm: &mut WindowManager) {
    run_colon(wm, ":agents focus snoop");
}

fn type_text(wm: &mut WindowManager, text: &str) {
    for c in text.chars() {
        let mods = if c.is_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
        press_mod(wm, KeyCode::Char(c), mods);
    }
}

fn click_button(wm: &mut WindowManager, area: Rect, which: bilterm::ui::agents_ui::SnoopButton) {
    let lay = snoop_lay(wm, area);
    let (r, _) = lay.header_buttons.iter().find(|(_, b)| *b == which).expect("header button fits");
    click(wm, area, r.x + 1, r.y);
}

#[test]
fn snoop_agent_filter() {
    with_temp_xdg(|| {
        let prep = || {
            let (mut wm, area) = snoop_fixture();
            focus_snoop(&mut wm);
            run_colon(&mut wm, ":agents line 3");
            (wm, area)
        };
        let (mut by_key, _) = prep();
        press(&mut by_key, KeyCode::Char('a'));
        assert_eq!(by_key.agents_window().snoop.filter_agent, Some(NodeKey::pane(1)));
        assert_eq!(seqs(&by_key), vec![1, 3], "only events to or from #1");

        let (mut by_button, area) = prep();
        click_button(&mut by_button, area, bilterm::ui::agents_ui::SnoopButton::Agent);
        let (mut by_colon, _) = prep();
        run_colon(&mut by_colon, ":agents filter agent");
        assert_eq!(observe(&by_key), observe(&by_button));
        assert_eq!(observe(&by_key), observe(&by_colon));

        let (mut by_label, _) = prep();
        run_colon(&mut by_label, ":agents filter agent #1");
        assert_eq!(by_label.agents_window().snoop.filter_agent, by_key.agents_window().snoop.filter_agent);
        assert_eq!(seqs(&by_label), seqs(&by_key));
    });
}

fn edge_click_cell(wm: &WindowManager, area: Rect, a: &str, b: &str) -> (u16, u16) {
    let rects = snoop_rects(wm, area);
    let scene = wm.agents_graph_scene();
    let find = |k: &str| scene.nodes.iter().find(|n| n.key.as_str() == k).unwrap().clone();
    let (na, nb) = (find(a), find(b));
    let pair = if na.key < nb.key { (na.key.clone(), nb.key.clone()) } else { (nb.key.clone(), na.key.clone()) };
    bilterm::wm::agents_geometry::edge_curve_cells(&na, &nb, rects.graph)
        .iter()
        .map(|&(x, y)| (rects.graph.x + x as u16, rects.graph.y + y as u16))
        .find(|&(x, y)| {
            wm.agents_hit(ws_of(wm, area), x, y)
                == bilterm::wm::agents_geometry::AgentsHit::Edge(pair.0.clone(), pair.1.clone())
        })
        .expect("a cell of the edge that is not a node")
}

#[test]
fn snoop_pair_filter_from_edge_enter_and_click() {
    with_temp_xdg(|| {
        let want = Some((NodeKey::pane(2), NodeKey("user".to_string())));
        // Enter on a selected edge.
        let (mut by_enter, _) = snoop_fixture();
        run_colon(&mut by_enter, ":agents edge user pane:2");
        press(&mut by_enter, KeyCode::Enter);
        assert_eq!(by_enter.agents_window().snoop.filter_pair, want);
        assert_eq!(by_enter.agents_window().pane_focus, WindowPane::Snoop, "Enter focuses the conversation");
        assert_eq!(seqs(&by_enter), vec![2]);
        assert!(by_enter.agents_window().active, "the window stays open");

        // A second click on the already selected edge.
        let (mut by_click, area) = snoop_fixture();
        let (x, y) = edge_click_cell(&by_click, area, "user", "pane:2");
        click(&mut by_click, area, x, y);
        assert!(by_click.agents_window().snoop.filter_pair.is_none(), "the first click only selects");
        click(&mut by_click, area, x, y);
        assert_eq!(by_click.agents_window().snoop.filter_pair, want);
        assert_eq!(by_click.agents_window().pane_focus, WindowPane::Snoop);

        // `p` on a line of the pair, the header button and the colon forms.
        let prep = || {
            let (mut wm, area) = snoop_fixture();
            focus_snoop(&mut wm);
            run_colon(&mut wm, ":agents line 2");
            (wm, area)
        };
        let (mut by_p, _) = prep();
        press(&mut by_p, KeyCode::Char('p'));
        let (mut by_button, area) = prep();
        click_button(&mut by_button, area, bilterm::ui::agents_ui::SnoopButton::Pair);
        let (mut by_colon, _) = prep();
        run_colon(&mut by_colon, ":agents filter pair");
        let (mut by_labels, _) = prep();
        run_colon(&mut by_labels, ":agents filter pair you #2");
        for wm in [&by_p, &by_button, &by_colon, &by_labels] {
            assert_eq!(wm.agents_window().snoop.filter_pair, want);
            assert_eq!(seqs(wm), vec![2]);
        }
        assert_eq!(observe(&by_p), observe(&by_button));
        assert_eq!(observe(&by_p), observe(&by_colon));
    });
}

#[test]
fn snoop_keyword_filter_case_insensitive() {
    with_temp_xdg(|| {
        let (mut by_key, _) = snoop_fixture();
        focus_snoop(&mut by_key);
        press(&mut by_key, KeyCode::Char('/'));
        assert_eq!(by_key.agents_window().snoop.keyword_entry.as_deref(), Some(""));
        type_text(&mut by_key, "ECHO");
        assert_eq!(by_key.agents_window().snoop.keyword_entry.as_deref(), Some("ECHO"));
        press(&mut by_key, KeyCode::Enter);
        assert!(by_key.agents_window().snoop.keyword_entry.is_none());
        assert_eq!(by_key.agents_window().snoop.filter_text.as_deref(), Some("ECHO"));
        assert_eq!(seqs(&by_key), vec![1, 2, 3], "matches `echo hi` and `ECHO back`");

        let (mut by_button, area) = snoop_fixture();
        click_button(&mut by_button, area, bilterm::ui::agents_ui::SnoopButton::Keyword);
        assert_eq!(by_button.agents_window().snoop.keyword_entry.as_deref(), Some(""));
        assert_eq!(by_button.agents_window().pane_focus, WindowPane::Snoop);
        type_text(&mut by_button, "ECHO");
        press(&mut by_button, KeyCode::Enter);
        assert_eq!(observe(&by_key).snoop, observe(&by_button).snoop);

        let (mut by_colon, _) = snoop_fixture();
        run_colon(&mut by_colon, ":agents filter text echo");
        assert_eq!(seqs(&by_colon), vec![1, 2, 3]);
        // Backspace edits the entry.
        let (mut wm, _) = snoop_fixture();
        focus_snoop(&mut wm);
        press(&mut wm, KeyCode::Char('/'));
        type_text(&mut wm, "lsx");
        press(&mut wm, KeyCode::Backspace);
        press(&mut wm, KeyCode::Enter);
        assert_eq!(seqs(&wm), vec![4]);
    });
}

#[test]
fn snoop_keyword_entry_esc_cancels_only_entry() {
    with_temp_xdg(|| {
        let (mut wm, area) = snoop_fixture();
        focus_snoop(&mut wm);
        run_colon(&mut wm, ":agents filter text hi");
        press(&mut wm, KeyCode::Char('/'));
        type_text(&mut wm, "xm*[");
        assert_eq!(wm.agents_window().snoop.keyword_entry.as_deref(), Some("xm*["));
        assert!(!wm.agents_window().maximized, "`m` types into the keyword");
        assert!(!wm.agents_window().pinned, "`*` types into the keyword");
        let rect = win_rect(&wm, area);
        press(&mut wm, KeyCode::Esc);
        assert!(wm.agents_window().snoop.keyword_entry.is_none());
        assert_eq!(wm.agents_window().snoop.filter_text.as_deref(), Some("hi"), "filters are unchanged");
        assert!(wm.agents_window().active, "Esc only cancelled the entry");
        assert_eq!(win_rect(&wm, area), rect);
        press(&mut wm, KeyCode::Esc);
        assert!(!wm.agents_window().active, "the next Esc closes the window");
    });
}

#[test]
fn snoop_clear_and_remove_filters() {
    with_temp_xdg(|| {
        let setup = || {
            let (mut wm, area) = snoop_fixture();
            focus_snoop(&mut wm);
            run_colon(&mut wm, ":agents filter agent #2");
            run_colon(&mut wm, ":agents filter pair you #2");
            run_colon(&mut wm, ":agents filter text echo");
            (wm, area)
        };
        let (wm, _) = setup();
        assert_eq!(seqs(&wm), vec![2], "filters combine with AND");

        let (mut by_key, _) = setup();
        press(&mut by_key, KeyCode::Char('c'));
        assert_eq!(seqs(&by_key), vec![1, 2, 3, 4]);
        let (mut by_button, area) = setup();
        click_button(&mut by_button, area, bilterm::ui::agents_ui::SnoopButton::Clear);
        let (mut by_colon, _) = setup();
        run_colon(&mut by_colon, ":agents filter clear");
        assert_eq!(observe(&by_key), observe(&by_button));
        assert_eq!(observe(&by_key), observe(&by_colon));

        // Remove one filter at a time: key, chip `×` and colon agree.
        for (key, kind, word) in [
            ('A', SnoopFilterKind::Agent, "agent"),
            ('P', SnoopFilterKind::Pair, "pair"),
            ('T', SnoopFilterKind::Text, "text"),
        ] {
            let (mut by_key, _) = setup();
            press_mod(&mut by_key, KeyCode::Char(key), KeyModifiers::SHIFT);
            let (mut by_chip, area) = setup();
            let lay = snoop_lay(&by_chip, area);
            let (r, _) = lay.chips.iter().find(|(_, k)| *k == kind).expect("chip fits");
            click(&mut by_chip, area, r.x + r.width - 1, r.y);
            let (mut by_colon, _) = setup();
            run_colon(&mut by_colon, &format!(":agents filter remove {word}"));
            assert_eq!(observe(&by_key), observe(&by_chip), "{word}");
            assert_eq!(observe(&by_key), observe(&by_colon), "{word}");
            let st = &by_key.agents_window().snoop;
            let left = [st.filter_agent.is_some(), st.filter_pair.is_some(), st.filter_text.is_some()];
            assert_eq!(left.iter().filter(|b| **b).count(), 2, "exactly one filter removed ({word})");
        }
        // Clicking a chip's label (not its ×) removes nothing.
        let (mut wm, area) = setup();
        let lay = snoop_lay(&wm, area);
        let (r, _) = lay.chips[0];
        click(&mut wm, area, r.x, r.y);
        assert!(wm.agents_window().snoop.filter_agent.is_some());
        let text = lines(&render(&wm, area)).join("\n");
        assert!(text.contains("agent:#2") && text.contains("pair:") && text.contains("text:echo"), "chips are drawn");
    });
}

/// Fixture with enough lines to need scrolling.
fn long_fixture() -> (WindowManager, Rect) {
    let (mut wm, area) = snoop_fixture();
    for i in 0..60 {
        wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(1), TrafficKind::Exec, &format!("job {i}"), Instant::now());
    }
    (wm, area)
}

fn visible_seqs(wm: &WindowManager, area: Rect) -> Vec<u64> {
    snoop_lay(wm, area).lines.iter().map(|(_, s)| *s).collect()
}

/// WR-03: a scrolled-back view stays on the same lines while the ring evicts old events.
#[test]
fn snoop_scrolled_view_is_stable_under_ring_eviction() {
    with_temp_xdg(|| {
        let (mut wm, area) = long_fixture();
        for i in 0..600 {
            wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(1), TrafficKind::Exec, &format!("fill {i}"), Instant::now());
        }
        focus_snoop(&mut wm);
        press(&mut wm, KeyCode::PageUp);
        press(&mut wm, KeyCode::PageUp);
        assert!(!wm.agents_window().snoop.follow);
        let before = visible_seqs(&wm, area);
        assert!(!before.is_empty());
        let oldest_before = wm.snoop_lines().first().unwrap().seq;
        for i in 0..20 {
            wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(2), TrafficKind::Exec, &format!("more {i}"), Instant::now());
        }
        assert!(wm.snoop_lines().first().unwrap().seq > oldest_before, "the ring evicted old events");
        assert_eq!(visible_seqs(&wm, area), before, "eviction does not move the scrolled view");
    });
}

#[test]
fn snoop_follow_and_scroll() {
    with_temp_xdg(|| {
        let (mut wm, area) = long_fixture();
        focus_snoop(&mut wm);
        let latest = wm.traffic().latest_seq();
        assert!(wm.agents_window().snoop.follow);
        assert_eq!(visible_seqs(&wm, area).last(), Some(&latest));
        wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(2), TrafficKind::Exec, "newer", Instant::now());
        assert_eq!(visible_seqs(&wm, area).last(), Some(&(latest + 1)), "following keeps the newest line visible");

        press(&mut wm, KeyCode::PageUp);
        assert!(!wm.agents_window().snoop.follow, "PageUp turns follow off");
        let before = visible_seqs(&wm, area);
        assert!(before.last().unwrap() < &(latest + 1));
        wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(2), TrafficKind::Exec, "newest", Instant::now());
        assert_eq!(visible_seqs(&wm, area), before, "new traffic does not move the view");

        press(&mut wm, KeyCode::End);
        assert!(wm.agents_window().snoop.follow);
        assert_eq!(visible_seqs(&wm, area).last(), Some(&wm.traffic().latest_seq()));

        press(&mut wm, KeyCode::Home);
        assert!(!wm.agents_window().snoop.follow);
        assert_eq!(visible_seqs(&wm, area).first(), Some(&1), "Home shows the oldest line");
        let (mut other, oarea) = long_fixture();
        focus_snoop(&mut other);
        run_colon(&mut other, ":agents scroll top");
        assert_eq!(observe(&wm).snoop, observe(&other).snoop);
        assert_eq!(visible_seqs(&other, oarea).first(), Some(&1));

        // PageDown after PageUp returns toward the newest line without re-enabling follow.
        press(&mut wm, KeyCode::PageDown);
        assert!(!wm.agents_window().snoop.follow);
        assert!(visible_seqs(&wm, area).first() > Some(&1));
    });
}

#[test]
fn snoop_mouse_wheel_scrolls_and_follow_marker() {
    with_temp_xdg(|| {
        let (mut by_wheel, area) = long_fixture();
        let lay = snoop_lay(&by_wheel, area);
        mouse(&mut by_wheel, area, MouseEventKind::ScrollUp, lay.lines_area.x + 3, lay.lines_area.y + 2);
        assert!(!by_wheel.agents_window().snoop.follow, "wheel up disables follow");
        let (mut by_colon, _) = long_fixture();
        run_colon(&mut by_colon, ":agents scroll -3");
        assert_eq!(observe(&by_wheel).snoop, observe(&by_colon).snoop);
        mouse(&mut by_wheel, area, MouseEventKind::ScrollDown, lay.lines_area.x + 3, lay.lines_area.y + 2);
        run_colon(&mut by_colon, ":agents scroll 3");
        assert_eq!(observe(&by_wheel).snoop, observe(&by_colon).snoop);

        // The follow marker toggles follow, like `f` and `:agents follow`.
        let (mut by_click, area) = long_fixture();
        let marker = snoop_lay(&by_click, area).follow;
        assert!(marker.width > 0, "follow marker fits at 120 columns");
        click(&mut by_click, area, marker.x + 1, marker.y);
        assert!(!by_click.agents_window().snoop.follow);
        let (mut by_f, _) = long_fixture();
        focus_snoop(&mut by_f);
        press(&mut by_f, KeyCode::Char('f'));
        let (mut by_cmd, _) = long_fixture();
        run_colon(&mut by_cmd, ":agents focus snoop");
        run_colon(&mut by_cmd, ":agents follow");
        assert_eq!(observe(&by_click), observe(&by_f));
        assert_eq!(observe(&by_click), observe(&by_cmd));
        click(&mut by_click, area, marker.x + 1, marker.y);
        assert!(by_click.agents_window().snoop.follow, "a second click follows again");
    });
}

#[test]
fn snoop_colon_forms_and_errors() {
    with_temp_xdg(|| {
        let (mut wm, _) = long_fixture();
        run_colon(&mut wm, ":agents follow off");
        assert!(!wm.agents_window().snoop.follow);
        run_colon(&mut wm, ":agents follow on");
        assert!(wm.agents_window().snoop.follow);
        run_colon(&mut wm, ":agents scroll 5");
        assert!(wm.agents_window().snoop.follow, "scrolling down while following does nothing");
        run_colon(&mut wm, ":agents scroll -5");
        let up = wm.agents_window().snoop.scroll;
        run_colon(&mut wm, ":agents scroll 5");
        assert_eq!(wm.agents_window().snoop.scroll, up + 5);
        for bad in [
            ":agents filter",
            ":agents filter remove x",
            ":agents filter nope",
            ":agents filter text",
            ":agents scroll abc",
            ":agents follow maybe",
            ":agents line x",
            ":agents focus sideways",
        ] {
            let parsed = parse_colon_command(bad).expect("an :agents command");
            let err = parsed.expect_err(bad);
            assert!(err.starts_with("Usage:"), "{bad}: {err}");
        }
        run_colon(&mut wm, ":agents filter agent nosuchnode");
        assert!(wm.agents_window().snoop.filter_agent.is_none(), "an unknown node changes nothing");
    });
}

#[test]
fn snoop_keyword_is_substring_not_regex() {
    with_temp_xdg(|| {
        let (mut wm, _) = snoop_fixture();
        wm.record_traffic(TrafficEndpoint::Api, TrafficEndpoint::Pane(1), TrafficKind::Exec, "grep .*( x", Instant::now());
        run_colon(&mut wm, ":agents filter text .*(");
        assert_eq!(seqs(&wm), vec![5], "matched literally");
        run_colon(&mut wm, ":agents filter text [a-z]+");
        assert!(seqs(&wm).is_empty(), "no regex semantics");
        let long = "x".repeat(200);
        run_colon(&mut wm, &format!(":agents filter text {long}"));
        assert_eq!(wm.agents_window().snoop.filter_text.as_ref().map(|t| t.chars().count()), Some(64));
    });
}

#[test]
fn snoop_ctrl_o_cycles_and_clicks_focus() {
    with_temp_xdg(|| {
        let (mut wm, area) = snoop_fixture();
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Graph);
        for want in [WindowPane::Snoop, WindowPane::Compose, WindowPane::Graph] {
            ctrl_o(&mut wm);
            assert_eq!(wm.agents_window().pane_focus, want);
        }
        let (mut by_cmd, _) = snoop_fixture();
        run_colon(&mut by_cmd, ":agents focus");
        assert_eq!(by_cmd.agents_window().pane_focus, WindowPane::Snoop);

        // Clicking inside a pane focuses it.
        let lay = snoop_lay(&wm, area);
        click(&mut wm, area, lay.lines_area.x + 2, lay.lines_area.y + 1);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Snoop);
        let g = snoop_rects(&wm, area).graph;
        click(&mut wm, area, g.x + g.width - 1, g.y + g.height - 1);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Graph);

        // F6 keeps its global meaning: DeckNext after the leader while the window has the keyboard,
        // and plainly once the window is gone.
        ctrl_o(&mut wm);
        wm.focus_and_raise(1);
        wm.agents_window_mut().focused = true; // pane focus blurs the window; this test wants it back
        press_mod(&mut wm, KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(wm.is_leader_pending());
        press(&mut wm, KeyCode::F(6));
        let after_leader = wm.active_pane_id();
        assert_ne!(after_leader, 1, "leader then F6 ran DeckNext");
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Snoop, "F6 did not touch the window focus");
        wm.close_agents_window();
        press(&mut wm, KeyCode::F(6));
        assert_ne!(wm.active_pane_id(), after_leader, "plain F6 ran DeckNext");
    });
}

#[test]
fn snoop_edge_open_helper_filters_pair() {
    with_temp_xdg(|| {
        let (mut wm, _) = snoop_fixture();
        wm.close_agents_window();
        wm.open_agents_window_on_edge(&NodeKey::pane(1), &NodeKey::pane(2));
        assert!(wm.agents_window().active);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Snoop);
        assert_eq!(seqs(&wm), vec![3]);
    });
}

// ---- Plan 50-10 Task 3: disabled compose line and parity ----

fn pane_bytes(wm: &WindowManager) -> Vec<(usize, usize)> {
    wm.panes().iter().map(|(id, p)| (*id, p.vt().total_bytes_processed())).collect()
}

fn prompts(wm: &WindowManager) -> Vec<String> {
    wm.panes().values().map(|p| p.prompt().text()).collect()
}

fn status_of(wm: &WindowManager) -> Option<String> {
    wm.active_pane().status_message().map(str::to_string)
}

#[test]
fn compose_line_rendered_disabled() {
    let (wm, area) = snoop_fixture();
    let buf = render(&wm, area);
    let rows = lines(&buf);
    let compose = snoop_lay(&wm, area).compose;
    assert!(compose.width > 0);
    let line = &rows[usize::from(compose.y)];
    assert!(line.contains("send as me"), "{line}");
    assert!(line.contains("disabled"), "{line}");
    assert!(line.contains("[send]"), "{line}");
    assert!(line.find("send as me") < line.find("[send]"));
    let lay = snoop_lay(&wm, area);
    assert!(lay.lines.iter().all(|(y, _)| *y < compose.y), "the compose line is the last row");
    // Muted: the disabled line does not use the primary text colour of the conversation lines.
    let tokens = DesignTokens::from_palette(&bilterm::ui::theme::ThemePalette::default());
    let sym_x = compose.x + 4;
    assert_ne!(buf[(sym_x, compose.y)].fg, tokens.text_primary, "disabled style is muted");
}

#[test]
fn compose_typing_sends_nothing() {
    with_temp_xdg(|| {
        let (mut wm, _) = snoop_fixture();
        let (bytes, prompt, seq) = (pane_bytes(&wm), prompts(&wm), wm.traffic().latest_seq());
        ctrl_o(&mut wm);
        ctrl_o(&mut wm);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Compose);
        type_text(&mut wm, "hello");
        assert!(!wm.agents_window().maximized, "`m` is swallowed, not a window key");
        assert!(!wm.agents_window().pinned);
        assert_eq!(status_of(&wm), None, "typing alone reports nothing");
        press(&mut wm, KeyCode::Enter);
        let status = status_of(&wm).expect("Enter reports");
        assert!(status.contains("not enabled"), "{status}");
        assert_eq!(pane_bytes(&wm), bytes, "no byte reached any pane");
        assert_eq!(prompts(&wm), prompt, "no prompt text changed");
        assert_eq!(wm.traffic().latest_seq(), seq, "nothing recorded on the bus");
        assert!(wm.agents_window().active);
    });
}

/// WR-04: distinct raw names never collapse into one graph node.
#[test]
fn node_keys_are_injective_over_untrusted_names() {
    let key = |ep: TrafficEndpoint| NodeKey::from_endpoint(&ep);
    let plain = key(TrafficEndpoint::Session("work".into()));
    assert_eq!(plain.as_str(), "session:work", "ordinary names keep a readable key");
    let spoofed = key(TrafficEndpoint::Session("wo\u{200B}rk".into()));
    let bidi = key(TrafficEndpoint::Session("wo\u{202E}rk".into()));
    assert_ne!(spoofed, plain);
    assert_ne!(bidi, plain);
    assert_ne!(spoofed, bidi);
    let long_a = key(TrafficEndpoint::Agent(format!("{}A", "x".repeat(80))));
    let long_b = key(TrafficEndpoint::Agent(format!("{}B", "x".repeat(80))));
    assert_ne!(long_a, long_b, "names sharing a long prefix stay distinct");
    // A raw name that imitates an encoded key cannot collide with the encoded form.
    let imitation = key(TrafficEndpoint::Session(spoofed.as_str().trim_start_matches("session:").to_string()));
    assert_ne!(imitation, spoofed);
    assert_eq!(key(TrafficEndpoint::Session("wo\u{200B}rk".into())), spoofed, "keys are stable");
}

/// WR-02: geometry setters clamp against the scaled (virtual) workspace the window renders in.
#[test]
fn geometry_setters_use_scaled_virtual_workspace() {
    let (mut wm, area) = fixture(120, 40);
    wm.set_gui_surface(true);
    wm.set_scale(PanelTarget::Agents, 200);
    wm.open_agents_window(None);
    let vws = wm.panel_virtual_area(area, PanelTarget::Agents);
    assert_eq!((vws.width, vws.height), (60, 20));
    let stored = wm.set_agents_rect(Rect::new(0, 0, 200, 100));
    assert_eq!((stored.width, stored.height), (vws.width, vws.height), "clamped to the virtual area");
    // Nudging from the default rect stays inside the virtual area.
    wm.set_agents_rect(Rect::new(50, 15, 40, 12));
    wm.move_agents_window(30, 30);
    let r = wm.agents_window_rect(vws).expect("rect");
    assert!(r.x + r.width <= vws.width && r.y + r.height <= vws.height, "{r:?} within {vws:?}");
}

/// WR-07: a workspace modal raised while the window is open is drawn above it and gets the keys.
#[test]
fn exit_guard_wins_over_open_agents_window() {
    with_temp_xdg(|| {
        let (mut wm, area) = snoop_fixture();
        ctrl_o(&mut wm);
        ctrl_o(&mut wm);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Compose);
        wm.open_exit_guard("test trigger");
        let text = lines(&render(&wm, area)).join("\n");
        assert!(text.contains("EXIT GUARD"), "guard is not hidden behind the window");
        // Enter must answer the guard, not trigger the window's compose submit.
        press(&mut wm, KeyCode::Enter);
        assert_eq!(status_of(&wm).filter(|s| s.contains("not enabled")), None, "Enter did not reach compose");
        wm.open_exit_guard("again");
        press(&mut wm, KeyCode::Esc);
        assert!(wm.exit_guard_state().is_none(), "Esc cancelled the guard");
        assert!(wm.agents_window().active, "the window stayed open");
    });
}

/// WR-01: a window hidden by a tiny workspace does not swallow the keyboard or pastes.
#[test]
fn invisible_window_does_not_capture_input() {
    let (mut wm, _) = fixture(120, 40);
    wm.open_agents_window(None);
    assert!(wm.agents_window_captures_input());
    wm.resize_all(Rect::new(0, 0, 20, 8)).unwrap();
    assert!(wm.agents_window().active, "still open, just not drawn");
    assert!(!wm.agents_window_captures_input(), "invisible window must not capture");
    let before = prompts(&wm);
    bilterm::input::handle_wm_paste_event(&mut wm, "hello").unwrap();
    assert_ne!(prompts(&wm), before, "paste reaches the pane while the window is invisible");
    wm.resize_all(Rect::new(0, 0, 120, 40)).unwrap();
    assert!(wm.agents_window_captures_input());
}

/// CR-01: a paste while the window owns the keyboard never reaches the pane beneath.
#[test]
fn paste_while_window_captures_input_never_reaches_pane() {
    with_temp_xdg(|| {
        let (mut wm, _) = snoop_fixture();
        let (bytes, prompt, seq) = (pane_bytes(&wm), prompts(&wm), wm.traffic().latest_seq());
        let wrote = wm.active_pane().last_pty_write().to_vec();
        ctrl_o(&mut wm);
        ctrl_o(&mut wm);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Compose);
        let r = bilterm::input::handle_wm_paste_event(&mut wm, "rm -rf /\nsecret");
        assert!(r.is_ok());
        assert_eq!(pane_bytes(&wm), bytes, "no byte reached any pane");
        assert_eq!(prompts(&wm), prompt, "no prompt text changed");
        assert_eq!(wm.active_pane().last_pty_write(), wrote.as_slice(), "no PTY write");
        assert_eq!(wm.traffic().latest_seq(), seq);
        assert!(wm.agents_window().snoop.keyword_entry.is_none());
    });
}

/// CR-01: keyword entry (and only keyword entry) accepts pasted text, control characters dropped.
#[test]
fn paste_into_keyword_entry_filters_controls_and_skips_pane() {
    with_temp_xdg(|| {
        let (mut wm, _) = snoop_fixture();
        let prompt = prompts(&wm);
        let wrote = wm.active_pane().last_pty_write().to_vec();
        run_colon(&mut wm, ":agents focus snoop");
        press(&mut wm, KeyCode::Char('/'));
        assert_eq!(wm.agents_window().snoop.keyword_entry.as_deref(), Some(""));
        bilterm::input::handle_wm_paste_event(&mut wm, "ec\nho\x1b[31m").unwrap();
        assert_eq!(wm.agents_window().snoop.keyword_entry.as_deref(), Some("echo[31m"));
        assert_eq!(prompts(&wm), prompt);
        assert_eq!(wm.active_pane().last_pty_write(), wrote.as_slice());
    });
}

/// CR-01: with the window closed a paste still goes to the active pane.
#[test]
fn paste_with_window_closed_reaches_active_pane() {
    let (mut wm, _) = fixture(120, 40);
    bilterm::input::handle_wm_paste_event(&mut wm, "echo pasted").unwrap();
    assert!(wm.active_pane().prompt().text().contains("echo pasted"));
}

#[test]
fn compose_send_badge_matches_enter() {
    with_temp_xdg(|| {
        let (mut by_enter, _) = snoop_fixture();
        run_colon(&mut by_enter, ":agents focus compose");
        press(&mut by_enter, KeyCode::Enter);
        let (mut by_click, area) = snoop_fixture();
        let bytes = pane_bytes(&by_click);
        let send = snoop_lay(&by_click, area).send;
        assert!(send.width > 0, "[send] fits at 120 columns");
        click(&mut by_click, area, send.x + 1, send.y);
        assert_eq!(status_of(&by_click), status_of(&by_enter));
        assert!(status_of(&by_click).unwrap().contains("not enabled"));
        assert_eq!(observe(&by_click), observe(&by_enter));
        assert_eq!(pane_bytes(&by_click), bytes);
        assert_eq!(by_click.traffic().latest_seq(), by_enter.traffic().latest_seq());
        // A click elsewhere on the compose line only focuses it.
        let (mut wm, area) = snoop_fixture();
        let c = snoop_lay(&wm, area).compose;
        click(&mut wm, area, c.x + 2, c.y);
        assert_eq!(wm.agents_window().pane_focus, WindowPane::Compose);
        assert_eq!(status_of(&wm), None);
    });
}

#[test]
fn compose_gate_is_unavailable_in_phase50() {
    let (wm, _) = snoop_fixture();
    assert!(matches!(wm.compose_gate(), bilterm::wm::agents_window::ComposeGate::Unavailable(_)));
}

#[test]
fn compose_not_reachable_over_rpc() {
    with_temp_xdg(|| {
        for cmd in [":agents send hi", ":agents compose hi", ":agents submit", ":agents compose"] {
            let err = parse_colon_command(cmd).expect("an :agents command").expect_err(cmd);
            assert!(err.starts_with("Usage:"), "{cmd}: {err}");
        }
        assert!(agents_op_colon_form(&AgentsOp::ComposeSubmit).is_none(), "ComposeSubmit has no colon form");
        // Nothing the parser can produce is a ComposeSubmit.
        for op in sample_ops() {
            assert_ne!(op, AgentsOp::ComposeSubmit);
        }
    });
}

/// Everything a parity scenario compares.
fn parity_state(wm: &WindowManager) -> (Observed, Vec<(usize, usize)>, Option<String>) {
    (observe(wm), pane_bytes(wm), status_of(wm))
}

#[test]
fn parity_snoop_key_mouse_colon_match() {
    type Act = Box<dyn Fn(&mut WindowManager, Rect)>;
    struct Scenario {
        name: &'static str,
        long: bool,
        prep: &'static [&'static str],
        key: Act,
        mouse: Option<Act>,
        colon: Option<&'static str>,
    }
    let k = |code: KeyCode| -> Act { Box::new(move |wm, _| press(wm, code)) };
    let ks = |code: KeyCode, mods: KeyModifiers| -> Act { Box::new(move |wm, _| press_mod(wm, code, mods)) };
    let line_click = |seq: u64| -> Act {
        Box::new(move |wm, area| {
            let (x, y) = line_cell(wm, area, seq);
            click(wm, area, x, y)
        })
    };
    let button = |b: bilterm::ui::agents_ui::SnoopButton| -> Act { Box::new(move |wm, area| click_button(wm, area, b)) };
    let chip = |kind: SnoopFilterKind| -> Act {
        Box::new(move |wm, area| {
            let lay = snoop_lay(wm, area);
            let (r, _) = lay.chips.iter().find(|(_, k)| *k == kind).expect("chip");
            click(wm, area, r.x + r.width - 1, r.y)
        })
    };
    let at_rect = |pick: fn(&bilterm::ui::agents_ui::SnoopLayout) -> ratatui::layout::Rect| -> Act {
        Box::new(move |wm, area| {
            let r = pick(&snoop_lay(wm, area));
            click(wm, area, r.x + 1, r.y)
        })
    };
    let snoop = &[":agents focus snoop"];
    let sel3 = &[":agents focus snoop", ":agents line 3"];
    let scenarios = vec![
        Scenario { name: "focus", long: false, prep: &[], key: ks(KeyCode::Char('o'), KeyModifiers::CONTROL),
            mouse: Some(Box::new(|wm, area| {
                let l = snoop_lay(wm, area).lines_area;
                click(wm, area, l.x + 2, l.y + l.height - 1)
            })), colon: Some(":agents focus snoop") },
        Scenario { name: "step", long: false, prep: snoop, key: k(KeyCode::Char('k')), mouse: Some(line_click(3)), colon: Some(":agents line -1") },
        Scenario { name: "select", long: false, prep: snoop, key: Box::new(|wm, _| { press(wm, KeyCode::Char('k')); press(wm, KeyCode::Char('k')) }),
            mouse: Some(line_click(2)), colon: Some(":agents line 2") },
        Scenario { name: "pageup", long: true, prep: snoop, key: k(KeyCode::PageUp), mouse: None, colon: Some(":agents scroll -10") },
        Scenario { name: "home", long: true, prep: snoop, key: k(KeyCode::Home), mouse: None, colon: Some(":agents scroll top") },
        Scenario { name: "wheel", long: true, prep: snoop,
            key: Box::new(|wm, _| run_colon(wm, ":agents scroll -3")),
            mouse: Some(Box::new(|wm, area| {
                let l = snoop_lay(wm, area).lines_area;
                mouse(wm, area, MouseEventKind::ScrollUp, l.x + 2, l.y + 1)
            })), colon: Some(":agents scroll -3") },
        Scenario { name: "follow-toggle", long: true, prep: snoop, key: k(KeyCode::Char('f')), mouse: Some(at_rect(|l| l.follow)), colon: Some(":agents follow") },
        Scenario { name: "end", long: true, prep: &[":agents focus snoop", ":agents scroll top"], key: k(KeyCode::End), mouse: None, colon: Some(":agents follow on") },
        Scenario { name: "agent", long: false, prep: sel3, key: k(KeyCode::Char('a')), mouse: Some(button(bilterm::ui::agents_ui::SnoopButton::Agent)), colon: Some(":agents filter agent") },
        Scenario { name: "pair", long: false, prep: sel3, key: k(KeyCode::Char('p')), mouse: Some(button(bilterm::ui::agents_ui::SnoopButton::Pair)), colon: Some(":agents filter pair") },
        Scenario { name: "clear", long: false, prep: &[":agents focus snoop", ":agents filter agent #1", ":agents filter text echo"],
            key: k(KeyCode::Char('c')), mouse: Some(button(bilterm::ui::agents_ui::SnoopButton::Clear)), colon: Some(":agents filter clear") },
        Scenario { name: "remove-agent", long: false, prep: &[":agents focus snoop", ":agents filter agent #1", ":agents filter text echo"],
            key: ks(KeyCode::Char('A'), KeyModifiers::SHIFT), mouse: Some(chip(SnoopFilterKind::Agent)), colon: Some(":agents filter remove agent") },
        Scenario { name: "remove-text", long: false, prep: &[":agents focus snoop", ":agents filter agent #1", ":agents filter text echo"],
            key: ks(KeyCode::Char('T'), KeyModifiers::SHIFT), mouse: Some(chip(SnoopFilterKind::Text)), colon: Some(":agents filter remove text") },
        Scenario { name: "copy", long: false, prep: sel3, key: k(KeyCode::Char('y')), mouse: Some(at_rect(|l| l.copy)), colon: Some(":agents copy") },
        Scenario { name: "compose-submit", long: false, prep: &[":agents focus compose"], key: k(KeyCode::Enter), mouse: Some(at_rect(|l| l.send)), colon: None },
    ];
    for sc in &scenarios {
        let build = || {
            let (mut wm, area) = if sc.long { long_fixture() } else { snoop_fixture() };
            for cmd in sc.prep {
                run_colon(&mut wm, cmd);
            }
            (wm, area)
        };
        let by_key = with_temp_xdg(|| {
            let (mut wm, area) = build();
            (sc.key)(&mut wm, area);
            parity_state(&wm)
        });
        if let Some(m) = &sc.mouse {
            let by_mouse = with_temp_xdg(|| {
                let (mut wm, area) = build();
                m(&mut wm, area);
                parity_state(&wm)
            });
            assert_eq!(by_key, by_mouse, "{}: key vs mouse", sc.name);
        }
        if let Some(cmd) = sc.colon {
            let by_colon = with_temp_xdg(|| {
                let (mut wm, _) = build();
                run_colon(&mut wm, cmd);
                parity_state(&wm)
            });
            assert_eq!(by_key, by_colon, "{}: key vs `{cmd}`", sc.name);
        }
    }
}
