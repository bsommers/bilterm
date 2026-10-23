---
phase: 19-security-hardening-of-the-automation-shell-integration-surfa
plan: 04
subsystem: api-server
tags: [rust, security, jsonrpc, unix-socket, auth-login, bearer-token, headless-server]

requires: [19-01]
provides:
  - "ListenEndpoint and ListenConfig types with redacted Debug preventing token leaks"
  - "Pure validate_listen_config enforcing loopback TCP, Unix socket defaults, and --allow-remote with token"
  - "load_token_file enforcing private 0600 file permissions via secure_fs"
  - "default_socket_path resolving <private_runtime_dir>/bilterm.sock with path length boundary checks"
  - "Generic serve_jsonrpc_connection over any AsyncRead + AsyncWrite + Unpin stream"
  - "Per-connection auth.login handshake gate enforcing bearer token check before control plane dispatch"
  - "Unix domain socket server with 0600 permissions, active daemon collision detection, stale socket cleanup, and peer UID verification"
  - "send_rpc_request_with_token supporting bearer authentication and unix: / unix:// endpoint prefixes"
affects: [19-05, 19-06, 19-07]

tech-stack:
  added: []
  patterns: ["RpcAcceptor trait for unified TCP and Unix socket dispatch", "Drop-guarded Unix socket unlinking", "Constant-time bearer token validation"]

key-files:
  created: []
  modified: [src/api/mod.rs]

key-decisions:
  - "Default headless daemon endpoint defaults to Unix domain socket under private runtime dir (or loopback TCP with token on non-Unix)"
  - "Non-loopback TCP listeners strictly prohibited unless --allow-remote is passed AND a bearer token is provided"
  - "Unauthenticated requests on token-protected listeners receive -32001 and connection is terminated immediately"
  - "Token-less servers accept auth.login transparently to maintain compatibility with configured ctl clients"
  - "Tokens are never emitted in logs, error responses, or Debug representation of ListenConfig"

patterns-established:
  - "All automation control plane streaming listeners use RpcAcceptor and serve_loop with per-connection auth gate"

requirements-completed: [SEC-R01]

duration: 10min
completed: 2026-10-23
status: complete
---

# Phase 19 Plan 04: Daemon Listen Policy & Auth Gate Summary

**Hardened the headless JSON-RPC automation daemon and client (SEC-R01): default 0600 Unix domain socket with peer UID validation, strict loopback / remote listen policy with mandatory bearer token, per-connection `auth.login` authentication gate, and token-aware client.**

## Performance & Verification

- **Tests added:** 12 comprehensive unit and integration tests in `src/api/mod.rs`:
  - `listen_policy_default_is_unix_socket`
  - `listen_policy_loopback_tcp_without_token_ok`
  - `listen_policy_non_loopback_requires_allow_remote`
  - `listen_policy_allow_remote_requires_token`
  - `listen_policy_unix_prefixes`
  - `listen_policy_rejects_garbage`
  - `listen_config_debug_redacts_token`
  - `default_socket_path_rejects_overlong_path`
  - `load_token_file_enforces_private_mode`
  - `bearer_token_gate_rejects_unauthenticated_first_request`
  - `bearer_token_gate_rejects_wrong_token`
  - `bearer_token_gate_accepts_correct_token`
  - `tokenless_server_accepts_auth_login`
  - `unix_socket_server_is_0600_and_roundtrips`
  - `unix_socket_stale_file_is_replaced`
  - `unix_socket_live_daemon_is_not_stolen`
  - `client_authenticates_with_token`
  - `client_wrong_token_is_error`
  - `client_unix_prefix_endpoint`
- **Suite status:** 100% tests passing across the entire workspace (127 unit tests + 28 integration tests); `cargo clippy --all-targets -- -D warnings` 0 warnings.
