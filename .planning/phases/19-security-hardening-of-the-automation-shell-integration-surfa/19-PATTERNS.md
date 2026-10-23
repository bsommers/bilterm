# Phase 19: Security Hardening - Pattern Map

**Mapped:** 2026-10-23
**Files analyzed:** 14 (1 new module, 13 modified/test)
**Analogs found:** 13 / 14 (new `secure_fs.rs` only partial: `private_open_options` in history_store.rs)

Decisions D-01..D-06 (19-CONTEXT.md) are locked; RESEARCH recommendations followed where not contradicted.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match |
|---|---|---|---|---|
| `src/secure_fs.rs` (NEW; add `pub mod secure_fs;` to `src/lib.rs`, alphabetical after `remote`) | utility | file-I/O | `private_open_options` in `src/pane/history_store.rs:163-172` | partial |
| `src/api/mod.rs` `serve_jsonrpc` / `serve_jsonrpc_connection` (generic stream + auth gate) | service (server) | streaming / request-response | itself (lines ~1531-1608) | exact (refactor) |
| `src/api/mod.rs` new `run_headless_*` (Unix listener, listen policy) | service | request-response | `run_headless_tcp_server` (1531-1534) + `send_rpc_request` unix arm (1491-1500) | role-match |
| `src/api/mod.rs` `send_rpc_request` (auth.login + endpoint detect) | utility (client) | request-response | itself (1467-1510) | exact |
| `src/api/mod.rs` `dispatch_method` "workspace.snapshot" (confinement) + `AutomationControlPlane` field `snapshot_dir` | controller | file-I/O | itself (719-728); struct at 483-501 | exact |
| `src/api/mod.rs` playbook `export_ppm` (1319-1327) | controller | file-I/O | itself; D-02 unconfined helper | exact |
| `src/main.rs` (args: `--headless` w/o `--listen`, `--allow-remote`, `--token-file`, `--snapshot-dir`, ctl `--token-file`, help/completions) | config/CLI | request-response | itself (lines 24-100) | exact |
| `docs/bilterm.1` + README/docs | docs | - | existing man page (`include_str!("../docs/bilterm.1")`, main.rs ~100) | exact |
| `src/pty/shell_integration.rs` (rc files) | utility | file-I/O | itself (127-174) | exact |
| `src/pane/history_store.rs` (fallback dir 230, export 465-490) | service | file-I/O | `private_open_options` + tmp `create_new` (403-406) | exact |
| `src/pane/colon_command.rs:886-891` + `src/vt/mod.rs:772-792` (`.cast`) | utility | file-I/O | history_store export | role-match |
| `src/remote/mod.rs` builders (64-117) + `validate_destination` | utility | transform | itself; tests/full_suite_battle_and_perf.rs:44-66 | exact |
| `src/wm/mod.rs` (spawn_ssh_pane 190, connect_ssh_tmux 249, session fallback 479) | service | request-response | itself | exact |
| tests: `src/api/mod.rs` `mod tests` (1610+), `src/secure_fs.rs` tests, `tests/v110_v120_v130_automation_api_mcp_playbooks.rs`, `tests/full_suite_battle_and_perf.rs` | test | - | `daemon_survives_abusive_clients...` (1634) | exact |

## Pattern Assignments

### `src/secure_fs.rs` (utility, file-I/O) - NEW
**Analog:** `src/pane/history_store.rs:163-172` (cfg(unix) + `OpenOptionsExt::mode` idiom) and `:403-406` (`create_new(true)` private write).
```rust
fn private_open_options() -> OpenOptions {
    #[allow(unused_mut)]
    let mut opts = OpenOptions::new();
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; opts.mode(0o600); }
    opts
}
// 403: private_open_options().write(true).create_new(true).open(&tmp_path)?
```
Extend with: `custom_flags(libc::O_NOFOLLOW)`; `DirBuilderExt::mode(0o700)`; `verify_private_dir` (RESEARCH Code Examples: symlink_metadata, `uid()==libc::geteuid()`, `mode()&0o077==0`). Per D-06 all fns take a base path param (`private_dir_in(base: &Path)`), env read only in thin wrappers (`private_runtime_dir()`: `$XDG_RUNTIME_DIR/bilterm` else `temp_dir()/bilterm-<uid>`). Non-unix: plain `create_dir_all` / `create_new` best effort. Keep names short (sun_path: `bilterm.sock`, `ssh-%C`).
Move/reuse `private_open_options` rather than duplicating (history_store should call `secure_fs`).
**Tests:** in-file `mod tests` using `tempfile::tempdir()` (dev-dep already present); assert mode via `metadata().permissions().mode() & 0o777`, symlink at target not followed, wrong-mode dir rejected.

