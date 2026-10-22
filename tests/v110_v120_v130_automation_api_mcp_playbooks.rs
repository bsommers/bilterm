use bilterm::api::{AutomationControlPlane, JsonValue, Playbook, send_rpc_request};

#[test]
fn v110_jsonrpc_control_plane_manages_workspace_panes_and_safety_guards() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);

    // 1. Query initial workspace status
    let status_resp = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":1,"method":"workspace.status","params":{}}"#,
    );
    let parsed = JsonValue::parse(&status_resp).expect("valid JSON-RPC response");
    assert_eq!(
        parsed
            .get("result")
            .and_then(|r| r.get("pane_count"))
            .and_then(JsonValue::as_i64),
        Some(1)
    );

    // 2. Split pane vertically via wm.command (:split v)
    let split_resp = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":2,"method":"wm.command","params":{"command":":split v"}}"#,
    );
    let split_json = JsonValue::parse(&split_resp).expect("valid split response");
    assert_eq!(
        split_json
            .get("result")
            .and_then(|r| r.get("pane_count"))
            .and_then(JsonValue::as_i64),
        Some(2)
    );

    // 3. Submit a command via pane.exec and feed OSC 133 + URL output via pane.feed_output
    let exec_resp = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":3,"method":"pane.exec","params":{"pane_id":1,"command":"curl https://bilterm.dev/api"}}"#,
    );
    assert!(exec_resp.contains("\"submitted\""));

    let feed_resp = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":4,"method":"pane.feed_output","params":{"pane_id":1,"data":"\u001b]133;C\u0007Response from https://bilterm.dev/api (commit deadbeefcafebabe)\r\n\u001b]133;D;0\u0007"}}"#,
    );
    assert!(feed_resp.contains("\"processed\""));

    // 4. Read structured pane state (lines, 3270 QuickSelect hints, and OSC 133 command blocks)
    let read_resp = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":5,"method":"pane.read","params":{"pane_id":1}}"#,
    );
    let read_json = JsonValue::parse(&read_resp).expect("valid read JSON");
    let result = read_json.get("result").expect("result object");
    let hints = result
        .get("hints")
        .and_then(JsonValue::as_array)
        .expect("hints array");
    assert!(
        hints
            .iter()
            .any(|h| h.get("text").and_then(JsonValue::as_str) == Some("https://bilterm.dev/api"))
    );
    let blocks = result
        .get("command_blocks")
        .and_then(JsonValue::as_array)
        .expect("command_blocks array");
    assert_eq!(
        blocks
            .last()
            .and_then(|b| b.get("exit_code"))
            .and_then(JsonValue::as_i64),
        Some(0)
    );

    // 5. Verify read-only lock (🔒 PROT) blocks pane.exec unless unlocked
    let _ = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":6,"method":"wm.command","params":{"command":":focus 1"}}"#,
    );
    let _ = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":7,"method":"wm.command","params":{"command":":lock on"}}"#,
    );
    let locked_resp = plane.handle_jsonrpc_line(
        r#"{"jsonrpc":"2.0","id":8,"method":"pane.exec","params":{"pane_id":1,"command":"rm -rf /"}}"#,
    );
    assert!(locked_resp.contains("locked read-only"));
}

