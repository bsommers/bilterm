use std::path::PathBuf;

use super::json::MAX_JSON_DEPTH;
use super::server::MAX_RPC_LINE_BYTES;
use super::*;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[test]
fn json_parser_rejects_pathological_nesting_without_overflowing_the_stack() {
    let deep = "[".repeat(200_000);
    assert!(JsonValue::parse(&deep).is_none());
    let ok = format!("{}{}", "[".repeat(MAX_JSON_DEPTH), "]".repeat(MAX_JSON_DEPTH));
    assert!(JsonValue::parse(&ok).is_some());
}

async fn status_roundtrip(addr: std::net::SocketAddr) -> String {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (rd, mut wr) = stream.into_split();
    wr.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"workspace.status\"}\n")
        .await
        .unwrap();
    let mut line = String::new();
    BufReader::new(rd).read_line(&mut line).await.unwrap();
    line
}

#[tokio::test]
async fn daemon_survives_abusive_clients_and_serves_concurrent_connections() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve_jsonrpc(
        listener,
        AutomationControlPlane::new_headless(24, 80),
    ));

    // An idle connection must not block other clients.
    let _idle = tokio::net::TcpStream::connect(addr).await.unwrap();

    // Deeply nested JSON gets a parse error instead of crashing the process.
    let mut deep = tokio::net::TcpStream::connect(addr).await.unwrap();
    deep.write_all(format!("{}\n", "[".repeat(200_000)).as_bytes())
        .await
        .unwrap();
    let mut resp = vec![0u8; 256];
    let n = deep.read(&mut resp).await.unwrap();
    assert!(String::from_utf8_lossy(&resp[..n]).contains("-32700"));

    // A client that closes with an unread reply pending (kernel sends RST) must not stop the daemon.
    let mut abrupt = tokio::net::TcpStream::connect(addr).await.unwrap();
    abrupt
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"workspace.status\"}\n")
        .await
        .unwrap();
    drop(abrupt);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // An oversized line closes only that connection.
    let mut huge = tokio::net::TcpStream::connect(addr).await.unwrap();
    let _ = huge
        .write_all(&vec![b'a'; (MAX_RPC_LINE_BYTES + 16) as usize])
        .await;

    let line = status_roundtrip(addr).await;
    assert!(line.contains("\"id\":7") && line.contains("result"), "{line}");
}

#[test]
#[cfg(unix)]
fn listen_policy_default_is_unix_socket() {
    let default_path = PathBuf::from("/tmp/test-bilterm/bilterm.sock");
    let cfg = validate_listen_config(None, false, None, Some(default_path.clone())).unwrap();
    assert_eq!(cfg.endpoint, ListenEndpoint::Unix(default_path));
    assert_eq!(cfg.token, None);

    let err = validate_listen_config(None, false, None, None).unwrap_err();
    assert!(err.to_string().contains("--listen unix:PATH"));
}

#[test]
fn listen_policy_loopback_tcp_without_token_ok() {
    for addr_str in ["127.0.0.1:7370", "[::1]:7370", "localhost:7370"] {
        let cfg = validate_listen_config(Some(addr_str), false, None, None).unwrap();
        match cfg.endpoint {
            ListenEndpoint::Tcp(addr) => {
                assert!(addr.ip().is_loopback(), "expected loopback for {addr_str}");
                assert_eq!(addr.port(), 7370);
            }
            _ => panic!("expected Tcp endpoint for {addr_str}"),
        }
        assert_eq!(cfg.token, None);
    }
}

#[test]
fn listen_policy_non_loopback_requires_allow_remote() {
    for addr_str in ["0.0.0.0:7370", "192.168.1.5:7370"] {
        let err = validate_listen_config(Some(addr_str), false, None, None).unwrap_err();
        assert!(
            err.to_string().contains("--allow-remote"),
            "expected --allow-remote error for {addr_str}, got: {err}"
        );
    }
}

