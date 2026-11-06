//! Phase 52 Integration Test Suite: Agent-to-Agent Messaging & Coordination (`AGT-01..03`).

use std::fs;
use std::time::Instant;

use bilterm::action::Action;
use bilterm::api::{AutomationControlPlane, JsonValue};
use bilterm::wm::agent_mail::{
    AgentMailOp, AgentMessage, MAX_BODY_BYTES, MAX_HOPS, MAX_INBOX_DEPTH, MAX_MSGS_PER_WINDOW,
};
use bilterm::wm::drawer::DrawerRowKind;

fn new_plane() -> AutomationControlPlane {
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    // Split to have at least 2 panes
    plane.wm.apply(Action::SplitVertical).unwrap();
    plane
}

#[test]
fn test_01_two_agent_round_trip() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);

    // Register pane #1 as "alice" and pane #2 as "bob"
    let _ = plane
        .dispatch_method(
            "agent.name",
            &JsonValue::obj([
                ("pane_id", JsonValue::Number(1)),
                ("name", JsonValue::String("alice".to_string())),
            ]),
        )
        .expect("name alice");

    let _ = plane
        .dispatch_method(
            "agent.name",
            &JsonValue::obj([
                ("pane_id", JsonValue::Number(2)),
                ("name", JsonValue::String("bob".to_string())),
            ]),
        )
        .expect("name bob");

    // Alice sends to Bob
    let send_res = plane
        .dispatch_method(
            "agent.send",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("to", JsonValue::String("bob".to_string())),
                ("body", JsonValue::String("Hello Bob!".to_string())),
                ("kind", JsonValue::String("greeting".to_string())),
            ]),
        )
        .expect("send message");

    assert_eq!(send_res.get("to").and_then(JsonValue::as_str), Some("bob"));
    assert_eq!(
        send_res.get("from").and_then(JsonValue::as_str),
        Some("alice")
    );
    let msg_id = send_res.get("message_id").and_then(JsonValue::as_i64).unwrap();
    assert_eq!(msg_id, 1);

    // Bob checks inbox
    let inbox_res = plane
        .dispatch_method(
            "agent.inbox",
            &JsonValue::obj([
                ("agent", JsonValue::String("bob".to_string())),
                ("unread_only", JsonValue::Bool(true)),
            ]),
        )
        .expect("inbox query");

    assert_eq!(
        inbox_res.get("unread").and_then(JsonValue::as_i64),
        Some(1)
    );
    let messages = inbox_res
        .get("messages")
        .and_then(JsonValue::as_array)
        .unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].get("body").and_then(JsonValue::as_str),
        Some("Hello Bob!")
    );

    // Bob acks message
    let ack_res = plane
        .dispatch_method(
            "agent.ack",
            &JsonValue::obj([
                ("agent", JsonValue::String("bob".to_string())),
                ("message_id", JsonValue::Number(msg_id)),
            ]),
        )
        .expect("ack message");

    assert_eq!(ack_res.get("count").and_then(JsonValue::as_i64), Some(1));
    assert_eq!(plane.wm.agent_mail().unread_count_by_name("bob"), 0);
}

#[test]
fn test_02_unknown_recipient_error_no_silent_drop() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();

    let err = plane.dispatch_method(
        "agent.send",
        &JsonValue::obj([
            ("from", JsonValue::String("alice".to_string())),
            ("to", JsonValue::String("ghost".to_string())),
            ("body", JsonValue::String("Are you there?".to_string())),
        ]),
    );

    assert!(err.is_err());
    let err_msg = err.unwrap_err();
    assert!(
        err_msg.contains("Unknown agent 'ghost'"),
        "error was: {err_msg}"
    );
}

