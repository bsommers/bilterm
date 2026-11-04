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
use bilterm::wm::{SplitDirection, WindowManager};
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
