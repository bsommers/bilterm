//! Agents window follow-ups: the window is a normal, non-modal window (keys follow focus, clicks on
//! other panes pass through), graph edges glow with recent communication, and the communication
//! topology is recorded for the session.

use std::sync::Mutex;

use bilterm::colon_command::parse_colon_command;
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::wm::{SplitDirection, WindowManager, rect_contains};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Isolates config/state directories for tests that drive real key dispatch.
fn with_temp_xdg<T>(f: impl FnOnce() -> T) -> T {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!(
        "bilterm-agents-nonmodal-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Serialized by ENV_LOCK.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_STATE_HOME", &dir);
        std::env::set_var("XDG_DATA_HOME", &dir);
    }
    let out = f();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn fixture() -> (WindowManager, Rect) {
    let mut wm = WindowManager::new_headless(40, 120);
    let area = Rect::new(0, 0, 120, 40);
    wm.split_focused(SplitDirection::Vertical).unwrap();
    wm.resize_all(area).unwrap();
    (wm, area)
}

fn traffic_fixture() -> (WindowManager, Rect) {
    let (mut wm, area) = fixture();
    wm.broadcast_command("echo hi").unwrap();
    wm.open_agents_window(None);
    (wm, area)
}

fn run_colon(wm: &mut WindowManager, cmd: &str) {
    let parsed = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    wm.apply(parsed).unwrap();
}

fn press(wm: &mut WindowManager, code: KeyCode) {
    handle_wm_key_event(wm, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

fn mouse(wm: &mut WindowManager, area: Rect, kind: MouseEventKind, col: u16, row: u16) {
    let ev = MouseEvent { kind, column: col, row, modifiers: KeyModifiers::NONE };
    handle_wm_mouse_event(wm, ev, area).unwrap();
}

fn click(wm: &mut WindowManager, area: Rect, col: u16, row: u16) {
    mouse(wm, area, MouseEventKind::Down(MouseButton::Left), col, row);
    mouse(wm, area, MouseEventKind::Up(MouseButton::Left), col, row);
}

fn ws_of(wm: &WindowManager, area: Rect) -> Rect {
    bilterm::ui::shell::ShellLayout::compute(area, wm.config(), wm.pane_count()).workspace
}

fn win_rect(wm: &WindowManager, area: Rect) -> Rect {
    wm.agents_window_rect(ws_of(wm, area)).expect("window rect")
}

fn prompts(wm: &WindowManager) -> Vec<String> {
    wm.panes().values().map(|p| p.prompt().text()).collect()
}

/// A workspace cell outside the window, inside pane `1` (left) or `2` (right).
fn outside_cell(wm: &WindowManager, area: Rect, right: bool) -> (u16, u16) {
    let ws = ws_of(wm, area);
    let r = win_rect(wm, area);
    let col = if right { ws.x + ws.width - 1 } else { ws.x };
    let row = ws.y + ws.height - 1;
    assert!(!rect_contains(r, col, row), "cell must lie outside the window");
    (col, row)
}

// ---- Non-modal window ----

#[test]
fn nonmodal_click_on_other_pane_passes_through_and_blurs() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        wm.focus_and_raise(1);
        wm.agents_window_mut().focused = true;
        assert!(wm.agents_window().captures_input());
        let (c, r) = outside_cell(&wm, area, true);
        click(&mut wm, area, c, r);
        assert_eq!(wm.active_pane_id(), 2, "the click reached the pane under the pointer");
        assert!(wm.agents_window().active, "the window stays open");
        assert!(!wm.agents_window().focused, "the click took focus away from the window");
        assert!(!wm.agents_window().captures_input());
    });
}

#[test]
fn nonmodal_keys_go_to_pane_when_window_not_focused() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let (c, r) = outside_cell(&wm, area, true);
        click(&mut wm, area, c, r);
        let before = prompts(&wm);
        let maximized = wm.agents_window().maximized;
        press(&mut wm, KeyCode::Char('m'));
        assert_eq!(wm.agents_window().maximized, maximized, "`m` did not reach the window");
        assert_ne!(prompts(&wm), before, "`m` was typed into the active pane");
        press(&mut wm, KeyCode::Esc);
        assert!(wm.agents_window().active, "Esc is not the window's while it has no focus");
    });
}

