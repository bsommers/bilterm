//! Phase 57 integration test suite:
//! - `CLIP-01`: Smart `Ctrl+V` paste in `RawPtyPassthrough` / `ALT_SCREEN` (Herdr / Claude Code fix),
//!   `Shift+RightClick` / `Shift+MiddleClick` SGR mouse paste bypass, and inbound `OSC 52` decoding.
//! - `CLIP-02`: Multi-session & multi-window `ClipboardRegistry` with secret redaction.
//! - `CLIP-03`: Floating, pinnable, dockable `CLIPBOARD` utility window rendering & interaction.
//! - `CLIP-04`: `UNI-R01` surface uniformity across Keyboard, Mouse, `:clipboard`/`:clips`,
//!   Command Palette, Top Menu Bar, JSON-RPC `clipboard.*`, and MCP `bilterm_clipboard`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::clipboard::{
    ClipboardMimeProbe, clear_process_clipboard, get_system_clipboard, set_system_clipboard,
    set_test_clipboard_mime_override,
};
use bilterm::colon_command::{execute_wm_colon_command, parse_colon_command};
use bilterm::input::{handle_wm_key_event, handle_wm_mouse_event};
use bilterm::ui::WorkspaceWidget;
use bilterm::ui::modals::{
    PALETTE_CATALOG, menubar_category_item_count, menubar_item_command,
};
use bilterm::wm::clipboard_registry::{ClipboardFilterScope, ClipboardSource};
use bilterm::wm::clipboard_window::ClipboardWindowOp;
use bilterm::wm::split_tree::SplitDirection;
use bilterm::wm::WindowManager;

use std::sync::Mutex;

static CLIP_TEST_MUTEX: Mutex<()> = Mutex::new(());

fn render_to_string(wm: &WindowManager, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(WorkspaceWidget::new(wm), frame.area());
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn clip_01_smart_ctrl_v_in_raw_pty_pastes_text_and_forwards_0x16_for_media() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    clear_process_clipboard();
    set_test_clipboard_mime_override(None);

    let mut wm = WindowManager::new_headless(25, 80);
    // Enter alternate screen (?1049h) + bracketed paste (?2004h) like Herdr / Claude Code TUI.
    wm.active_pane_mut()
        .process_pty_output(b"\x1b[?1049h\x1b[?2004h");
    assert!(wm.active_pane().vt().is_alternate_screen());
    assert!(wm.active_pane().vt().is_bracketed_paste());
    wm.active_pane_mut().write_to_pty(b"").unwrap();

    // Case 1: System clipboard has text -> Ctrl+V must bracket-paste the text, NOT send 0x16.
    set_system_clipboard("herdr pasted text");
    set_test_clipboard_mime_override(Some(ClipboardMimeProbe::Text));
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
    )
    .unwrap();

    let joined = String::from_utf8_lossy(wm.active_pane().last_pty_write()).into_owned();
    assert_eq!(joined, "\x1b[200~herdr pasted text\x1b[201~");
    assert!(!joined.contains('\x16'));

    // Case 2: System clipboard has image/png media -> Ctrl+V forwards raw 0x16 to PTY and records MediaRef.
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    set_test_clipboard_mime_override(Some(ClipboardMimeProbe::Media {
        mime: "image/png".to_string(),
        bytes_len: 16384,
    }));
    handle_wm_key_event(
        &mut wm,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
    )
    .unwrap();

    assert_eq!(wm.active_pane().last_pty_write(), &[0x16]);
    let media_entries = wm.clipboard_registry().filtered_entries(
        ClipboardFilterScope::Media,
        None,
        None,
        "default",
        wm.active_pane_id(),
        "",
    );
    assert_eq!(media_entries.len(), 1);
    assert_eq!(media_entries[0].mime.label(), "image/png");

    // Case 3: `:paste raw-ctrl-v` explicitly sends 0x16 even when clipboard has text.
    wm.active_pane_mut().write_to_pty(b"").unwrap();
    set_test_clipboard_mime_override(Some(ClipboardMimeProbe::Text));
    let cmd = parse_colon_command(":paste raw-ctrl-v").unwrap().unwrap();
    execute_wm_colon_command(&mut wm, cmd);
    assert_eq!(wm.active_pane().last_pty_write(), &[0x16]);

    set_test_clipboard_mime_override(None);
}

