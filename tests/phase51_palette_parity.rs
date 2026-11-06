//! Phase 51-04: palette parity (UIT-02). The probe sweep itself lives in
//! `src/command_registry.rs` (it needs crate-private tables); this file holds the behaviour
//! checks of the entries it forced (spot execution, key and mouse paths, filtering, help rows)
//! and the automation view of the exclusions.

use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::handle_wm_key_event;
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::ui::modals::{HELP_LINES, PALETTE_CATALOG};
use bilterm::wm::drawer::DrawerStyle;
use bilterm::wm::{Modal, WindowManager};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

const AREA: Rect = Rect { x: 0, y: 0, width: 120, height: 40 };

fn call(plane: &mut AutomationControlPlane, line: &str) -> JsonValue {
    JsonValue::parse(&plane.handle_mcp_line(line).expect("response")).expect("json")
}

fn commands_resource() -> JsonValue {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let resp = call(
        &mut plane,
        r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"bilterm://commands"}}"#,
    );
    let text = resp
        .get("result")
        .and_then(|r| r.get("contents"))
        .and_then(JsonValue::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("text"))
        .and_then(JsonValue::as_str)
        .expect("commands resource text")
        .to_string();
    JsonValue::parse(&text).expect("commands body json")
}

#[test]
fn commands_resource_carries_exclusions() {
    let body = commands_resource();
    let cmds = body.get("commands").and_then(JsonValue::as_array).expect("commands array");
    for c in cmds {
        assert!(
            c.get("palette_exclusions").and_then(JsonValue::as_array).is_some(),
            "{} has no palette_exclusions array",
            c.get("name").and_then(JsonValue::as_str).unwrap_or("?")
        );
    }
    let ssh = cmds
        .iter()
        .find(|c| c.get("name").and_then(JsonValue::as_str) == Some(":ssh"))
        .expect(":ssh in the catalog");
    let ex = ssh.get("palette_exclusions").and_then(JsonValue::as_array).unwrap();
    assert!(
        ex.iter().any(|e| e.get("reason").and_then(JsonValue::as_str) == Some("host-or-path")
            && e.get("probe").and_then(JsonValue::as_str).is_some_and(|p| p.starts_with(":ssh"))),
        ":ssh lists a host-or-path exclusion: {}",
        ssh.to_json_string()
    );
}

fn run(wm: &mut WindowManager, cmd: &str) {
    let action = parse_colon_command(cmd)
        .unwrap_or_else(|| panic!("not a colon command: {cmd}"))
        .unwrap_or_else(|e| panic!("parse error for {cmd}: {e}"));
    execute_wm_colon_command(wm, action);
}

/// The entries added by this plan are the tail of the table after this marker row.
fn new_entries() -> &'static [(&'static str, &'static str, &'static str)] {
    let i = PALETTE_CATALOG.iter().position(|e| e.0 == ":appearance save").expect("marker entry");
    &PALETTE_CATALOG[i + 1..]
}

#[test]
fn new_palette_entries_execute() {
    // host side effects (files, clipboard, recording) are covered by the parse sweep only
    let skip = [":history save", ":record pane", ":yank", ":copy yank", ":rect yank"];
    let entries = new_entries();
    assert!(entries.len() >= 100, "expected the ~110-130 new entries, found {}", entries.len());
    for &(cmd, _, _) in entries {
        if skip.contains(&cmd) {
            continue;
        }
        let mut wm = WindowManager::new_headless(24, 80);
        run(&mut wm, cmd); // parses and executes without panic
    }

    // checked state changes
    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":drawer style classic");
    assert_eq!(wm.drawer_config().style, DrawerStyle::Classic);
    run(&mut wm, ":drawer style navigator");
    assert_eq!(wm.drawer_config().style, DrawerStyle::Navigator);
    run(&mut wm, ":drawer rows 1");
    assert_eq!(wm.drawer_config().row_lines, 1);
    run(&mut wm, ":drawer rows 2");
    assert_eq!(wm.drawer_config().row_lines, 2);

    run(&mut wm, ":agents list open");
    assert!(wm.agents_list_summary().active);
    run(&mut wm, ":agents list filter agents");
    run(&mut wm, ":agents list pin");
    run(&mut wm, ":agents list close");

    let panes = wm.pane_count();
    run(&mut wm, ":agents demo");
    // demo bookkeeping and the pane-count restore belong to plan 51-03 (tests/phase51_demo_stop.rs)
    run(&mut wm, ":agents demo stop");
    assert!(wm.pane_count() <= panes.max(3), "demo stop never adds panes");

    let mut wm = WindowManager::new_headless(24, 80);
    run(&mut wm, ":arrange columns");
}