#[test]
fn nonmodal_click_inside_window_takes_focus_back() {
    with_temp_xdg(|| {
        let (mut wm, area) = traffic_fixture();
        let (c, r) = outside_cell(&wm, area, false);
        click(&mut wm, area, c, r);
        assert!(!wm.agents_window().captures_input());
        let w = win_rect(&wm, area);
        click(&mut wm, area, w.x + w.width / 2, w.y + w.height / 2);
        assert!(wm.agents_window().captures_input(), "a click inside focuses the window");
        press(&mut wm, KeyCode::Char('m'));
        assert!(wm.agents_window().maximized, "keys go to the focused window");
    });
}

#[test]
fn nonmodal_pane_focus_change_releases_window_focus() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        assert!(wm.agents_window().captures_input());
        wm.focus_and_raise(1);
        assert!(wm.agents_window().active);
        assert!(!wm.agents_window().captures_input(), "focusing a pane (leader keys) blurs the window");
        run_colon(&mut wm, ":agents zoom");
        assert!(wm.agents_window().captures_input(), "`:agents zoom` focuses it again");
    });
}

#[test]
fn nonmodal_blur_command_releases_focus_without_closing() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        run_colon(&mut wm, ":agents blur");
        assert!(wm.agents_window().active && !wm.agents_window().focused);
        assert!(parse_colon_command(":agents blur extra").unwrap().is_err());
    });
}

#[test]
fn nonmodal_exit_guard_still_wins_over_focused_window() {
    with_temp_xdg(|| {
        let (mut wm, _) = traffic_fixture();
        wm.open_exit_guard("test trigger");
        press(&mut wm, KeyCode::Esc);
        assert!(wm.exit_guard_state().is_none(), "Esc cancelled the guard");
        assert!(wm.agents_window().active, "the window was not touched");
    });
}

// ---- Edge heat: brightness follows recent communication frequency ----

use std::time::{Duration, Instant};

use bilterm::api::JsonValue;
use bilterm::ui::graph_raster::{GraphColours, draw_agent_graph, draw_agent_graph_hires, edge_rgb};
use bilterm::ui::pixel_canvas::PixelCanvasBuffer;
use bilterm::wm::agents_topology::{MIN_EDGE_BRIGHTNESS, edge_brightness, edge_heat};
use bilterm::wm::agents_window::{GraphScene, NodeKey, SceneEdge, SceneNode};
use bilterm::wm::traffic::{TrafficEndpoint as Ep, TrafficKind};

fn say(wm: &mut WindowManager, from: Ep, to: Ep, n: usize, at: Instant) {
    for _ in 0..n {
        wm.record_traffic(from.clone(), to.clone(), TrafficKind::Message, "hello", at);
    }
}

fn edge_between(scene: &GraphScene, a: &str, b: &str) -> SceneEdge {
    scene
        .edges
        .iter()
        .find(|e| {
            let (x, y) = (e.from.as_str(), e.to.as_str());
            (x == a && y == b) || (x == b && y == a)
        })
        .unwrap_or_else(|| panic!("no edge {a} - {b}"))
        .clone()
}

#[test]
fn heat_pure_function_monotone_in_count_and_age() {
    let base = Instant::now();
    let fade = Duration::from_secs(30);
    let now = base + Duration::from_secs(100);
    let at = |ago: u64| now - Duration::from_secs(ago);
    let h = |n: usize, ago: u64| edge_heat(vec![at(ago); n], now, fade);
    // More events: hotter.
    assert!(h(1, 1) < h(2, 1) && h(2, 1) < h(10, 1) && h(10, 1) < 1.0);
    // Older events: cooler, and zero once the window passed.
    assert!(h(3, 1) > h(3, 10) && h(3, 10) > h(3, 25));
    assert_eq!(h(3, 30), 0.0);
    assert_eq!(h(3, 500), 0.0);
    // Deterministic: same inputs, same output.
    assert_eq!(h(4, 7), h(4, 7));
    // A longer window keeps the same events hotter.
    assert!(edge_heat(vec![at(20)], now, Duration::from_secs(60)) > edge_heat(vec![at(20)], now, fade));
    // The brightness floor is positive and heat only raises it.
    assert_eq!(edge_brightness(0.0), MIN_EDGE_BRIGHTNESS);
    assert!(edge_brightness(0.0) > 0.0);
    assert!(edge_brightness(0.5) > MIN_EDGE_BRIGHTNESS);
}