#[test]
fn clip_01_shift_mouse_click_bypasses_sgr_mouse_tracking_to_paste_and_osc52_decodes() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    clear_process_clipboard();
    set_test_clipboard_mime_override(None);

    let mut wm = WindowManager::new_headless(25, 80);
    let area = Rect::new(0, 0, 80, 25);
    // Enable alternate screen + SGR mouse tracking (?1000h + ?1006h) + bracketed paste (?2004h).
    wm.active_pane_mut()
        .process_pty_output(b"\x1b[?1049h\x1b[?1000h\x1b[?1006h\x1b[?2004h");
    wm.active_pane_mut().write_to_pty(b"").unwrap();

    set_system_clipboard("shift_right_paste");
    handle_wm_mouse_event(
        &mut wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 10,
            row: 5,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
    )
    .unwrap();
    let joined = String::from_utf8_lossy(wm.active_pane().last_pty_write()).into_owned();
    assert_eq!(joined, "\x1b[200~shift_right_paste\x1b[201~");

    // Inbound OSC 52 from child TUI decodes base64, sets process clipboard, and records Osc52 entry.
    // "OSC52_CLIP_PAYLOAD" in base64 is "T1NDNTJfQ0xJUF9QQVlMT0FE"
    wm.active_pane_mut()
        .process_pty_output(b"\x1b]52;c;T1NDNTJfQ0xJUF9QQVlMT0FE\x07");
    wm.sync_clipboard_records();
    assert_eq!(
        get_system_clipboard().as_deref(),
        Some("OSC52_CLIP_PAYLOAD")
    );
    let entries = wm.clipboard_registry().entries();
    assert!(
        entries
            .iter()
            .any(|e| e.source == ClipboardSource::Osc52 && e.content == "OSC52_CLIP_PAYLOAD")
    );
}

#[test]
fn clip_02_registry_multi_session_multi_window_filters_and_secret_redaction() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let mut wm = WindowManager::new_headless(25, 80);
    wm.config_mut().history_session = "alpha".to_string();
    wm.active_pane_mut().history_store_mut().base_dir = tmp.path().to_path_buf();
    wm.sync_history_config();

    let p1 = wm.active_pane_id();
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":clipboard copy alpha_win1_clip")
            .unwrap()
            .unwrap(),
    );

    let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
    assert_eq!(wm.active_pane_id(), p2);
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":clipboard copy alpha_win2_clip")
            .unwrap()
            .unwrap(),
    );

    // Switch session to "beta"
    wm.config_mut().history_session = "beta".to_string();
    wm.sync_history_config();
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(":clipboard copy beta_win2_clip")
            .unwrap()
            .unwrap(),
    );

    // Record a secret and verify it is marked redacted, masked in snippet/preview, and excluded from disk log.
    let secret = "OPENAI_API_KEY=sk-proj-1234567890abcdef1234567890abcdef";
    execute_wm_colon_command(
        &mut wm,
        parse_colon_command(&format!(":clipboard copy {secret}"))
            .unwrap()
            .unwrap(),
    );
    let head = &wm.clipboard_registry().entries()[0];
    assert!(head.redacted, "secret must be flagged redacted");
    assert!(head.masked_snippet(80).contains("REDACTED SECRET"));
    assert!(!head.masked_snippet(80).contains("sk-proj-"));
    let disk_clips = wm.active_pane().history_store().load_recent_clipboard(20);
    assert!(
        !disk_clips.iter().any(|c| c.contains("sk-proj-")),
        "secret must never be persisted to disk clipboard log"
    );

    // Filter by Session ("alpha" vs "beta")
    let alpha_items = wm.clipboard_registry().filtered_entries(
        ClipboardFilterScope::Session,
        Some("alpha"),
        None,
        "beta",
        p2,
        "",
    );
    assert_eq!(alpha_items.len(), 2);

    // Filter by Window (p1 vs p2)
    let p1_items = wm.clipboard_registry().filtered_entries(
        ClipboardFilterScope::Window,
        None,
        Some(p1),
        "beta",
        p2,
        "",
    );
    assert_eq!(p1_items.len(), 1);
    assert_eq!(p1_items[0].content, "alpha_win1_clip");

    // Pin p1's entry and clear unpinned entries
    let p1_entry_id = p1_items[0].id;
    wm.apply(Action::ClipboardWindow(ClipboardWindowOp::PinEntryId(
        p1_entry_id,
        Some(true),
    )))
    .unwrap();
    let pinned_items = wm.clipboard_registry().filtered_entries(
        ClipboardFilterScope::Pinned,
        None,
        None,
        "beta",
        p2,
        "",
    );
    assert_eq!(pinned_items.len(), 1);
    assert_eq!(pinned_items[0].id, p1_entry_id);

    wm.apply(Action::ClipboardWindow(ClipboardWindowOp::ClearUnpinned))
        .unwrap();
    assert_eq!(wm.clipboard_registry().len(), 1);
    assert_eq!(wm.clipboard_registry().entries()[0].id, p1_entry_id);
}