fn open_palette_by_key(wm: &mut WindowManager) {
    handle_wm_key_event(wm, KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)).unwrap();
    assert!(wm.is_palette_visible(), "Ctrl+P opens the palette");
}

#[test]
fn palette_entry_by_key_and_mouse() {
    // key path: Ctrl+P, type, Enter
    let mut by_key = WindowManager::new_headless(40, 120);
    by_key.config_mut().drawer.style = DrawerStyle::Classic;
    open_palette_by_key(&mut by_key);
    for c in "drawer style navig".chars() {
        handle_wm_key_event(&mut by_key, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)).unwrap();
    }
    assert_eq!(by_key.filtered_palette_items().first().map(|e| e.0), Some(":drawer style navigator"));
    handle_wm_key_event(&mut by_key, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    assert!(matches!(by_key.modal(), Modal::None));
    assert_eq!(by_key.drawer_config().style, DrawerStyle::Navigator);

    // mouse path: click the same entry in the open palette
    let mut by_mouse = WindowManager::new_headless(40, 120);
    by_mouse.config_mut().drawer.style = DrawerStyle::Classic;
    by_mouse.open_palette_with_query("drawer style navig");
    let idx = by_mouse
        .filtered_palette_items()
        .iter()
        .position(|e| e.0 == ":drawer style navigator")
        .expect("entry listed");
    let outer = by_mouse.active_modal_rect(AREA).expect("palette rect");
    let inner = bilterm::wm::inset_window_frame(outer);
    let hdr = bilterm::ui::modals::palette_header_rows(by_mouse.config(), inner.height);
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: inner.x + 4,
        row: inner.y + hdr + idx as u16,
        modifiers: KeyModifiers::NONE,
    };
    handle_wm_mouse_event(&mut by_mouse, click, AREA).unwrap();
    assert!(matches!(by_mouse.modal(), Modal::None));
    assert_eq!(by_mouse.drawer_config().style, DrawerStyle::Navigator);
    assert_eq!(by_key.drawer_config().style, by_mouse.drawer_config().style, "key and mouse reach the same state");
}

#[test]
fn palette_filter_finds_new_entries_by_plain_words() {
    for (query, want) in [
        ("navigator style", ":drawer style navigator"),
        ("stop demo", ":agents demo stop"),
        ("agents list pin", ":agents list pin"),
        ("arrange grid", ":arrange grid"),
    ] {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.open_palette_with_query(query);
        let top: Vec<_> = wm.filtered_palette_items().iter().take(3).map(|e| e.0).collect();
        assert!(top.contains(&want), "`{query}` should list {want} in the first 3, got {top:?}");
    }
}

#[test]
fn palette_enter_fill_in_entries_unchanged() {
    // Outcomes recorded at the unmodified HEAD (D-02: the prefill UX change is a follow-up that
    // needs sign-off): Enter runs the bare verb and a parse error is silently dropped.
    for (text, want_status) in [
        (":ssh ", Some("Fleet nodes: (local only) \u{2014} Connect via :ssh <user@host> [port]")),
        (":rename ", None),
    ] {
        let mut wm = WindowManager::new_headless(24, 80);
        wm.open_palette_with_query("");
        let idx = wm.filtered_palette_items().iter().position(|e| e.0 == text).expect("fill-in entry");
        if let Modal::Palette { selected, .. } = wm.modal_mut() {
            *selected = idx;
        }
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
        assert!(matches!(wm.modal(), Modal::None), "the palette closes on Enter for {text:?}");
        assert_eq!(wm.pane_count(), 1, "no pane change for {text:?}");
        assert_eq!(wm.active_pane().status_message(), want_status, "status after Enter on {text:?}");
    }
}

#[test]
fn help_has_rows_for_new_verbs() {
    let all: String = HELP_LINES.iter().map(|(l, r)| format!("{l} {r}\n")).collect();
    for needle in [
        ":agents demo stop",
        ":agents list open",
        ":agents list dismiss",
        ":agents list select",
        ":agents list jump",
        ":agents list zoom",
        ":agents list pin",
        ":agents list scale",
        ":agents list filter",
        ":agents list dock",
        ":agents list move",
        ":agents list resize",
        ":agents list rect",
        ":agents list size",
        ":agents list scroll",
        ":drawer style",
    ] {
        assert!(HELP_LINES.iter().any(|(l, _)| l.contains(needle)), "HELP_LINES lacks a row for `{needle}`");
    }
    assert!(HELP_LINES.iter().any(|(l, _)| *l == "Agents list keys"));
    assert!(HELP_LINES.iter().any(|(l, _)| *l == "Agents list mouse"));
    let demo = HELP_LINES.iter().find(|(l, _)| *l == ":agents demo").expect(":agents demo row");
    assert!(demo.1.contains("stop"), "the :agents demo row mentions stop");
    assert!(all.contains(":arrange"));
}