#[test]
fn test_03_gating_off_by_default_rejected() {
    let mut plane = new_plane();
    // Gating is off by default
    assert!(!plane.wm.is_agent_messaging_enabled());
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    let err = plane.dispatch_method(
        "agent.send",
        &JsonValue::obj([
            ("from", JsonValue::String("alice".to_string())),
            ("to", JsonValue::String("bob".to_string())),
            ("body", JsonValue::String("Hello".to_string())),
        ]),
    );

    assert!(err.is_err());
    let err_msg = err.unwrap_err();
    assert!(
        err_msg.contains("Agent messaging is disabled"),
        "error was: {err_msg}"
    );
}

#[test]
fn test_04_gating_toggle_via_colon_command_enables() {
    let mut plane = new_plane();
    assert!(!plane.wm.is_agent_messaging_enabled());

    // Enable via colon command
    plane
        .wm
        .apply(Action::AgentMail(AgentMailOp::Enable(Some(true))))
        .unwrap();
    assert!(plane.wm.is_agent_messaging_enabled());

    // Disable via colon command
    plane
        .wm
        .apply(Action::AgentMail(AgentMailOp::Enable(Some(false))))
        .unwrap();
    assert!(!plane.wm.is_agent_messaging_enabled());
}

#[test]
fn test_05_loop_guard_rejects_exceeding_max_hops() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    let mut last_id = None;
    for i in 1..=MAX_HOPS {
        let (from, to) = if i % 2 == 1 {
            ("alice", "bob")
        } else {
            ("bob", "alice")
        };
        let mut params = vec![
            ("from", JsonValue::String(from.to_string())),
            ("to", JsonValue::String(to.to_string())),
            ("body", JsonValue::String(format!("hop {i}"))),
        ];
        if let Some(lid) = last_id {
            params.push(("reply_to", JsonValue::Number(lid)));
        }
        let res = plane
            .dispatch_method("agent.send", &JsonValue::obj(params))
            .expect("hop send");
        last_id = res.get("message_id").and_then(JsonValue::as_i64);
    }

    // 9th hop exceeds MAX_HOPS (8)
    let res = plane.dispatch_method(
        "agent.send",
        &JsonValue::obj([
            ("from", JsonValue::String("alice".to_string())),
            ("to", JsonValue::String("bob".to_string())),
            ("body", JsonValue::String("hop 9".to_string())),
            ("reply_to", JsonValue::Number(last_id.unwrap())),
        ]),
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("loop guard triggered"),
        "expected loop guard error, got: {err}"
    );
}

#[test]
fn test_06_inbox_queue_depth_cap_rejection() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    // Fill queue to MAX_INBOX_DEPTH (64) directly into mailbox
    for i in 1..=MAX_INBOX_DEPTH {
        let msg = AgentMessage {
            id: 0,
            from: "alice".to_string(),
            to: "bob".to_string(),
            body: format!("msg {i}"),
            kind: "chat".to_string(),
            reply_to: None,
            hop_count: 1,
            timestamp_ms: 0,
            acked: false,
            file_path: None,
        };
        plane.wm.agent_mail_mut().enqueue(msg).unwrap();
    }

    // 65th must be rejected
    let res = plane.dispatch_method(
        "agent.send",
        &JsonValue::obj([
            ("from", JsonValue::String("charlie".to_string())),
            ("to", JsonValue::String("bob".to_string())),
            ("body", JsonValue::String("overflow".to_string())),
        ]),
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("is full"), "expected full queue error, got: {err}");
}

#[test]
fn test_07_rate_limit_per_sender_rejection() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    let now = Instant::now();

    for _ in 0..MAX_MSGS_PER_WINDOW {
        assert!(plane.wm.agent_mail_mut().check_rate_limit("spammer", now).is_ok());
    }

    let err = plane.wm.agent_mail_mut().check_rate_limit("spammer", now);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("Rate limit exceeded"));
}

#[test]
fn test_08_oversized_body_rejection() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    let huge_body = "x".repeat(MAX_BODY_BYTES + 1);
    let res = plane.dispatch_method(
        "agent.send",
        &JsonValue::obj([
            ("from", JsonValue::String("alice".to_string())),
            ("to", JsonValue::String("bob".to_string())),
            ("body", JsonValue::String(huge_body)),
        ]),
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("Message body exceeds"),
        "expected size cap error, got: {err}"
    );
}