#[test]
fn v120_mcp_server_protocol_exposes_and_executes_agent_tools() {
    let mut plane = AutomationControlPlane::new_headless(24, 80);

    // 1. initialize handshake
    let init_resp = plane
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
        .expect("initialize response");
    let init_json = JsonValue::parse(&init_resp).expect("valid JSON");
    assert_eq!(
        init_json
            .get("result")
            .and_then(|r| r.get("serverInfo"))
            .and_then(|s| s.get("name"))
            .and_then(JsonValue::as_str),
        Some("bilterm-mcp")
    );

    // 2. notifications/initialized (no id -> returns None)
    assert!(
        plane
            .handle_mcp_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none()
    );

    // 3. tools/list
    let list_resp = plane
        .handle_mcp_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
        .expect("tools/list response");
    let list_json = JsonValue::parse(&list_resp).expect("valid tools/list JSON");
    let tools = list_json
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(JsonValue::as_array)
        .expect("tools array");
    assert_eq!(tools.len(), 6);

    // 4. tools/call -> bilterm_colon_command (:theme 3278)
    let call_resp = plane
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bilterm_colon_command","arguments":{"command":":theme 3278"}}}"#,
        )
        .expect("tools/call response");
    assert!(call_resp.contains("\"isError\":false"));
    assert_eq!(
        plane.wm.config.theme,
        bilterm::ui::theme::ThemePreset::Ibm3278Green
    );

    // 5. tools/call -> bilterm_send_keys (Ctrl+P opens Fuzzy Command Palette)
    let key_resp = plane
        .handle_mcp_line(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"bilterm_send_keys","arguments":{"keys":"Ctrl+P"}}}"#,
        )
        .expect("tools/call send_keys response");
    assert!(key_resp.contains("\"isError\":false"));
    assert!(plane.wm.palette_visible);
}

#[tokio::test]
async fn v130_declarative_playbook_and_live_tcp_rpc_client_work_end_to_end() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ppm_path = dir.path().join("playbook-frame.ppm");

    let toml_playbook = format!(
        r#"
name = "robot-ci-verification"

[[steps]]
action = "colon"
command = ":trigger add CRITICAL_FAULT"

[[steps]]
action = "exec"
pane_id = 1
command = "run_diagnostics --all"

[[steps]]
action = "feed_output"
pane_id = 1
data = "\u001b]133;C\u0007[WARN] CRITICAL_FAULT detected on channel 4\r\n\u001b]9;Alert from sensor\u0007\u001b]133;D;42\u0007"

[[steps]]
action = "assert_screen_contains"
pane_id = 1
expected = "CRITICAL_FAULT detected"

[[steps]]
action = "assert_trigger_fired"
pattern = "CRITICAL_FAULT"

[[steps]]
action = "assert_exit_code"
pane_id = 1
exit_code = 42

[[steps]]
action = "export_ppm"
path = "{}"
"#,
        ppm_path.display()
    );

    let playbook = Playbook::from_toml_str(&toml_playbook).expect("valid TOML playbook");
    let mut plane = AutomationControlPlane::new_headless(24, 80);
    let report = plane.run_playbook(&playbook).expect("playbook passes");
    assert!(report.passed);
    assert_eq!(report.steps_completed, 7);
    assert!(ppm_path.exists());

    // Poll structured events via JSON-RPC events.poll
    let poll_resp =
        plane.handle_jsonrpc_line(r#"{"jsonrpc":"2.0","id":9,"method":"events.poll","params":{}}"#);
    assert!(poll_resp.contains("command_submitted"));
    assert!(poll_resp.contains("trigger_matched"));
    assert!(poll_resp.contains("notification_received"));
    assert!(poll_resp.contains("command_completed"));

    // Verify live TCP JSON-RPC server + send_rpc_request client roundtrip
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr").to_string();

    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let mut server_plane = AutomationControlPlane::new_headless(24, 80);
        if let Ok((stream, _)) = listener.accept().await {
            let (rd, mut wr) = stream.into_split();
            let mut lines = BufReader::new(rd).lines();
            if let Ok(Some(line)) = lines.next_line().await {
                let resp = server_plane.handle_jsonrpc_line(&line);
                let _ = wr.write_all(format!("{resp}\n").as_bytes()).await;
                let _ = wr.flush().await;
            }
        }
    });

    let client_resp = send_rpc_request(
        &addr,
        "workspace.status",
        JsonValue::obj([] as [(&str, JsonValue); 0]),
    )
    .await
    .expect("TCP RPC client request");
    assert!(client_resp.contains("\"layout_mode\":\"tiled\""));
}