### `src/api/mod.rs` generic server (streaming)
**Analog (current code to refactor), 1539-1608:**
```rust
pub async fn serve_jsonrpc(listener: tokio::net::TcpListener, mut plane: AutomationControlPlane) -> Result<()> {
    type RpcRequest = (String, tokio::sync::oneshot::Sender<String>);
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel::<RpcRequest>(64);
    loop { tokio::select! {
        accepted = listener.accept() => { match accepted { Ok((stream, peer)) => {
            let req_tx = req_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = serve_jsonrpc_connection(stream, req_tx).await {
                    tracing::warn!(%peer, error = %e, "JSON-RPC connection closed"); } }); }
            Err(e) => tracing::warn!(error = %e, "JSON-RPC accept failed"), } }
        Some((line, reply_tx)) = req_rx.recv() => { let _ = reply_tx.send(plane.handle_jsonrpc_line(&line)); }
    } }
}
async fn serve_jsonrpc_connection(stream: tokio::net::TcpStream, req_tx: ...) -> std::io::Result<()> {
    let (rd, mut wr) = stream.into_split();
    let mut reader = BufReader::new(rd);
    loop { buf.clear();
        let n = (&mut reader).take(MAX_RPC_LINE_BYTES + 1).read_until(b'\n', &mut buf).await?;
        if n == 0 { return Ok(()); }
        if buf.last() != Some(&b'\n') && n as u64 > MAX_RPC_LINE_BYTES { return Err(... "JSON-RPC request line exceeds 1 MiB"); }
        ... send (trimmed, reply_tx) to req_tx; await reply; wr.write_all(format!("{resp}\n"))...
```
**Changes:** `serve_jsonrpc_connection<S: AsyncRead+AsyncWrite+Unpin+Send+'static>(stream: S, auth: Option<Arc<str>>, ...)` with `tokio::io::split`. Keep the 1 MiB `take` limiter verbatim. Per-connection state `authed: bool` (D-01): if `auth.is_some()`, first line must parse as `{"method":"auth.login","params":{"token":..}}`; verify via a manual constant-time fold; on mismatch/other method reply JSON-RPC error (reuse `Self::error_response(id, -32001, msg).to_json_string()`, same style as `-32700` at line 513) and `return Ok(())` (close). Handle auth in the connection task (no plane access needed) so plane stays single-owner; never log the line/token. Keep `pub async fn serve_jsonrpc(TcpListener, plane)` signature compatible for existing test (1634-1637), e.g. thin wrapper over new `serve_jsonrpc_with(acceptor, plane, auth)`; accept loop peer type differs (`SocketAddr` vs unix addr) so log without `%peer` or format to String. Unix variant: after accept, `stream.peer_cred()` uid == `geteuid` (cfg(unix)).
**Unix bind (RESEARCH Pattern 2):** `secure_fs::private_runtime_dir()` -> `bilterm.sock`; stale probe via `UnixStream::connect` (ECONNREFUSED -> remove); `UnixListener::bind`; `set_permissions(0o600)`; best-effort remove on shutdown. Non-unix: loopback TCP + required token.
**Listen policy:** pure fn `validate_listen_config(endpoint, allow_remote, token_file) -> Result<ListenConfig>` (unit-testable, D-04): loopback via `SocketAddr::ip().is_loopback()`; non-loopback needs `--allow-remote` AND token file with `mode&0o077==0`, owner==euid, non-empty (>=16 bytes), trimmed. Errors use `BiltermError::Config(String)` (src/error.rs:18; already used at api/mod.rs ~1505).

### `src/api/mod.rs` `send_rpc_request` (client)
**Analog:** itself (1467-1510); endpoint sniff is `endpoint.contains(':') && !Path::new(endpoint).exists()`. Fix pitfall 9: check `strip_prefix("unix://")` first. To send token (D-01): write `auth.login` line first, read its reply, then the real request (both arms duplicate the write/read pattern; factor into one generic helper over `AsyncRead+AsyncWrite` to avoid triple copy). Add `token: Option<&str>` param (callers: main.rs ctl, tests).