#[test]
fn listen_policy_allow_remote_requires_token() {
    let err = validate_listen_config(Some("0.0.0.0:7370"), true, None, None).unwrap_err();
    assert!(
        err.to_string().contains("--token-file"),
        "expected --token-file error, got: {err}"
    );

    let cfg = validate_listen_config(
        Some("0.0.0.0:7370"),
        true,
        Some("mysecrettoken".to_string()),
        None,
    )
    .unwrap();
    match cfg.endpoint {
        ListenEndpoint::Tcp(addr) => assert_eq!(addr.port(), 7370),
        _ => panic!("expected Tcp endpoint"),
    }
    assert_eq!(cfg.token.as_deref(), Some("mysecrettoken"));
}

#[test]
#[cfg(unix)]
fn listen_policy_unix_prefixes() {
    for path_str in [
        "unix:/tmp/test.sock",
        "unix:///tmp/test.sock",
        "/tmp/test.sock",
    ] {
        let cfg = validate_listen_config(Some(path_str), false, None, None).unwrap();
        assert_eq!(
            cfg.endpoint,
            ListenEndpoint::Unix(PathBuf::from("/tmp/test.sock")),
            "mismatch for {path_str}"
        );
    }
}

#[test]
fn listen_policy_rejects_garbage() {
    let err = validate_listen_config(Some("not an addr"), false, None, None).unwrap_err();
    assert!(err.to_string().contains("invalid listen address"));
}

#[test]
fn listen_config_debug_redacts_token() {
    let cfg = ListenConfig {
        endpoint: ListenEndpoint::Tcp("127.0.0.1:7370".parse().unwrap()),
        token: Some("supersecrettoken123".to_string()),
    };
    let formatted = format!("{cfg:?}");
    assert!(
        !formatted.contains("supersecret"),
        "Debug output leaked token: {formatted}"
    );
    assert!(
        formatted.contains("<redacted>"),
        "Debug output missing <redacted>: {formatted}"
    );
}

#[test]
fn default_socket_path_rejects_overlong_path() {
    let long_dir = PathBuf::from(format!("/tmp/{}", "a".repeat(120)));
    let err = socket_path_in(&long_dir).unwrap_err();
    assert!(
        err.to_string().contains("--listen unix:PATH"),
        "expected hint in error: {err}"
    );

    let short_dir = PathBuf::from("/tmp/short");
    let ok = socket_path_in(&short_dir).unwrap();
    assert!(ok.ends_with("bilterm.sock"));
}

#[test]
#[cfg(unix)]
fn load_token_file_enforces_private_mode() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let token_path = temp_dir.path().join("token.secret");
    crate::secure_fs::write_private_file(&token_path, b"0123456789abcdef0123\n").unwrap();

    let loaded = load_token_file(&token_path).unwrap();
    assert_eq!(loaded, "0123456789abcdef0123");

    // Make file group/world readable: 0644
    std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let err = load_token_file(&token_path).unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("600") || err_str.contains("private"),
        "expected mode 600 complaint: {err_str}"
    );
    assert!(
        !err_str.contains("0123456789abcdef0123"),
        "token leaked in error message: {err_str}"
    );
}

#[tokio::test]
async fn bearer_token_gate_rejects_unauthenticated_first_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = std::sync::Arc::from("0123456789abcdef0123");
    tokio::spawn(serve_jsonrpc_tcp(
        listener,
        AutomationControlPlane::new_headless(24, 80),
        Some(token),
    ));

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"workspace.status\"}\n")
        .await
        .unwrap();

    let (rd, _) = stream.into_split();
    let mut lines = BufReader::new(rd).lines();
    let resp = lines.next_line().await.unwrap().unwrap_or_default();
    assert!(resp.contains("-32001"), "expected -32001 error, got: {resp}");
    assert!(
        !resp.contains("pane_count"),
        "unauthenticated request reached plane: {resp}"
    );
    let next = lines.next_line().await.unwrap();
    assert!(next.is_none(), "expected connection to be closed");
}