#[test]
fn heat_more_talkative_pair_has_brighter_edge() {
    let (mut wm, _) = fixture();
    let base = Instant::now();
    say(&mut wm, Ep::Pane(1), Ep::Pane(2), 8, base);
    say(&mut wm, Ep::Api, Ep::Pane(1), 1, base);
    let scene = wm.agents_graph_scene_at(base);
    let chatty = edge_between(&scene, "pane:1", "pane:2");
    let quiet = edge_between(&scene, "api", "pane:1");
    assert!(chatty.brightness > quiet.brightness, "{} vs {}", chatty.brightness, quiet.brightness);
    assert!(quiet.brightness > MIN_EDGE_BRIGHTNESS);
    assert_eq!(chatty.count, 8);
}

#[test]
fn heat_decays_with_age_to_a_floor_but_edge_stays() {
    let (mut wm, _) = fixture();
    let base = Instant::now();
    run_colon(&mut wm, ":agents fade 20");
    say(&mut wm, Ep::Pane(1), Ep::Pane(2), 5, base);
    let b = |s: u64| edge_between(&wm.agents_graph_scene_at(base + Duration::from_secs(s)), "pane:1", "pane:2").brightness;
    assert!(b(0) > b(5) && b(5) > b(15), "{} {} {}", b(0), b(5), b(15));
    assert_eq!(b(20), MIN_EDGE_BRIGHTNESS);
    assert_eq!(b(100_000), MIN_EDGE_BRIGHTNESS, "never fully fades once the pair has spoken");
}

#[test]
fn heat_fade_window_is_configurable_by_config_key_and_command() {
    let (mut wm, _) = fixture();
    let base = Instant::now();
    say(&mut wm, Ep::Pane(1), Ep::Pane(2), 3, base);
    let at30 = |wm: &WindowManager| {
        edge_between(&wm.agents_graph_scene_at(base + Duration::from_secs(30)), "pane:1", "pane:2").brightness
    };
    assert_eq!(wm.config().agents_window.edge_fade_secs, 60, "default");
    let slow = at30(&wm);
    assert!(slow > MIN_EDGE_BRIGHTNESS);
    // Dotted config key path.
    run_colon(&mut wm, ":set agents_window.edge_fade_secs 10");
    assert_eq!(wm.config().agents_window.edge_fade_secs, 10);
    assert_eq!(at30(&wm), MIN_EDGE_BRIGHTNESS);
    // The :agents command.
    run_colon(&mut wm, ":agents fade 120");
    assert_eq!(wm.config().agents_window.edge_fade_secs, 120);
    assert!(at30(&wm) > slow);
    run_colon(&mut wm, ":agents fade");
    let d = JsonValue::parse(&wm.take_command_data().unwrap()).unwrap();
    assert_eq!(d.get("edge_fade_secs").and_then(JsonValue::as_i64), Some(120));
    assert!(parse_colon_command(":agents fade 0").unwrap().is_err());
    assert!(parse_colon_command(":agents fade 99999").unwrap().is_err());
    assert!(parse_colon_command(":agents fade x").unwrap().is_err());
    // TOML form.
    let cfg = bilterm::config::BiltermConfig::from_toml_str("[agents_window]\nedge_fade_secs = 5\n").unwrap();
    assert_eq!(cfg.agents_window.edge_fade_secs, 5);
}

fn scene_with_edge(brightness: f32) -> GraphScene {
    let node = |k: &str, x: f32| SceneNode {
        key: NodeKey(k.to_string()),
        label: k.to_string(),
        pos: (x, 0.5),
        state: None,
        selected: false,
    };
    GraphScene {
        nodes: vec![node("api", 0.1), node("user", 0.9)],
        edges: vec![SceneEdge {
            from: NodeKey("api".to_string()),
            to: NodeKey("user".to_string()),
            count: 1,
            last_seq: 1,
            active: false,
            selected: false,
            heat: 0.0,
            brightness,
        }],
    }
}