### `src/api/mod.rs` `workspace.snapshot` confinement (file-I/O)
**Analog:** 719-728:
```rust
"workspace.snapshot" => {
    if let Some(ppm_path) = params.get("ppm_path").and_then(JsonValue::as_str) {
        let mut renderer = GuiWorkspaceRenderer::default();
        let mut surface = GuiFrameSurface::new(8, 16);
        renderer.render_workspace(&self.wm, self.viewport, &mut surface);
        std::fs::write(ppm_path, surface.export_ppm())
            .map_err(|e| format!("failed to write PPM snapshot: {e}"))?;
    }
    Ok(self.workspace_status_json())
}
```
Errors are `Result<_, String>` with `map_err(|e| format!(...))`. Split into `fn write_snapshot_trusted(&self, path: &Path)` (render + `fs::write`) and `fn write_snapshot_confined(&self, name: &str)`: accept bare file name only (`Path::new(name).file_name() == Some(name)`, no separators/absolute/`..`), join onto `self.snapshot_dir` (new field on `AutomationControlPlane`, struct at 483-487, initialized in `new` 490-496; default `$XDG_STATE_HOME/bilterm/snapshots` -> `~/.local/state/bilterm/snapshots`, mirror env fallback chain at `src/wm/mod.rs:465-479`; `--snapshot-dir` override; create 0700 via secure_fs), open with `create`+truncate+`O_NOFOLLOW` (or `create_new` after removing own previous file), then canonicalize parent and assert `starts_with(canonical snapshot_dir)`. Error text: "ppm_path must be a file name within the snapshot directory". The `dispatch_method` arm calls confined (covers RPC and MCP via `handle_mcp_line`, line 538).
**Playbook (1319-1327, D-02):** switch `export_ppm` arm to call `write_snapshot_trusted` directly (no `dispatch_method`), so `tests/v110_v120_v130_automation_api_mcp_playbooks.rs:146-192` (absolute tempdir, asserts `ppm_path.exists()`) passes unchanged. Verify `run_playbook` is not reachable from dispatch (grep showed it is not).

### `src/main.rs` (CLI)
**Analog:** itself. Existing arg style: `args.iter().position(|a| a == "--x")` + `args.get(pos+1)` inside `if let ... &&` let-chains (lines 60-67 `--playbook`, 69-75 headless). Current:
```rust
if args.iter().any(|a| a == "--headless")
    && let Some(pos) = args.iter().position(|a| a == "--listen")
    && let Some(bind_addr) = args.get(pos + 1)
{ run_headless_tcp_server(bind_addr, 24, 80).await?; return Ok(()); }
```
Rework: `--headless` alone -> default Unix socket; `--listen <ADDR|unix:PATH>` optional; add `--allow-remote`, `--token-file <P>` (also `BILTERM_TOKEN_FILE`), `--snapshot-dir <P>`. `ctl` (line 45-57): endpoint default `127.0.0.1:7370` -> default socket path; add `--token-file`. Update: help text (lines 28-41, "TCP/Unix socket" claim), bash/zsh/fish completions (lines 78-92, three one-line strings listing every flag), `docs/bilterm.1`, README/docs (use `keeping-docs-in-sync`). Do not put tokens in argv.

### `src/pty/shell_integration.rs` (file-I/O)
**Analog:** itself 127-174. Pattern per shell: `fs::create_dir_all(&dir).ok()?; let p = dir.join(name); fs::write(&p, SCRIPT).ok()?;` and `fn shell_integration_root_dir() -> PathBuf { std::env::temp_dir().join(format!("bilterm-shell-integration-{}", std::process::id())) }` (line 171). Replace root with `secure_fs::private_runtime_dir().join(format!("shell-integration-{pid}"))`, create subdirs 0700, replace `fs::write` with `secure_fs::write_private_file` (remove-then-`create_new`, 0600, O_NOFOLLOW). Keep `Option` return style (`.ok()?`; on failure return `None` = integration skipped, fail-closed). Existing test (178-203) only asserts `Path::new(&args[1]).exists()` and env keys - should pass unchanged.

### `src/pane/history_store.rs` (file-I/O)
**Analog:** itself. Fallback line 230 `std::env::temp_dir().join("bilterm-history")` (end of `default_history_dir`, lines ~215-230 chain XDG -> HOME -> temp) -> `secure_fs` private dir. Export (465-490): default path `temp_dir().join(format!("bilterm-commands-pane{}-{}-{}-{ts}.sh", ...))` then `create_dir_all(parent)` -> default goes into private dir via `create_private_file` (O_EXCL; custom_path stays user-intent). **Tests to update:** `headless_store_never_resolves_user_dir_and_exports_temp_file` (~line 715-730) asserts `base_dir.starts_with(std::env::temp_dir())` twice - change to assert under private fallback dir (or still under temp_dir when XDG_RUNTIME_DIR unset: make assertion base-param-driven).

### `src/pane/colon_command.rs:886-891` and `src/vt/mod.rs:772-792` (`.cast`)
**Analog:** history_store export. Default: `std::env::temp_dir().join(format!("bilterm-pane-{}.cast", pane.id))` -> private dir + ts/pid suffix (avoid EEXIST). `stop_recording` does `create_dir_all(parent)` then `std::fs::write(&path, out)`; for default path use `secure_fs::create_private_file` + `write_all`; user-supplied explicit paths keep current behavior (or at least `create_new` not required). Consider recording a flag/ensuring default path only goes through private branch.

