//! Edge cases for `api/server.rs`: listen-address validation and the token-gated JSON-RPC handshake.

use std::sync::Arc;
use std::time::Duration;

use bilterm::api::{
    serve_jsonrpc_tcp, validate_listen_config, AutomationControlPlane, ListenEndpoint,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "edge-token-0123456789";

async fn start(token: Option<&str>) -> std::net::SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let plane = AutomationControlPlane::new_headless(24, 80);
    tokio::spawn(serve_jsonrpc_tcp(l, plane, token.map(Arc::from)));
    addr
}

/// Sends each line and returns the replies read until the server closes or goes quiet.
async fn converse(addr: std::net::SocketAddr, lines: &[&str]) -> Vec<String> {
    let s = TcpStream::connect(addr).await.unwrap();
    let (rd, mut wr) = tokio::io::split(s);
    let mut rd = BufReader::new(rd);
    let mut out = Vec::new();
    for l in lines {
        if wr.write_all(format!("{l}\n").as_bytes()).await.is_err() {
            break;
        }
        let mut reply = String::new();
        match tokio::time::timeout(Duration::from_secs(5), rd.read_line(&mut reply)).await {
            Ok(Ok(n)) if n > 0 => out.push(reply),
            _ => break,
        }
    }
    out
}

#[test]
fn listen_config_rejects_bad_localhost_port() {
    let err = validate_listen_config(Some("localhost:notaport"), false, None, None).unwrap_err();
    assert!(err.to_string().contains("invalid port"), "{err}");
}

#[test]
fn listen_config_localhost_port_maps_to_loopback() {
    let cfg = validate_listen_config(Some("localhost:4321"), false, None, None).unwrap();
    match cfg.endpoint {
        ListenEndpoint::Tcp(a) => assert_eq!(a.to_string(), "127.0.0.1:4321"),
        #[allow(unreachable_patterns)]
        other => panic!("unexpected endpoint {other:?}"),
    }
}

#[test]
fn listen_config_remote_requires_flag_and_token() {
    let err = validate_listen_config(Some("0.0.0.0:9"), false, None, None).unwrap_err();
    assert!(err.to_string().contains("--allow-remote"), "{err}");
    let err = validate_listen_config(Some("0.0.0.0:9"), true, None, None).unwrap_err();
    assert!(err.to_string().contains("bearer token"), "{err}");
    assert!(validate_listen_config(Some("0.0.0.0:9"), true, Some("t".into()), None).is_ok());
}

#[tokio::test]
async fn garbage_before_auth_gets_auth_error_and_close() {
    let addr = start(Some(TOKEN)).await;
    let replies = converse(addr, &["{not json", r#"{"jsonrpc":"2.0","id":1,"method":"workspace.status"}"#]).await;
    assert_eq!(replies.len(), 1, "connection must close after the first rejection: {replies:?}");
    assert!(replies[0].contains("authentication required"), "{}", replies[0]);
}

#[tokio::test]
async fn login_without_token_param_is_rejected() {
    let addr = start(Some(TOKEN)).await;
    let replies = converse(addr, &[r#"{"jsonrpc":"2.0","id":1,"method":"auth.login","params":{}}"#]).await;
    assert_eq!(replies.len(), 1);
    assert!(replies[0].contains("error"), "{}", replies[0]);
    assert!(!replies[0].contains("authenticated"), "{}", replies[0]);
}

#[tokio::test]
async fn correct_login_then_rpc_succeeds() {
    let addr = start(Some(TOKEN)).await;
    let login = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"auth.login","params":{{"token":"{TOKEN}"}}}}"#);
    let replies = converse(
        addr,
        &[&login, r#"{"jsonrpc":"2.0","id":2,"method":"workspace.status"}"#],
    )
    .await;
    assert!(replies[0].contains("\"authenticated\":true"), "{}", replies[0]);
    assert!(replies.last().unwrap().contains("\"id\":2"), "{replies:?}");
}

#[tokio::test]
async fn login_without_server_token_reports_not_required() {
    let addr = start(None).await;
    let replies = converse(addr, &[r#"{"jsonrpc":"2.0","id":1,"method":"auth.login","params":{}}"#]).await;
    assert!(replies[0].contains("\"required\":false"), "{}", replies[0]);
}