fn luma(c: (u8, u8, u8)) -> u32 {
    u32::from(c.0) * 3 + u32::from(c.1) * 6 + u32::from(c.2)
}

#[test]
fn heat_edge_colour_and_both_renderers_follow_brightness() {
    let (wm, _) = fixture();
    let colours = GraphColours::from_palette(&wm.config().palette());
    let (cold, mid, hot) = (scene_with_edge(MIN_EDGE_BRIGHTNESS), scene_with_edge(0.7), scene_with_edge(0.97));
    let c = |s: &GraphScene| luma(edge_rgb(&s.edges[0], &colours));
    let bg = luma(colours.bg);
    let dist = |v: u32| v.abs_diff(bg);
    assert!(dist(c(&cold)) > 0, "the floor stays visible against the background");
    assert!(dist(c(&cold)) < dist(c(&mid)) && dist(c(&mid)) < dist(c(&hot)));

    // Braille (terminal) renderer: the hotter edge draws brighter pixels.
    let bgpx = [colours.bg.0, colours.bg.1, colours.bg.2, 255];
    // Total contrast against the background; the (identical) nodes cancel out of the comparison.
    let strongest = |cv: &PixelCanvasBuffer| cv.rgba.chunks(4).map(|p| u64::from(luma((p[0], p[1], p[2])).abs_diff(bg))).sum::<u64>();
    let tui = |s: &GraphScene| {
        let mut cv = PixelCanvasBuffer::new(120, 80, bgpx);
        draw_agent_graph(&mut cv, s, &colours, 0);
        strongest(&cv)
    };
    assert!(tui(&cold) > 0, "cold edge still drawn");
    assert!(tui(&cold) < tui(&hot));
    // GUI vector renderer.
    let gui = |s: &GraphScene| {
        let mut cv = PixelCanvasBuffer::new(400, 200, bgpx);
        draw_agent_graph_hires(&mut cv, s, &colours, 0, (8.0, 16.0));
        strongest(&cv)
    };
    assert!(gui(&cold) > 0, "cold edge still drawn");
    assert!(gui(&cold) < gui(&hot));
}

// ---- Communication topology ----

#[test]
fn topology_survives_ring_eviction() {
    let (mut wm, _) = fixture();
    let base = Instant::now();
    say(&mut wm, Ep::Pane(1), Ep::Pane(2), 2, base);
    // Push the ring far past its 512 events with another pair.
    say(&mut wm, Ep::Api, Ep::User, 700, base);
    assert!(wm.traffic().dropped() > 0 && wm.traffic().len() <= 512);
    assert!(wm.traffic().iter().all(|e| e.from != Ep::Pane(1)), "the first pair left the ring");
    let topo = wm.traffic().topology();
    let rec = topo.pair(&NodeKey::pane(1), &NodeKey::pane(2)).expect("pair kept after eviction");
    assert_eq!((rec.count(), rec.first_seq(), rec.last_seq()), (2, 1, 2));
    assert_eq!(topo.len(), 2);
    // The graph keeps the edge (at its floor once the window passed).
    let scene = wm.agents_graph_scene_at(base + Duration::from_secs(10_000));
    assert_eq!(edge_between(&scene, "pane:1", "pane:2").brightness, MIN_EDGE_BRIGHTNESS);
    assert_eq!(edge_between(&scene, "api", "user").count, 700);
}

#[test]
fn topology_is_bounded_and_ignores_self_pairs() {
    let (mut wm, _) = fixture();
    let base = Instant::now();
    wm.record_traffic(Ep::Pane(1), Ep::Pane(1), TrafficKind::Input, "x", base);
    assert!(wm.traffic().topology().is_empty());
    for i in 0..400 {
        wm.record_traffic(Ep::Session(format!("s{i}")), Ep::User, TrafficKind::Broadcast, "x", base);
    }
    assert!(wm.traffic().topology().len() <= bilterm::wm::agents_topology::TOPOLOGY_CAP);
    assert!(wm.traffic().topology().dropped() > 0);
}

