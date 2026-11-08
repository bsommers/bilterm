//! Surface Uniformity and Prompt Staging Test Suite for STT (`STT-01..03`, `UNI-R01`, `ARCH-R01..02`).

use bilterm::action::{Action, VoiceOp};
use bilterm::api::json::JsonValue;
use bilterm::api::rpc::AutomationControlPlane;
use bilterm::input::keymap::{DEFAULT_GLOBAL_BINDINGS, DEFAULT_LEADER_BINDINGS};
use bilterm::input::mouse::handle_wm_mouse_event;
use bilterm::stt::SttBackendKind;
use bilterm::ui::modals::{menubar_category_item_count, menubar_item_command, PALETTE_CATALOG};
use bilterm::ui::oia::{oia_mic_badge_rect, render_oia_status_bar};
use bilterm::wm::WindowManager;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_stt_keymap_and_leader_chords() {
    // 1. Leader ',' is bound to Action::Voice(VoiceOp::Toggle)
    let leader_comma = DEFAULT_LEADER_BINDINGS
        .iter()
        .find(|(k, _)| *k == KeyCode::Char(','));
    assert!(
        leader_comma.is_some(),
        "Leader ',' must be bound in DEFAULT_LEADER_BINDINGS"
    );
    assert_eq!(
        leader_comma.unwrap().1,
        Action::Voice(VoiceOp::Toggle),
        "Leader ',' must map to Action::Voice(VoiceOp::Toggle)"
    );

    // 2. Global Ctrl+Alt+S / Ctrl+Alt+s is bound to Action::Voice(VoiceOp::Toggle)
    let global_s = DEFAULT_GLOBAL_BINDINGS.iter().find(|(m, k, _)| {
        *m == KeyModifiers::CONTROL.union(KeyModifiers::ALT) && *k == KeyCode::Char('s')
    });
    assert!(
        global_s.is_some(),
        "Ctrl+Alt+s must be bound in DEFAULT_GLOBAL_BINDINGS"
    );
    assert_eq!(
        global_s.unwrap().2,
        Action::Voice(VoiceOp::Toggle),
        "Ctrl+Alt+s must map to Action::Voice(VoiceOp::Toggle)"
    );

    let global_cap_s = DEFAULT_GLOBAL_BINDINGS.iter().find(|(m, k, _)| {
        *m == KeyModifiers::CONTROL.union(KeyModifiers::ALT) && *k == KeyCode::Char('S')
    });
    assert!(
        global_cap_s.is_some(),
        "Ctrl+Alt+S must be bound in DEFAULT_GLOBAL_BINDINGS"
    );
    assert_eq!(
        global_cap_s.unwrap().2,
        Action::Voice(VoiceOp::Toggle),
        "Ctrl+Alt+S must map to Action::Voice(VoiceOp::Toggle)"
    );

    // Leader 'v' MUST remain bound to PasteClipboard (anti-regression check)
    let leader_v = DEFAULT_LEADER_BINDINGS
        .iter()
        .find(|(k, _)| *k == KeyCode::Char('v'));
    assert_eq!(
        leader_v.unwrap().1,
        Action::PasteClipboard,
        "Leader 'v' must remain bound to PasteClipboard"
    );
}

#[test]
fn test_stt_colon_commands_and_palette_parity() {
    let mut wm = WindowManager::new_headless(24, 80);

    // 1. :voice start
    let res = wm.apply(Action::Voice(VoiceOp::Start { pane_id: None }));
    assert!(res.is_ok());
    assert!(wm.stt_status().recording);

    // 2. :voice stop
    let res = wm.apply(Action::Voice(VoiceOp::Stop));
    assert!(res.is_ok());
    assert!(!wm.stt_status().recording);
    assert!(wm.stt_status().transcribing);

    // 3. :voice cancel
    let res = wm.apply(Action::Voice(VoiceOp::Cancel));
    assert!(res.is_ok());
    assert!(!wm.stt_status().recording);
    assert!(!wm.stt_status().transcribing);

    // 4. :voice provider mock
    let res = wm.apply(Action::Voice(VoiceOp::SetProvider {
        provider: SttBackendKind::Mock,
    }));
    assert!(res.is_ok());
    assert_eq!(wm.stt_status().active_provider, SttBackendKind::Mock);

    // 5. :voice status
    let res = wm.apply(Action::Voice(VoiceOp::Status));
    assert!(res.is_ok());
    let data = wm.take_command_data().expect("status command data");
    assert!(data.contains("\"mock\""));

    // 6. Verify PALETTE_CATALOG has :voice toggle
    assert!(
        PALETTE_CATALOG
            .iter()
            .any(|(cmd, _, _)| *cmd == ":voice toggle"),
        "PALETTE_CATALOG must contain :voice toggle"
    );

    // 7. Verify TopMenu View category contains :voice toggle
    let view_items_count = menubar_category_item_count(3);
    let has_menubar_voice = (0..view_items_count)
        .filter_map(|i| menubar_item_command(3, i))
        .any(|cmd| cmd == ":voice toggle");
    assert!(
        has_menubar_voice,
        "TopMenu View category must contain :voice toggle"
    );
}

