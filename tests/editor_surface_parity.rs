//! `EDIT-01` / `UNI-R01`: the built-in editor and scratchpad are ordinary windows reachable from
//! every surface: colon (`:edit`, `:scratch`, `:editor ...`), palette, key chord, `editor.*` RPC,
//! `bilterm_editor` MCP, `bilterm editor ...` CLI; all lower to `Action::Editor`. Also covers the
//! window lifecycle (tile, close guard, session persistence) and file safety. Isolated state dirs.
#![cfg(unix)]

use bilterm::action::{Action, EditorOp};
use bilterm::api::{AutomationControlPlane, JsonValue, RPC_METHODS};
use bilterm::colon_command::parse_colon_command;
use bilterm::input::{AppAction, handle_wm_key_event};
use bilterm::ui::modals::PALETTE_CATALOG;
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn sandbox() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let d = tempfile::tempdir().unwrap();
    // SAFETY: serialized by ENV_LOCK; every test calls sandbox() first.
    unsafe {
        std::env::set_var("XDG_STATE_HOME", d.path().join("state"));
        std::env::set_var("XDG_DATA_HOME", d.path().join("data"));
        std::env::set_var("XDG_CONFIG_HOME", d.path().join("config"));
        std::env::set_var("HOME", d.path());
    }
    // API file verbs are confined to the editor's working directory (`editor_path_denied`).
    std::env::set_current_dir(d.path()).unwrap();
    (g, d)
}

fn k(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}
fn press(wm: &mut WindowManager, ev: KeyEvent) {
    let _ = handle_wm_key_event(wm, ev).unwrap();
}
fn type_str(wm: &mut WindowManager, s: &str) {
    for c in s.chars() {
        press(wm, if c == '\n' { k(KeyCode::Enter) } else { k(KeyCode::Char(c)) });
    }
}
fn colon(wm: &mut WindowManager, c: &str) {
    wm.apply(parse_colon_command(c).unwrap().unwrap()).unwrap();
}
fn screen(wm: &WindowManager) -> String {
    wm.active_pane().vt().contents()
}
fn json(s: &str) -> JsonValue {
    JsonValue::parse(s).unwrap()
}

const VERBS: &[(&str, &str, &str)] = &[
    // (rpc verb, colon form, palette head)
    ("open", ":edit /tmp/x.md", ":edit"),
    ("scratch", ":scratch", ":scratch"),
    ("artifact", ":artifact plan", ":artifact"),
    ("board", ":board plan", ":board"),
    ("save", ":editor save", ":editor save"),
    ("save_as", ":editor saveas /tmp/x.md", ":editor saveas <path>"),
    ("wrap", ":editor wrap on", ":editor wrap"),
    ("render", ":editor render on", ":editor render"),
    ("check", ":editor check", ":editor check"),
    ("broadcast", ":editor broadcast ready", ":editor broadcast [note]"),
    ("preview", ":editor preview", ":editor preview"),
    ("find", ":editor find abc", ":editor find <text>"),
    ("undo", ":editor undo", ":editor undo"),
    ("redo", ":editor redo", ":editor redo"),
    ("goto", ":editor goto 3", ":editor goto <line>"),
    ("revert", ":editor revert", ":editor revert"),
    ("insert", ":editor insert hi", ":editor insert <text>"),
    ("text", ":editor text", ":editor text"),
    ("status", ":editor status", ":editor status"),
    ("close", ":editor close", ":editor close"),
    ("menu", ":editor menu open file", ":editor menu"),
];