#[test]
fn topology_command_reports_pairs_as_data() {
    let (mut wm, _) = fixture();
    let base = Instant::now();
    say(&mut wm, Ep::Pane(1), Ep::Pane(2), 3, base);
    say(&mut wm, Ep::Api, Ep::Pane(2), 1, base);
    run_colon(&mut wm, ":agents topology");
    let d = JsonValue::parse(&wm.take_command_data().expect("data")).unwrap();
    assert_eq!(d.get("pair_count").and_then(JsonValue::as_i64), Some(2));
    let pairs = d.get("pairs").and_then(JsonValue::as_array).unwrap();
    assert_eq!(pairs[0].get("count").and_then(JsonValue::as_i64), Some(3), "busiest pair first");
    assert_eq!(pairs[0].get("a").and_then(JsonValue::as_str), Some("#1"));
    assert_eq!(pairs[0].get("b").and_then(JsonValue::as_str), Some("#2"));
    assert!(pairs[0].get("first_seq").is_some() && pairs[0].get("last_ago_ms").is_some());
    assert!(wm.active_pane().status_message().unwrap().contains("2 pairs"));
}

#[test]
fn topology_labels_are_sanitised() {
    let (mut wm, _) = fixture();
    let evil = Ep::Agent("bad\u{1b}[31m\u{202e}name".to_string());
    say(&mut wm, evil, Ep::Pane(1), 1, Instant::now());
    run_colon(&mut wm, ":agents topology");
    let text = wm.take_command_data().unwrap();
    assert!(!text.contains('\u{1b}') && !text.contains('\u{202e}'), "{text}");
}

fn runtime_dir(tag: &str) -> std::path::PathBuf {
    let rt = std::env::temp_dir().join(format!("bilterm-topo-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&rt);
    std::fs::create_dir_all(&rt).unwrap();
    // SAFETY: callers hold ENV_LOCK through with_temp_xdg.
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &rt) };
    rt
}

#[test]
fn topology_export_writes_private_file_and_rejects_bad_names() {
    with_temp_xdg(|| {
        let rt = runtime_dir("export");
        let (mut wm, _) = fixture();
        say(&mut wm, Ep::Pane(1), Ep::Pane(2), 2, Instant::now());
        run_colon(&mut wm, ":agents topology export run1");
        let msg = wm.active_pane().status_message().unwrap().to_string();
        assert!(msg.starts_with("Topology exported to "), "{msg}");
        let path = rt.join("bilterm").join("agent-topology").join("run1.json");
        let text = std::fs::read_to_string(&path).expect("file written");
        let v = JsonValue::parse(&text).unwrap();
        assert_eq!(v.get("pair_count").and_then(JsonValue::as_i64), Some(1));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(std::fs::metadata(path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        }
        for bad in ["..", "../x", "a/b", ".hidden", "a\\b"] {
            let cmd = format!(":agents topology export {bad}");
            let action = parse_colon_command(&cmd).unwrap().unwrap();
            wm.apply(action).unwrap();
            let m = wm.active_pane().status_message().unwrap().to_string();
            assert!(m.starts_with("Topology export failed"), "{bad}: {m}");
        }
        assert!(!rt.join("bilterm").join("x.json").exists());
        assert!(!rt.join("x.json").exists());
        assert!(parse_colon_command(":agents topology export").unwrap().is_err());
        assert!(parse_colon_command(":agents topology nope").unwrap().is_err());
        let _ = std::fs::remove_dir_all(&rt);
    });
}

#[test]
fn topology_export_refused_for_api_origin() {
    with_temp_xdg(|| {
        let rt = runtime_dir("api");
        let (mut wm, _) = fixture();
        say(&mut wm, Ep::Pane(1), Ep::Pane(2), 1, Instant::now());
        let action = parse_colon_command(":agents topology export viaapi").unwrap().unwrap();
        wm.apply_automation(action, Some(&rt)).unwrap();
        assert!(!rt.join("bilterm").join("agent-topology").join("viaapi.json").exists());
        let m = wm.active_pane().status_message().unwrap().to_string();
        assert!(m.contains("not available over the API"), "{m}");
        // The read path stays open to API callers.
        let action = parse_colon_command(":agents topology").unwrap().unwrap();
        wm.apply_automation(action, Some(&rt)).unwrap();
        assert!(wm.take_command_data().is_some());
        let _ = std::fs::remove_dir_all(&rt);
    });
}