#[test]
fn clip_03_04_clipboard_window_rendering_and_uni_r01_surface_parity() {
    let _guard = CLIP_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    clear_process_clipboard();
    let mut plane = AutomationControlPlane::new_headless(30, 100);
    let area = Rect::new(0, 0, 100, 30);

    // 1. Seed via JSON-RPC `clipboard.copy`
    let copy_res = plane
        .dispatch_method(
            "clipboard.copy",
            &JsonValue::parse(r#"{"text":"rpc_seeded_clip_line1\nrpc_seeded_clip_line2"}"#)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        copy_res.get("status").and_then(JsonValue::as_str),
        Some("copied")
    );

    // 2. Toggle open via Keyboard shortcut `Ctrl+Alt+V`
    handle_wm_key_event(
        &mut plane.wm,
        KeyEvent::new(
            KeyCode::Char('v'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ),
    )
    .unwrap();
    assert!(plane.wm.clipboard_window().active);
    assert!(plane.wm.clipboard_window().focused);

    // Render and verify window chrome, filter tabs, entry row, preview, and footer chips
    let rendered = render_to_string(&plane.wm, 100, 30);
    assert!(rendered.contains("CLIPBOARD"), "missing CLIPBOARD title");
    assert!(rendered.contains("[All]"), "missing [All] tab");
    assert!(rendered.contains("[Session]"), "missing [Session] tab");
    assert!(rendered.contains("[Window]"), "missing [Window] tab");
    assert!(rendered.contains("[Pinned]"), "missing [Pinned] tab");
    assert!(rendered.contains("[Media]"), "missing [Media] tab");
    assert!(
        rendered.contains("rpc_seeded_clip_line1"),
        "missing entry snippet in rendered CLIPBOARD window"
    );
    assert!(
        rendered.contains("[Enter:Paste]"),
        "missing [Enter:Paste] footer chip"
    );

    // 3. Pin entry via `*` key, and pin window via `Alt+P`; verify Esc blurs instead of closing when window is pinned
    handle_wm_key_event(
        &mut plane.wm,
        KeyEvent::new(KeyCode::Char('*'), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(plane.wm.clipboard_registry().entries()[0].pinned);
    handle_wm_key_event(
        &mut plane.wm,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT),
    )
    .unwrap();
    assert!(plane.wm.clipboard_window().pinned);
    handle_wm_key_event(
        &mut plane.wm,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
    )
    .unwrap();
    assert!(
        plane.wm.clipboard_window().active,
        "pinned window must stay open on Esc"
    );
    assert!(
        !plane.wm.clipboard_window().focused,
        "pinned window must release focus on Esc"
    );

    // 4. Re-focus and dock via `:clips dock`
    let dock_cmd = parse_colon_command(":clips dock").unwrap().unwrap();
    execute_wm_colon_command(&mut plane.wm, dock_cmd);
    assert!(plane.wm.clipboard_window().docked);

    // Undock via `:clipboard float`
    let float_cmd = parse_colon_command(":clipboard float").unwrap().unwrap();
    execute_wm_colon_command(&mut plane.wm, float_cmd);
    assert!(!plane.wm.clipboard_window().docked);

    // 5. Mouse hit-testing on ScopeTab and Close badge
    let rect = plane.wm.clipboard_window_rect(area).unwrap();
    // Click `[Pinned]` tab on row `rect.y + 1`
    handle_wm_mouse_event(
        &mut plane.wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x + 1 + 27,
            row: rect.y + 1,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert_eq!(
        plane.wm.clipboard_window().scope,
        ClipboardFilterScope::Pinned
    );

    // Click `[×]` Close badge on top border
    let close_x = rect.x + rect.width.saturating_sub(4);
    handle_wm_mouse_event(
        &mut plane.wm,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: close_x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
    .unwrap();
    assert!(!plane.wm.clipboard_window().active);

    // 6. Toggle open via `Leader ;` (`Ctrl+X ;`)
    handle_wm_key_event(
        &mut plane.wm,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    )
    .unwrap();
    handle_wm_key_event(
        &mut plane.wm,
        KeyEvent::new(KeyCode::Char(';'), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(plane.wm.clipboard_window().active);

    // 7. Verify PALETTE_CATALOG and top menu bar ("View" -> category index 3) entries
    assert!(
        PALETTE_CATALOG
            .iter()
            .any(|(cmd, _, _)| *cmd == ":clipboard toggle")
    );
    let view_cat_count = menubar_category_item_count(3);
    let has_menubar_clip = (0..view_cat_count)
        .filter_map(|i| menubar_item_command(3, i))
        .any(|cmd| cmd == ":clipboard toggle");
    assert!(has_menubar_clip, "View menu missing :clipboard toggle");

    // 8. Verify MCP `bilterm_clipboard` tool call
    let mcp_resp = plane
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":77,"method":"tools/call","params":{"name":"bilterm_clipboard","arguments":{"action":"list","scope":"all"}}}"#,
        )
        .unwrap();
    assert!(mcp_resp.contains("\"isError\":false"), "{mcp_resp}");
    assert!(mcp_resp.contains("rpc_seeded_clip_line1"), "{mcp_resp}");
}