#[test]
fn every_editor_verb_has_colon_palette_rpc_mcp_and_cli() {
    let mcp_src = include_str!("../src/api/mcp.rs");
    assert!(mcp_src.contains("\"bilterm_editor\" => Some(Self::editor_method"));
    for (verb, c, palette) in VERBS {
        let a = parse_colon_command(c).unwrap().unwrap_or_else(|e| panic!("{c}: {e}"));
        assert!(matches!(a, Action::Editor(_)), "{c}");
        assert!(PALETTE_CATALOG.iter().any(|e| e.0 == *palette), "palette entry {palette}");
        assert!(RPC_METHODS.contains(&format!("editor.{verb}").as_str()), "RPC editor.{verb}");
    }
    // CLI: every verb lowers to the matching RPC method.
    let env = bilterm::cli::CliEnv::default();
    for (args, method) in [
        (vec!["editor", "open", "/tmp/a.md"], "editor.open"),
        (vec!["editor", "scratch"], "editor.scratch"),
        (vec!["editor", "artifact", "plan"], "editor.artifact"),
        (vec!["editor", "board", "plan"], "editor.board"),
        (vec!["editor", "save", "--force"], "editor.save"),
        (vec!["editor", "saveas", "/tmp/b.md"], "editor.save_as"),
        (vec!["editor", "wrap", "off"], "editor.wrap"),
        (vec!["editor", "render", "on"], "editor.render"),
        (vec!["editor", "check"], "editor.check"),
        (vec!["editor", "broadcast", "ready"], "editor.broadcast"),
        (vec!["editor", "preview"], "editor.preview"),
        (vec!["editor", "find", "x"], "editor.find"),
        (vec!["editor", "findprev", "x"], "editor.find"),
        (vec!["editor", "goto", "4"], "editor.goto"),
        (vec!["editor", "insert", "hi", "there"], "editor.insert"),
        (vec!["editor", "text"], "editor.text"),
        (vec!["editor", "status"], "editor.status"),
        (vec!["editor", "close"], "editor.close"),
        (vec!["editor", "menu"], "editor.menu"),
        (vec!["editor", "menu", "do", "cut"], "editor.menu"),
    ] {
        let a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let call = bilterm::cli::parse(&a, &env).unwrap_or_else(|e| panic!("{args:?}: {e}"));
        assert_eq!(call.method, method, "{args:?}");
    }
    assert!(bilterm::cli::parse(&["editor".into(), "goto".into(), "x".into()], &env).is_err());
}

#[test]
fn key_chord_ctrl_alt_e_opens_the_scratchpad_like_the_colon_command() {
    let (_g, _d) = sandbox();
    let mut wm = WindowManager::new_headless(30, 100);
    let before = wm.pane_count();
    press(&mut wm, KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(wm.pane_count(), before + 1);
    assert!(wm.active_pane().editor().unwrap().is_scratch());
    let mut wm2 = WindowManager::new_headless(30, 100);
    colon(&mut wm2, ":scratch");
    assert_eq!(wm2.active_pane().editor().unwrap().status_json(), wm.active_pane().editor().unwrap().status_json());
    // Idempotent: a second chord focuses the existing scratchpad.
    press(&mut wm, KeyEvent::new(KeyCode::Char('E'), KeyModifiers::CONTROL | KeyModifiers::ALT));
    assert_eq!(wm.pane_count(), before + 1);
}

#[test]
fn scratch_window_is_a_normal_window_typing_persists_and_restores() {
    let (_g, _d) = sandbox();
    let mut wm = WindowManager::new_headless(30, 100);
    colon(&mut wm, ":scratch");
    let id = wm.active_pane_id();
    assert!(!wm.active_pane().is_prompt_visible(), "prompt hidden while the editor has the keyboard");
    type_str(&mut wm, "# Notes\nbuy milk");
    assert!(screen(&wm).contains("buy milk"), "{}", screen(&wm));
    assert!(wm.active_pane().title().starts_with("EDIT scratch"));
    // Autosave runs on the tick; closing flushes immediately.
    wm.editor_tick();
    let file = wm.active_pane().editor().unwrap().status_json();
    assert!(file.contains("\"kind\":\"scratch\""));
    // Session round trip: the saved pane restores as an editor window with the same text.
    let saved = wm.snapshot_session("edit-test");
    let ed = saved.panes.iter().find(|p| p.id == id).unwrap().editor.clone().unwrap();
    assert!(matches!(ed, bilterm::wm::SavedEditor::Scratch { .. }));
    let toml = toml::to_string_pretty(&saved).unwrap();
    assert!(toml.contains("kind = \"scratch\""), "{toml}");
    let back: bilterm::wm::SavedWorkspaceSession = toml::from_str(&toml).unwrap();
    // "Restart": a fresh window manager restores the snapshot; scratch text comes from disk.
    wm.editor_tick();
    let mut fresh = WindowManager::new_headless(30, 100);
    fresh.apply_saved_session_snapshot(&back);
    let restored = fresh.pane(id).unwrap();
    assert_eq!(restored.editor().unwrap().buf.text(), "# Notes\nbuy milk");
    assert!(restored.vt().contents().contains("buy milk"));
}

#[test]
fn edit_file_tiles_saves_guards_close_and_refuses_bad_targets() {
    let (_g, d) = sandbox();
    let f = d.path().join("doc.md");
    std::fs::write(&f, "# Doc\n\nbody\n").unwrap();
    let mut wm = WindowManager::new_headless(30, 100);
    let before = wm.pane_count();
    colon(&mut wm, &format!(":edit {}", f.display()));
    assert_eq!(wm.pane_count(), before + 1, "tiles like any other window");
    assert!(screen(&wm).contains("# Doc"));
    // Re-opening the same file focuses the existing window instead of opening a second one.
    colon(&mut wm, &format!(":edit {}", f.display()));
    assert_eq!(wm.pane_count(), before + 1);
    // Edit, dirty indicator, save.
    press(&mut wm, KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
    type_str(&mut wm, "tail");
    assert!(wm.active_pane().title().ends_with('*'), "{}", wm.active_pane().title());
    // Closing with unsaved changes is refused once, then honoured.
    colon(&mut wm, ":close");
    assert_eq!(wm.pane_count(), before + 1);
    assert!(wm.active_pane().status_message().unwrap().contains("unsaved"));
    press(&mut wm, KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "# Doc\n\nbodytail\n");
    assert!(!wm.active_pane().title().ends_with('*'));
    // Ctrl+] hands the keyboard to the colon prompt and back.
    press(&mut wm, KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL));
    assert!(wm.active_pane().is_prompt_visible());
    press(&mut wm, KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL));
    assert!(!wm.active_pane().is_prompt_visible());
    colon(&mut wm, ":close");
    assert_eq!(wm.pane_count(), before);
    // Directories, binary files and special files are refused without leaving a window.
    colon(&mut wm, &format!(":edit {}", d.path().display()));
    assert_eq!(wm.pane_count(), before);
    assert!(wm.active_pane().status_message().unwrap().contains("not a regular file"));
    let bin = d.path().join("x.bin");
    std::fs::write(&bin, b"\0\x01\x02").unwrap();
    colon(&mut wm, &format!(":edit {}", bin.display()));
    assert_eq!(wm.pane_count(), before);
    assert!(wm.active_pane().status_message().unwrap().contains("binary"));
}