#[test]
fn test_09_nudge_delivery_succeeds_when_pane_idle() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    let res = plane
        .dispatch_method(
            "agent.send",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("to", JsonValue::String("bob".to_string())),
                ("body", JsonValue::String("Check this out".to_string())),
                ("delivery", JsonValue::String("nudge".to_string())),
            ]),
        )
        .expect("nudge send");

    assert_eq!(
        res.get("status").and_then(JsonValue::as_str),
        Some("nudged")
    );
    assert_eq!(res.get("nudged").and_then(JsonValue::as_bool), Some(true));

    // Verify raw PTY write on pane 2 contains comment notification
    let pty_bytes = plane.wm.pane(2).unwrap().last_pty_write();
    let text = String::from_utf8_lossy(pty_bytes);
    assert!(
        text.contains("# [mail from alice #1]: Check this out"),
        "text was: {text}"
    );
}

#[test]
fn test_10_nudge_refused_when_target_in_alt_screen() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    // Target pane #2 enters alternate screen (e.g. vim or htop)
    plane.wm.pane_mut(2).unwrap().process_pty_output(b"\x1b[?1049h");
    assert!(plane.wm.pane(2).unwrap().vt().is_alternate_screen());

    let res = plane
        .dispatch_method(
            "agent.send",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("to", JsonValue::String("bob".to_string())),
                ("body", JsonValue::String("Do not interrupt vim".to_string())),
                ("delivery", JsonValue::String("nudge".to_string())),
            ]),
        )
        .expect("nudge send");

    assert_eq!(
        res.get("status").and_then(JsonValue::as_str),
        Some("queued in mailbox (nudge suppressed: pane busy/locked)")
    );
    assert_eq!(res.get("nudged").and_then(JsonValue::as_bool), Some(false));

    // Message is still safely queued in mailbox
    assert_eq!(plane.wm.agent_mail().unread_count_by_name("bob"), 1);
}

#[test]
fn test_11_nudge_refused_when_target_in_password_lock() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    // Simulate password prompt in pane #2
    {
        let pane = plane.wm.pane_mut(2).unwrap();
        pane.process_pty_output(b"[sudo] password for operator: ");
        pane.line_discipline_mut().echo_enabled = false;
    }
    assert!(
        plane.wm.pane(2).unwrap().is_password_prompt()
            || plane
                .wm
                .pane(2)
                .unwrap()
                .is_active_canvas_password_prompt()
    );

    let res = plane
        .dispatch_method(
            "agent.send",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("to", JsonValue::String("bob".to_string())),
                ("body", JsonValue::String("Secret msg".to_string())),
                ("delivery", JsonValue::String("nudge".to_string())),
            ]),
        )
        .expect("send during password prompt");

    assert_eq!(
        res.get("status").and_then(JsonValue::as_str),
        Some("queued in mailbox (nudge suppressed: pane busy/locked)")
    );
    assert_eq!(res.get("nudged").and_then(JsonValue::as_bool), Some(false));
}

#[test]
fn test_12_file_delivery_writes_private_file() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    let res = plane
        .dispatch_method(
            "agent.send",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("to", JsonValue::String("bob".to_string())),
                ("body", JsonValue::String("Big report content".to_string())),
                ("delivery", JsonValue::String("file".to_string())),
            ]),
        )
        .expect("file delivery");

    let file_path = res
        .get("file_path")
        .and_then(JsonValue::as_str)
        .expect("file_path attached");
    assert!(file_path.contains("agent-mail/bob/msg-1.txt"));

    let content = fs::read_to_string(file_path).expect("read written file");
    assert_eq!(content, "Big report content");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(file_path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600, "file must have 0600 permissions");
    }
}