#[tokio::test]
async fn bearer_token_gate_rejects_wrong_token() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = std::sync::Arc::from("0123456789abcdef0123");
    tokio::spawn(serve_jsonrpc_tcp(
        listener,
        AutomationControlPlane::new_headless(24, 80),
        Some(token),
    ));

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"auth.login\",\"params\":{\"token\":\"wrongwrongwrongwrong\"}}\n")
        .await
        .unwrap();

    let (rd, _) = stream.into_split();
    let mut lines = BufReader::new(rd).lines();
    let resp = lines.next_line().await.unwrap().unwrap_or_default();
    assert!(resp.contains("-32001"), "expected -32001 error, got: {resp}");
    assert!(
        !resp.contains("wrongwrong"),
        "leaked wrong token in response: {resp}"
    );
    assert!(
        !resp.contains("0123456789abcdef"),
        "leaked secret token in response: {resp}"
    );
    let next = lines.next_line().await.unwrap();
    assert!(next.is_none(), "expected connection to be closed");
}

#[tokio::test]
async fn bearer_token_gate_accepts_correct_token() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = std::sync::Arc::from("0123456789abcdef0123");
    tokio::spawn(serve_jsonrpc_tcp(
        listener,
        AutomationControlPlane::new_headless(24, 80),
        Some(token),
    ));

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (rd, mut wr) = stream.into_split();
    let mut lines = BufReader::new(rd).lines();

    wr.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"auth.login\",\"params\":{\"token\":\"0123456789abcdef0123\"}}\n")
        .await
        .unwrap();
    wr.flush().await.unwrap();
    let auth_resp = lines.next_line().await.unwrap().unwrap_or_default();
    assert!(auth_resp.contains("\"authenticated\":true"), "got: {auth_resp}");

    wr.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"workspace.status\"}\n")
        .await
        .unwrap();
    wr.flush().await.unwrap();
    let status_resp = lines.next_line().await.unwrap().unwrap_or_default();
    assert!(status_resp.contains("pane_count"), "got: {status_resp}");
}

#[tokio::test]
async fn tokenless_server_accepts_auth_login() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve_jsonrpc(
        listener,
        AutomationControlPlane::new_headless(24, 80),
    ));

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (rd, mut wr) = stream.into_split();
    let mut lines = BufReader::new(rd).lines();

    wr.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"auth.login\",\"params\":{\"token\":\"anytoken\"}}\n")
        .await
        .unwrap();
    wr.flush().await.unwrap();
    let auth_resp = lines.next_line().await.unwrap().unwrap_or_default();
    assert!(auth_resp.contains("\"authenticated\":true"), "got: {auth_resp}");
}