#[test]
fn session_round_trip_reopens_file_buffers_and_scratch_in_the_same_layout() {
    let (_g, d) = sandbox();
    let f = d.path().join("keep.txt");
    std::fs::write(&f, "persisted\n").unwrap();
    let mut wm = WindowManager::new_headless(30, 100);
    colon(&mut wm, &format!(":edit {}", f.display()));
    let id = wm.active_pane_id();
    colon(&mut wm, ":editor wrap off");
    let saved = wm.snapshot_session("edit-files");
    let mut fresh = WindowManager::new_headless(30, 100);
    fresh.apply_saved_session_snapshot(&saved);
    let ed = fresh.pane(id).unwrap().editor().unwrap();
    assert_eq!(ed.path.as_deref(), Some(f.as_path()));
    assert!(!ed.wrap, "wrap option persisted");
    assert_eq!(fresh.pane_count(), wm.pane_count());
}

#[test]
fn rpc_and_mcp_lower_to_the_same_action_and_are_gated() {
    let (_g, d) = sandbox();
    let f = d.path().join("api.md");
    let mut cp = AutomationControlPlane::new_headless(30, 100);
    let none = json("{}");
    assert!(cp.dispatch_method("editor.status", &none).unwrap_err().contains("no editor window"));
    // Scratch and in-buffer verbs are allowed; the scratchpad is not a host file.
    let r = cp.dispatch_method("editor.scratch", &none).unwrap();
    assert!(r.get("pane").is_some());
    cp.dispatch_method("editor.insert", &json(r#"{"text":"hello api"}"#)).unwrap();
    let t = cp.dispatch_method("editor.text", &none).unwrap();
    assert_eq!(t.get("text").and_then(JsonValue::as_str), Some("hello api"));
    let st = cp.dispatch_method("editor.status", &none).unwrap();
    assert_eq!(st.get("editor").and_then(|e| e.get("kind")).and_then(JsonValue::as_str), Some("scratch"));
    // Host file access is refused until [api] allow_config_write.
    let open = json(&format!(r#"{{"path":"{}"}}"#, f.display()));
    let e = cp.dispatch_method("editor.open", &open).unwrap_err();
    assert!(e.contains("allow_config_write"), "{e}");
    let e = cp.dispatch_method("editor.save_as", &open).unwrap_err();
    assert!(e.contains("allow_config_write"), "{e}");
    assert!(!f.exists());
    cp.wm.config_mut().api.allow_config_write = true;
    cp.dispatch_method("editor.save_as", &open).unwrap();
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello api\n");
    // Overwrite needs force.
    cp.dispatch_method("editor.scratch", &json(r#"{"name":"two"}"#)).unwrap();
    cp.dispatch_method("editor.insert", &json(r#"{"text":"other"}"#)).unwrap();
    let e = cp.dispatch_method("editor.save_as", &open).unwrap_err();
    assert!(e.contains("exists"), "{e}");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello api\n");
    cp.dispatch_method("editor.save_as", &json(&format!(r#"{{"path":"{}","force":true}}"#, f.display()))).unwrap();
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "other\n");
    // MCP routes to the same handler.
    let out = cp
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"bilterm_editor","arguments":{"action":"status"}}}"#)
        .unwrap();
    assert!(out.contains("kind") && !out.contains("\"isError\":true"), "{out}");
    let out = cp
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bilterm_editor","arguments":{"action":"bogus"}}}"#)
        .unwrap();
    assert!(out.contains("missing or invalid 'action'") || out.contains("isError"), "{out}");
    // The colon surface over `wm.command` reaches the same state.
    let r = cp.dispatch_method("wm.command", &json(r#"{"command":":editor text"}"#));
    assert!(r.is_ok());
}

#[test]
fn editor_op_actions_apply_without_panicking_for_every_verb_with_no_editor() {
    let (_g, _d) = sandbox();
    let mut wm = WindowManager::new_headless(30, 100);
    for op in [EditorOp::Save { force: false }, EditorOp::Undo, EditorOp::Text, EditorOp::Status, EditorOp::Goto(3), EditorOp::Close { force: true }] {
        assert!(matches!(wm.apply(Action::Editor(op)).unwrap(), AppAction::Continue | AppAction::Handled));
        assert!(wm.take_command_data().unwrap().contains("error"));
    }
}

#[test]
fn shared_artifact_syncs_bidirectionally_with_context_bus() {
    let (_g, _d) = sandbox();
    let mut cp = AutomationControlPlane::new_headless(30, 100);
    cp.wm.config_mut().api.allow_config_write = true; // `editor.artifact` is gated for API callers
    // Open a shared artifact via RPC; it seeds ContextBus under artifact:plan.
    let opened = cp.dispatch_method("editor.artifact", &json(r#"{"name":"plan"}"#)).unwrap();
    assert_eq!(opened.get("bus_key").and_then(JsonValue::as_str), Some("artifact:plan"));
    let seeded = cp.wm.context_bus().get("artifact:plan").expect("ContextBus seeded");
    assert!(seeded.value.contains("Shared Artifact: plan"), "{}", seeded.value);

    // External agent writes to ContextBus -> editor_tick() live-updates the open artifact pane
    // and marks autosave pending so synced bus content persists to disk.
    cp.dispatch_method(
        "context.set",
        &json(r##"{"key":"artifact:plan","value":"# Plan\n- [ ] Step A\n- [x] Step B"}"##),
    )
    .unwrap();
    assert!(cp.wm.editor_tick(), "editor_tick should report repaint after external ContextBus write");
    assert!(
        cp.wm.active_pane().editor().unwrap().autosave_pending(),
        "bus-synced artifact update must arm autosave"
    );
    let t = cp.dispatch_method("editor.text", &json("{}")).unwrap();
    assert_eq!(t.get("text").and_then(JsonValue::as_str), Some("# Plan\n- [ ] Step A\n- [x] Step B"));

    // Refocusing an already-open artifact still returns bus_key.
    let refocused = cp.dispatch_method("editor.artifact", &json(r#"{"name":"plan"}"#)).unwrap();
    assert_eq!(refocused.get("bus_key").and_then(JsonValue::as_str), Some("artifact:plan"));

    // Interactive checklist toggle updates buffer + status progress + pushes back to ContextBus.
    cp.dispatch_method("editor.goto", &json(r#"{"line":2}"#)).unwrap();
    cp.dispatch_method("editor.check", &json("{}")).unwrap();
    let st = cp.dispatch_method("editor.status", &json("{}")).unwrap();
    let ed = st.get("editor").unwrap();
    assert_eq!(ed.get("tasks_done").and_then(JsonValue::as_usize), Some(2));
    assert_eq!(ed.get("tasks_total").and_then(JsonValue::as_usize), Some(2));
    let bus_val = cp.wm.context_bus().get("artifact:plan").unwrap().value;
    assert!(bus_val.contains("- [x] Step A"), "{bus_val}");

    // Undo restores the unchecked state cleanly.
    cp.dispatch_method("editor.undo", &json("{}")).unwrap();
    let bus_val = cp.wm.context_bus().get("artifact:plan").unwrap().value;
    assert!(bus_val.contains("- [ ] Step A"), "{bus_val}");
}

#[test]
fn rendered_view_and_checkbox_interaction_work_end_to_end() {
    let (_g, _d) = sandbox();
    let mut wm = WindowManager::new_headless(30, 100);
    colon(&mut wm, ":artifact tasks");
    colon(&mut wm, ":editor render on");
    assert!(wm.active_pane().editor().unwrap().render_mode);
    let view = screen(&wm);
    assert!(view.contains("☐") && view.contains("VIEW"), "{view}");

    // Navigate to first checklist item (line 7 in template) and press Space to check it.
    colon(&mut wm, ":editor goto 7");
    press(&mut wm, k(KeyCode::Char(' ')));
    let view_after = screen(&wm);
    assert!(view_after.contains("☑"), "{view_after}");

    // Pressing 'e' drops right back to Edit mode.
    press(&mut wm, k(KeyCode::Char('e')));
    assert!(!wm.active_pane().editor().unwrap().render_mode);

    // Checkboxes inside fenced code blocks must not toggle, and ESC sequences must be stripped from rendered rows.
    let mut tb = bilterm::editor::buffer::TextBuffer::from_text("```md\n- [ ] fenced\n```\n\x1b]52;c;evil\x07ok");
    assert_eq!(bilterm::editor::artifact::toggle_checkbox_on_line(&mut tb, 1), None);
    let rows = bilterm::editor::artifact::render_markdown_rows(&tb, 40);
    assert!(rows.iter().all(|r| !r.ansi.contains("\x1b]52")));
}

#[test]
fn file_editor_auto_reloads_when_clean_and_flags_when_dirty() {
    let (_g, d) = sandbox();
    let path = d.path().join("CHECKLIST.md");
    std::fs::write(&path, "# List\n- [ ] first\n").unwrap();
    let mut wm = WindowManager::new_headless(30, 100);
    colon(&mut wm, &format!(":edit {}", path.display()));
    assert_eq!(wm.active_pane().editor().unwrap().buf.text(), "# List\n- [ ] first");

    // Agent modifies file on disk while buffer is clean -> auto-reloads cleanly.
    std::fs::write(&path, "# List\n- [x] first\n- [ ] second\n").unwrap();
    let changed = wm.active_pane_mut().editor_mut().unwrap().poll_external_file_change(true);
    assert!(changed);
    assert_eq!(wm.active_pane().editor().unwrap().buf.text(), "# List\n- [x] first\n- [ ] second");
    assert!(!wm.active_pane().editor().unwrap().external_change);

    // User edits buffer (now dirty); agent modifies disk again -> flagged, NOT clobbered.
    type_str(&mut wm, "LOCAL ");
    assert!(wm.active_pane().editor().unwrap().is_dirty());
    std::fs::write(&path, "# List\n- [x] overwritten externally\n").unwrap();
    let flagged = wm.active_pane_mut().editor_mut().unwrap().poll_external_file_change(true);
    assert!(flagged);
    let ed = wm.active_pane().editor().unwrap();
    assert!(ed.external_change);
    assert!(ed.buf.text().starts_with("LOCAL "), "dirty user edits must not be clobbered");
}

#[test]
fn editor_broadcast_notifies_named_agents_and_refuses_file_buffers() {
    let (_g, d) = sandbox();
    let mut cp = AutomationControlPlane::new_headless(30, 100);
    cp.wm.config_mut().api.allow_config_write = true;
    cp.wm.set_agent_messaging_enabled(true);
    let worker_id = cp.wm.active_pane_id();
    cp.dispatch_method("agent.name", &json(&format!(r#"{{"pane_id":{worker_id},"name":"worker-1"}}"#))).unwrap();
    cp.dispatch_method("editor.artifact", &json(r#"{"name":"spec"}"#)).unwrap();
    let b = cp.dispatch_method("editor.broadcast", &json(r#"{"note":"updated acceptance criteria"}"#)).unwrap();
    assert_eq!(b.get("notified_agents").and_then(JsonValue::as_bool), Some(true));
    let inbox = cp.dispatch_method("agent.inbox", &json(r#"{"agent":"worker-1"}"#)).unwrap().to_json_string();
    assert!(inbox.contains("updated acceptance criteria") && inbox.contains("bilterm://context/artifact:spec"), "{inbox}");

    // Broadcasting a host file buffer is refused: only shared artifacts are published.
    let md_path = d.path().join("README.md");
    std::fs::write(&md_path, "# Hi").unwrap();
    cp.dispatch_method("editor.open", &json(&format!(r#"{{"path":"{}"}}"#, md_path.display()))).unwrap();
    let e = cp.dispatch_method("editor.broadcast", &json("{}")).unwrap_err();
    assert!(e.contains("shared artifact"), "{e}");
    assert!(cp.wm.context_bus().list(Some("artifact:")).iter().all(|e| e.key == "artifact:spec"));
}

#[test]
fn editor_board_and_preview_redact_tokens_and_studio_html_has_csp() {
    let (_g, d) = sandbox();
    // SAFETY: serialized by sandbox() mutex across tests in this binary.
    unsafe { std::env::set_var("BILTERM_BROWSER_SYNTHETIC", "1") };
    let mut cp = AutomationControlPlane::new_headless(30, 100);
    // API callers need allow_config_write for board and preview (touches_files).
    assert!(cp.dispatch_method("editor.board", &json(r#"{"name":"plan"}"#)).is_err());
    assert!(cp.dispatch_method("editor.preview", &json("{}")).is_err());
    cp.wm.config_mut().api.allow_config_write = true;

    // `:board` reply url and message must be redacted (never expose the 64-char secret token).
    let board = cp.dispatch_method("editor.board", &json(r#"{"name":"plan"}"#)).unwrap();
    let board_url = board.get("url").and_then(JsonValue::as_str).unwrap();
    let board_msg = board.get("message").and_then(JsonValue::as_str).unwrap();
    assert!(board_url.ends_with(".../"), "expected redacted board url: {board_url}");
    assert!(board_msg.contains(".../"), "expected redacted board message: {board_msg}");
    let board_seg = board_url.trim_end_matches('/').rsplit('/').next().unwrap();
    assert!(board_seg.len() <= 10, "token must not appear in board url: {board_url}");

    // `:editor preview` reply url and message must likewise be redacted.
    cp.dispatch_method("editor.artifact", &json(r#"{"name":"spec"}"#)).unwrap();
    let prev = cp.dispatch_method("editor.preview", &json("{}")).unwrap();
    let prev_url = prev.get("url").and_then(JsonValue::as_str).unwrap();
    let prev_msg = prev.get("message").and_then(JsonValue::as_str).unwrap();
    assert!(prev_url.ends_with(".../"), "expected redacted preview url: {prev_url}");
    assert!(prev_msg.contains(".../"), "expected redacted preview message: {prev_msg}");
    let prev_seg = prev_url.trim_end_matches('/').rsplit('/').next().unwrap();
    assert!(prev_seg.len() <= 10, "token must not appear in preview url: {prev_url}");

    // Studio HTML escapes `</script>` breakouts and injects `connect-src 'none'` into iframe srcdoc.
    let html = bilterm::editor::studio_web::studio_app_html("/tok__INIT__", "a</script><script>alert(1)");
    assert!(!html.contains("a</script><script>"), "unencoded </script> breakout in studio HTML");
    assert!(html.contains("\\u003c/script\\u003e"), "expected unicode-escaped script tag");
    assert!(html.contains("connect-src 'none'"), "Studio iframe srcdoc must enforce connect-src 'none'");

    // Same-basename files in different directories get distinct, validate_safe_name-compliant studio keys.
    let dir_a = d.path().join("a");
    let dir_b = d.path().join("b");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();
    let file_a = dir_a.join("README.md");
    let file_b = dir_b.join("README.md");
    std::fs::write(&file_a, "# A").unwrap();
    std::fs::write(&file_b, "# B").unwrap();
    let key_a = bilterm::editor::EditorState::open_file(&file_a).unwrap().studio_artifact_name();
    let key_b = bilterm::editor::EditorState::open_file(&file_b).unwrap().studio_artifact_name();
    assert_ne!(key_a, key_b, "same-basename files must not collide: {key_a} vs {key_b}");
    assert!(key_a.starts_with("file-readme-md-"), "{key_a}");
    assert!(bilterm::secure_fs::validate_safe_name(&key_a).is_ok());
    assert!(bilterm::secure_fs::validate_safe_name(&key_b).is_ok());
    assert!(bilterm::editor::preview::is_raw_html_path(Some(std::path::Path::new("demo.html"))));
    assert!(bilterm::editor::preview::is_raw_html_path(Some(std::path::Path::new("chart.svg"))));
}