### `src/remote/mod.rs` builders + validation
**Analog:** itself 64-117. Both builders duplicate the control-master block with `ControlPath=/tmp/bilterm-ssh-%r@%h:%p` (lines ~72, ~95) and end `args.push(self.destination.clone());`. Plan:
- `pub fn validate_destination(&str) -> Result<(), BiltermError>`-style (RemoteError does not exist; reuse `crate::error::BiltermError::Config`, thiserror enum at src/error.rs:13-25): reject empty, leading `-`, any `char::is_control`/whitespace, NUL.
- `build_ssh_args(&self) -> Result<Vec<String>>`, `build_ssh_tmux_cc_args(..) -> Result<Vec<String>>` (D-05); `args.push("--".into())` immediately before destination; tmux remote command stays last.
- ControlPath: factor shared helper `fn push_control_args(&self, args: &mut Vec<String>, control_dir: &Path)`; use `<private dir>/ssh-%C`. Builders stay pure: pass `control_dir: &Path` (or a `RemoteHostProfile` field) so unit tests need no I/O.
**Callers:** `src/wm/mod.rs:215` (`spawn_ssh_pane`, uses `?`-style `Result<PaneId>`; validate BEFORE `split_focused` at line 214 to avoid orphan pane) ; `connect_ssh_tmux` (249-275) does not call a builder but registers `dest` - add `validate_destination(&profile.destination)?` before `split_focused`; `colon_command.rs:625/1109` `RemoteAdd` should surface error via `status_message = Some(format!("... {err}"))` (existing pattern ~1100-1103). `api/mod.rs:961-1000` already maps via `.map_err(|e| e.to_string())`.
**Tests to update:** `tests/full_suite_battle_and_perf.rs:44-66` - add `.unwrap()`; existing `last() == "ubuntu@10.0.0.42"` and tmux-last assertions still true. Add: `-oProxyCommand=evil` rejected, `--` index == len-2 (ssh) / len-3 (tmux), control chars rejected.

### `src/wm/mod.rs:465-479` session fallback
**Analog:** itself - `resolve_session_dir` chain config -> `XDG_STATE_HOME` -> `HOME/.local/state` -> `std::env::temp_dir().join("bilterm-sessions")`. Replace last arm with secure_fs private dir (fn returns `PathBuf`; fall back to temp_dir on error is NOT acceptable - propagate or log + skip). Reuse this exact XDG->HOME chain for the snapshot dir default.

## Shared Patterns

### cfg(unix) gating
**Source:** `src/pane/history_store.rs:166-170` (`#[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; ... }`) and `send_rpc_request` `#[cfg(unix)]`/`#[cfg(not(unix))]` arms (api/mod.rs ~1491-1507, non-unix returns `BiltermError::Config("Use host:port TCP endpoint on non-Unix platforms")`). **Apply to:** secure_fs, Unix listener, peer_cred, ownership checks.

### Error type / JSON-RPC errors
**Source:** `src/error.rs:13-25` (`BiltermError::{Io, Config, ...}`, `Result<T>` alias, thiserror); dispatch methods return `Result<JsonValue, String>` with `.map_err(|e| format!("...: {e}"))`; JSON-RPC error via `Self::error_response(id, code, msg)`. **Apply to:** all new fallible fns (use `io::Result` inside secure_fs; `BiltermError::Config` for CLI policy; `String` inside dispatch).

### Test conventions
**Source:** `src/api/mod.rs` `mod tests` (imports `tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}`; `status_roundtrip(addr)` helper at ~1620; `#[tokio::test]` binding `127.0.0.1:0`, `tokio::spawn(serve_jsonrpc(listener, AutomationControlPlane::new_headless(24, 80)))`). Integration tests use `tempfile::tempdir()` + `new_headless`. No env mutation (edition 2024 `set_var` unsafe): inject base paths. Unix-socket tests: bind in `tempdir()` path (short!). **Gate:** `cargo test` (127 baseline) + `cargo clippy --all-targets -- -D warnings`; revert `docs/assets/screenshot-3278-inspector.svg` rewritten by tests.

### Token handling
No existing analog (no auth code in repo). Use RESEARCH Pattern 2 + D-01; manual constant-time byte-fold compare; token never in argv/logs/errors.

## No Analog Found
| File | Role | Data Flow | Reason |
|---|---|---|---|
| bearer auth gate / `auth.login` in `serve_jsonrpc_connection` | middleware | request-response | no authentication code exists anywhere; use RESEARCH Pattern 2 |
| server-side `UnixListener` | service | streaming | only client `UnixStream::connect` exists (api/mod.rs ~1491) |
| ownership/mode verification (`geteuid`, `mode & 0o077`) | utility | file-I/O | `libc` is a dep but unused for this; use RESEARCH Code Examples |

## Metadata
**Analog search scope:** src/api, src/main.rs, src/pty, src/pane, src/remote, src/wm, src/vt, src/error.rs, tests/
**Files scanned:** ~14 (targeted line-range reads)
**Pattern extraction date:** 2026-10-23