#[test]
fn test_stt_mouse_hit_testing_on_oia_badge() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.stt_status_mut().enabled = true;
    wm.sync_stt_badges();

    let pane = wm.active_pane();
    assert_eq!(pane.stt_badge(), Some("[🎙 MIC]"));

    let viewport = Rect::new(0, 0, 80, 24);
    let status_rect = pane.status_bar_rect(viewport);
    let mic_rect = oia_mic_badge_rect(pane, status_rect).expect("oia_mic_badge_rect");
    assert!(mic_rect.width >= 5);
    assert_eq!(mic_rect.y, status_rect.y);

    // Render OIA bar to test buffer
    let mut buf = Buffer::empty(viewport);
    render_oia_status_bar(pane, status_rect, &mut buf);

    // Click inside the badge
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: mic_rect.x + 1,
        row: mic_rect.y,
        modifiers: KeyModifiers::NONE,
    };
    let app_action = handle_wm_mouse_event(&mut wm, mouse, viewport).expect("mouse click");
    assert_eq!(app_action, bilterm::input::AppAction::Handled);

    // Should have toggled into recording state
    assert!(
        wm.stt_status().recording,
        "Clicking OIA MIC badge must toggle recording on"
    );

    // Click again to toggle off
    wm.sync_stt_badges();
    let pane = wm.active_pane();
    let status_rect = pane.status_bar_rect(viewport);
    let mic_rect = oia_mic_badge_rect(pane, status_rect).expect("oia_mic_badge_rect recording");

    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: mic_rect.x + 1,
        row: mic_rect.y,
        modifiers: KeyModifiers::NONE,
    };
    let app_action = handle_wm_mouse_event(&mut wm, mouse, viewport).expect("mouse click 2");
    assert_eq!(app_action, bilterm::input::AppAction::Handled);
    assert!(
        !wm.stt_status().recording,
        "Clicking OIA REC badge must toggle recording off"
    );
}

#[test]
fn test_stt_rpc_and_mcp_surface_uniformity() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);

    // 1. JSON-RPC voice.status
    let status_resp = plane
        .dispatch_method("voice.status", &JsonValue::Null)
        .expect("voice.status");
    assert_eq!(
        status_resp.get("recording").and_then(JsonValue::as_bool),
        Some(false)
    );

    // 2. JSON-RPC voice.toggle
    let toggle_resp = plane
        .dispatch_method("voice.toggle", &JsonValue::Null)
        .expect("voice.toggle");
    assert_eq!(
        toggle_resp.get("recording").and_then(JsonValue::as_bool),
        Some(true)
    );

    // 3. JSON-RPC voice.toggle again (off)
    let toggle_off = plane
        .dispatch_method("voice.toggle", &JsonValue::Null)
        .expect("voice.toggle off");
    assert_eq!(
        toggle_off.get("recording").and_then(JsonValue::as_bool),
        Some(false)
    );

    // 4. JSON-RPC voice.provider
    let prov_resp = plane
        .dispatch_method(
            "voice.provider",
            &JsonValue::obj([("provider", JsonValue::String("mock".into()))]),
        )
        .expect("voice.provider");
    assert_eq!(
        prov_resp.get("active_provider").and_then(JsonValue::as_str),
        Some("mock")
    );

    // 5. MCP tools/call bilterm_voice toggle
    let mcp_call = plane
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":100,"method":"tools/call","params":{"name":"bilterm_voice","arguments":{"action":"toggle"}}}"#,
        )
        .expect("mcp voice toggle");
    assert!(mcp_call.contains("\"isError\":false"), "{mcp_call}");
    assert!(mcp_call.contains("recording") && mcp_call.contains("true"), "{mcp_call}");

    // 6. MCP tools/call bilterm_voice status
    let mcp_status = plane
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":101,"method":"tools/call","params":{"name":"bilterm_voice","arguments":{"action":"status"}}}"#,
        )
        .expect("mcp voice status");
    assert!(mcp_status.contains("\"isError\":false"), "{mcp_status}");
    assert!(mcp_status.contains("mock"), "{mcp_status}");
}

#[test]
fn test_voice_prompt_staging_and_auto_submit() {
    let mut wm = WindowManager::new_headless(24, 80);
    wm.config_mut().stt.auto_submit = false;

    // Apply transcript with auto_submit = false
    let transcript = "git status -s".to_string();
    wm.apply(Action::Voice(VoiceOp::ApplyTranscript {
        pane_id: None,
        text: transcript.clone(),
        is_final: true,
    }))
    .unwrap();

    let pane = wm.active_pane();
    assert_eq!(
        pane.prompt().text(),
        "git status -s",
        "Dictated text must be staged in FloatingCommandPrompt"
    );
    assert_eq!(
        wm.stt_status().last_transcript.as_deref(),
        Some("git status -s")
    );

    // Append second phrase with automatic space separation
    wm.apply(Action::Voice(VoiceOp::ApplyTranscript {
        pane_id: None,
        text: "&& cargo check".to_string(),
        is_final: true,
    }))
    .unwrap();

    let pane = wm.active_pane();
    assert_eq!(
        pane.prompt().text(),
        "git status -s && cargo check",
        "Subsequent dictated text must be space-separated"
    );

    // Now test auto_submit = true
    wm.config_mut().stt.auto_submit = true;
    wm.active_pane_mut().prompt_mut().clear_line();

    wm.apply(Action::Voice(VoiceOp::ApplyTranscript {
        pane_id: None,
        text: "echo hello".to_string(),
        is_final: true,
    }))
    .unwrap();

    let pane = wm.active_pane();
    // Prompt line is cleared upon submission
    assert_eq!(
        pane.prompt().text(),
        "",
        "Prompt should be submitted and cleared when auto_submit is true"
    );
}