#[tokio::test]
#[cfg(unix)]
async fn unix_socket_server_is_0600_and_roundtrips() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().unwrap();
    let sock_path = temp_dir.path().join("bilterm.sock");
    let cfg = ListenConfig {
        endpoint: ListenEndpoint::Unix(sock_path.clone()),
        token: None,
    };

    tokio::spawn(run_headless_server(
        cfg,
        AutomationControlPlane::new_headless(24, 80),
    ));

    let mut ready = false;
    for _ in 0..40 {
        if sock_path.exists() {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(ready, "socket was not created in time");

    let meta = std::fs::symlink_metadata(&sock_path).unwrap();
    assert_eq!(
        meta.permissions().mode() & 0o777,
        0o600,
        "socket mode is not 0600"
    );

    let resp = send_rpc_request(
        sock_path.to_str().unwrap(),
        "workspace.status",
        JsonValue::Null,
    )
    .await
    .unwrap();
    assert!(resp.contains("pane_count"), "roundtrip response: {resp}");
}

#[tokio::test]
#[cfg(unix)]
async fn unix_socket_stale_file_is_replaced() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sock_path = temp_dir.path().join("stale.sock");
    {
        let _ = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();
    }
    assert!(sock_path.exists());

    let cfg = ListenConfig {
        endpoint: ListenEndpoint::Unix(sock_path.clone()),
        token: None,
    };

    tokio::spawn(run_headless_server(
        cfg,
        AutomationControlPlane::new_headless(24, 80),
    ));

    let mut connected = false;
    for _ in 0..40 {
        if let Ok(resp) = send_rpc_request(
            sock_path.to_str().unwrap(),
            "workspace.status",
            JsonValue::Null,
        )
        .await
            && resp.contains("pane_count")
        {
            connected = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(connected, "failed to start server on stale socket path");
}

#[tokio::test]
#[cfg(unix)]
async fn unix_socket_live_daemon_is_not_stolen() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sock_path = temp_dir.path().join("live.sock");
    let cfg1 = ListenConfig {
        endpoint: ListenEndpoint::Unix(sock_path.clone()),
        token: None,
    };

    tokio::spawn(run_headless_server(
        cfg1,
        AutomationControlPlane::new_headless(24, 80),
    ));

    for _ in 0..40 {
        if sock_path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let cfg2 = ListenConfig {
        endpoint: ListenEndpoint::Unix(sock_path.clone()),
        token: None,
    };
    let err = run_headless_server(cfg2, AutomationControlPlane::new_headless(24, 80))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("already listening"),
        "expected already listening error, got: {err}"
    );

    let resp = send_rpc_request(
        sock_path.to_str().unwrap(),
        "workspace.status",
        JsonValue::Null,
    )
    .await
    .unwrap();
    assert!(resp.contains("pane_count"));
}

#[tokio::test]
async fn client_authenticates_with_token() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = "my-secret-test-token";
    tokio::spawn(serve_jsonrpc_tcp(
        listener,
        AutomationControlPlane::new_headless(24, 80),
        Some(std::sync::Arc::from(token)),
    ));

    let resp = send_rpc_request_with_token(
        &addr.to_string(),
        "workspace.status",
        JsonValue::Null,
        Some(token),
    )
    .await
    .unwrap();
    assert!(resp.contains("pane_count"), "got: {resp}");
}

#[tokio::test]
async fn client_wrong_token_is_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let token = "my-secret-test-token";
    tokio::spawn(serve_jsonrpc_tcp(
        listener,
        AutomationControlPlane::new_headless(24, 80),
        Some(std::sync::Arc::from(token)),
    ));

    let err = send_rpc_request_with_token(
        &addr.to_string(),
        "workspace.status",
        JsonValue::Null,
        Some("wrong-token-abc"),
    )
    .await
    .unwrap_err();

    let err_str = err.to_string();
    assert!(
        !err_str.contains("wrong-token-abc"),
        "leaked submitted token: {err_str}"
    );
    assert!(
        !err_str.contains("my-secret-test-token"),
        "leaked secret token: {err_str}"
    );
}

#[tokio::test]
#[cfg(unix)]
async fn client_unix_prefix_endpoint() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sock_path = temp_dir.path().join("bilterm.sock");
    let cfg = ListenConfig {
        endpoint: ListenEndpoint::Unix(sock_path.clone()),
        token: None,
    };

    tokio::spawn(run_headless_server(
        cfg,
        AutomationControlPlane::new_headless(24, 80),
    ));

    for _ in 0..40 {
        if sock_path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let p_str = sock_path.to_str().unwrap();
    let resp1 = send_rpc_request(
        &format!("unix:{p_str}"),
        "workspace.status",
        JsonValue::Null,
    )
    .await
    .unwrap();
    assert!(resp1.contains("pane_count"));

    let resp2 = send_rpc_request(
        &format!("unix://{p_str}"),
        "workspace.status",
        JsonValue::Null,
    )
    .await
    .unwrap();
    assert!(resp2.contains("pane_count"));
}