#[test]
fn test_13_broadcast_delivers_to_all_except_sender() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane
        .wm
        .apply(Action::SplitHorizontal)
        .unwrap(); // Now 3 panes
    plane
        .wm
        .agent_mail_mut()
        .register_name(1, "alice")
        .unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();
    plane
        .wm
        .agent_mail_mut()
        .register_name(3, "charlie")
        .unwrap();

    let res = plane
        .dispatch_method(
            "agent.broadcast",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("body", JsonValue::String("Team update".to_string())),
            ]),
        )
        .expect("broadcast");

    assert_eq!(res.get("count").and_then(JsonValue::as_i64), Some(2));
    assert_eq!(plane.wm.agent_mail().unread_count_by_name("bob"), 1);
    assert_eq!(plane.wm.agent_mail().unread_count_by_name("charlie"), 1);
    assert_eq!(plane.wm.agent_mail().unread_count_by_name("alice"), 0);
}

#[test]
fn test_14_mcp_resource_agent_inbox() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    // Send a message to bob
    plane
        .wm
        .agent_mail_mut()
        .enqueue(bilterm::wm::agent_mail::AgentMessage {
            id: 0,
            from: "alice".to_string(),
            to: "bob".to_string(),
            body: "Resource test message".to_string(),
            kind: "test".to_string(),
            reply_to: None,
            hop_count: 1,
            timestamp_ms: 1000,
            acked: false,
            file_path: None,
        })
        .unwrap();

    // Check resources/list advertises the inbox resource
    let list_req = r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#;
    let list_resp = JsonValue::parse(
        &plane
            .handle_mcp_line(list_req)
            .expect("mcp handle"),
    )
    .unwrap();
    let resources = list_resp
        .get("result")
        .and_then(|r| r.get("resources"))
        .and_then(JsonValue::as_array)
        .unwrap();
    assert!(
        resources
            .iter()
            .any(|r| r.get("uri").and_then(JsonValue::as_str)
                == Some("bilterm://agent/bob/inbox")),
        "agent resource missing from resources/list"
    );

    // Read the resource via resources/read
    let read_req = r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"bilterm://agent/bob/inbox"}}"#;
    let read_resp = JsonValue::parse(
        &plane
            .handle_mcp_line(read_req)
            .expect("mcp handle"),
    )
    .unwrap();
    let contents = read_resp
        .get("result")
        .and_then(|r| r.get("contents"))
        .and_then(JsonValue::as_array)
        .unwrap();
    let text = contents[0].get("text").and_then(JsonValue::as_str).unwrap();
    assert!(text.contains("Resource test message"));
}

#[test]
fn test_15_unread_badge_increments_and_clears_on_ack() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane.wm.agent_mail_mut().register_name(1, "alice").unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    assert_eq!(plane.wm.agent_mail().unread_count_by_pane(2), 0);

    // Initial drawer rows has no mail badge
    let rows = plane.wm.drawer_rows();
    let bob_row = rows.iter().find(|r| r.kind == DrawerRowKind::Pane(2)).unwrap();
    assert!(!bob_row.mode_badges.iter().any(|b| b.starts_with('✉')));

    // Send a message
    plane
        .dispatch_method(
            "agent.send",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("to", JsonValue::String("bob".to_string())),
                ("body", JsonValue::String("Unread test".to_string())),
            ]),
        )
        .unwrap();

    assert_eq!(plane.wm.agent_mail().unread_count_by_pane(2), 1);

    // Drawer rows now carries unread mail badge
    let rows2 = plane.wm.drawer_rows();
    let bob_row2 = rows2.iter().find(|r| r.kind == DrawerRowKind::Pane(2)).unwrap();
    assert!(
        bob_row2.mode_badges.iter().any(|b| b == "✉ 1"),
        "expected mail badge in badges: {:?}",
        bob_row2.mode_badges
    );

    // AgentsList rows also has unread_mail == 1
    plane.wm.update_agent_states(Instant::now());
    let agent_rows = plane.wm.agents_list_rows();
    let bob_agent_row = agent_rows.iter().find(|r| r.pane_id == 2).unwrap();
    assert_eq!(bob_agent_row.unread_mail, 1);

    // Acknowledge the mail
    plane
        .dispatch_method(
            "agent.ack",
            &JsonValue::obj([
                ("agent", JsonValue::String("bob".to_string())),
            ]),
        )
        .unwrap();

    assert_eq!(plane.wm.agent_mail().unread_count_by_pane(2), 0);
    let rows3 = plane.wm.drawer_rows();
    let bob_row3 = rows3.iter().find(|r| r.kind == DrawerRowKind::Pane(2)).unwrap();
    assert!(!bob_row3.mode_badges.iter().any(|b| b.starts_with('✉')));
}

#[test]
fn test_16_agent_wait_with_reply_and_timeout() {
    let mut plane = new_plane();
    plane.wm.set_agent_messaging_enabled(true);
    plane.wm.agent_mail_mut().register_name(1, "alice").unwrap();
    plane.wm.agent_mail_mut().register_name(2, "bob").unwrap();

    // Timeout when no message arrives
    let timeout_res = plane
        .dispatch_method(
            "agent.wait",
            &JsonValue::obj([
                ("agent", JsonValue::String("bob".to_string())),
                ("timeout_ms", JsonValue::Number(20)),
            ]),
        )
        .expect("wait timeout");

    assert_eq!(
        timeout_res.get("status").and_then(JsonValue::as_str),
        Some("timeout")
    );

    // Send a reply
    plane
        .dispatch_method(
            "agent.send",
            &JsonValue::obj([
                ("from", JsonValue::String("alice".to_string())),
                ("to", JsonValue::String("bob".to_string())),
                ("body", JsonValue::String("Here is the answer".to_string())),
                ("reply_to", JsonValue::Number(42)),
            ]),
        )
        .unwrap();

    // Wait specifically for reply_to 42
    let wait_res = plane
        .dispatch_method(
            "agent.wait",
            &JsonValue::obj([
                ("agent", JsonValue::String("bob".to_string())),
                ("reply_to", JsonValue::Number(42)),
                ("timeout_ms", JsonValue::Number(500)),
            ]),
        )
        .expect("wait reply");

    assert_eq!(
        wait_res.get("status").and_then(JsonValue::as_str),
        Some("received")
    );
    let msg = wait_res.get("message").unwrap();
    assert_eq!(
        msg.get("body").and_then(JsonValue::as_str),
        Some("Here is the answer")
    );
    assert_eq!(msg.get("reply_to").and_then(JsonValue::as_i64), Some(42));
}

#[test]
fn test_17_colon_command_surface_parity() {
    let mut plane = new_plane();

    // :agent enable on
    plane.wm.apply(bilterm::colon_command::parse_colon_command(":agent enable on").unwrap().unwrap()).unwrap();
    assert!(plane.wm.is_agent_messaging_enabled());

    // :agent name
    plane.wm.apply(bilterm::colon_command::parse_colon_command(":agent name 1 alice").unwrap().unwrap()).unwrap();
    plane.wm.apply(bilterm::colon_command::parse_colon_command(":agent name 2 bob").unwrap().unwrap()).unwrap();
    assert_eq!(plane.wm.agent_mail().name_for_pane(1), Some("alice"));
    assert_eq!(plane.wm.agent_mail().name_for_pane(2), Some("bob"));

    // :agent send
    plane.wm.apply(bilterm::colon_command::parse_colon_command(":agent send bob Greetings from colon").unwrap().unwrap()).unwrap();
    assert_eq!(plane.wm.agent_mail().unread_count_by_name("bob"), 1);

    // :agent inbox
    plane.wm.apply(bilterm::colon_command::parse_colon_command(":agent inbox bob").unwrap().unwrap()).unwrap();

    // :agent ack
    plane.wm.apply(bilterm::colon_command::parse_colon_command(":agent ack bob all").unwrap().unwrap()).unwrap();
    assert_eq!(plane.wm.agent_mail().unread_count_by_name("bob"), 0);

    // :agent list
    plane.wm.apply(bilterm::colon_command::parse_colon_command(":agent list").unwrap().unwrap()).unwrap();
}
