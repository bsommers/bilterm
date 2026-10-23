# Phase 19: Security Hardening of the Automation & Shell-Integration Surfaces - Research

**Researched:** 2026-10-23
**Domain:** Rust local-IPC auth, private-temp-dir handling, path confinement, ssh argv injection
**Confidence:** HIGH (code facts verified in repo); MEDIUM on API details of std/tempfile (training knowledge, `[ASSUMED]` where noted)

## Summary

All four requirements are small, local changes in existing files. Nothing needs a new runtime dependency except promoting `tempfile` (already a dev-dependency, 3.19.1 declared; 3.27.0 in cargo registry) to a regular dependency IF the planner picks `tempfile::Builder` for the fallback dir. `libc` 0.2 is already a regular dependency, so `geteuid`/`chmod` ownership checks need no new crate. [VERIFIED: Cargo.toml]

No CONTEXT.md exists; scope comes from CHECKPOINT §2, ROADMAP Phase 19 and review §5.1. `SEC-R01..04` are NOT defined in `.planning/REQUIREMENTS.md` (grep found no match; only ROADMAP and the review reference them). Planner should add them there, mapping R01=listener/auth, R02=ppm_path, R03=private dir, R04=ssh args (the mapping is inferred from success-criteria order, `[ASSUMED]`).

Key structural fact: `serve_jsonrpc` and `serve_jsonrpc_connection` are hard-typed to `tokio::net::TcpListener`/`TcpStream` (src/api/mod.rs:1539,1568), and no Unix listener exists server-side (only the `ctl` client in `send_rpc_request` handles `unix://`/path, line 1467-1495). The `--help` text claims "TCP/Unix socket" but only TCP works. Making the connection handler generic over `AsyncRead + AsyncWrite + Unpin + Send` (via `tokio::io::split`) is the core refactor.

**Primary recommendation:** (1) one new small module `src/secure_fs.rs` (private runtime dir + O_EXCL private writes + ownership check) used by shell_integration, history export, `.cast`, and ssh ControlPath; (2) make the RPC server generic over stream type, add `UnixListener` default, auth gate + `--allow-remote`/`--token-file` in main.rs; (3) confine `ppm_path` in `dispatch_method` only for the network RPC path, keeping the trusted playbook path working; (4) validate+`--` in both ssh builders.

## Project Constraints (from AGENTS.md)
- Rust 2024, MSRV 1.88; Linux first, macOS second, Windows third: all Unix-only code under `#[cfg(unix)]` with a non-Unix fallback that still compiles (`libc` and `std::os::unix` are cfg-gated already in history_store.rs `private_open_options`).
- Use `tui`-independent code here; no reedline etc. (irrelevant).
- Verification gate: `cargo test` and `cargo clippy --all-targets -- -D warnings` fresh before any completion claim.
- TDD preferred: failing tests first (insta/TestBackend for UI; plain `#[test]`/`#[tokio::test]` here).
- GSD workflow: edits through GSD commands; update `.planning/CHECKPOINT.md`/`STATE.md` at end.
- tracing logs to file only (never print to stdout/stderr in TUI mode); headless daemon may print.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SEC-R01 | Headless listener: 0600 Unix socket default under `$XDG_RUNTIME_DIR`; non-loopback TCP needs `--allow-remote` + bearer token from 0600 file | Generic stream refactor, `UnixListener` bind + `set_permissions`, token gate, main.rs arg parsing (see Patterns 1-2) |
| SEC-R02 | `workspace.snapshot` `ppm_path` confined to output dir; traversal rejected by test | Pattern 3; playbook caller at api/mod.rs:1325 must stay working |
| SEC-R03 | rc files, history exports, `.cast`, ssh ControlPath in per-user 0700 dir; ownership check; O_EXCL | `secure_fs` module (Pattern 4); 5 call sites enumerated below |
| SEC-R04 | ssh builders reject `-` destinations, emit `--` | Pattern 5; 2 builders, 3 call-site entry points |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Listener bind, perms, token check | `api` (server) | `main.rs` CLI parsing | Transport/auth belongs at the accept loop, before a request reaches `AutomationControlPlane` |
| ppm_path confinement | `api::dispatch_method` | config (output dir) | Single choke point used by RPC, MCP and playbook |
| Private dir / O_EXCL helper | new shared util (`secure_fs`) | pty, pane, remote callers | One implementation, many callers; avoids drift |
| ssh arg validation | `remote` | `wm`, `api`, `colon_command` (surface errors) | Builder is the last line of defence; callers should also surface a clean error |

## Standard Stack

### Core (no new third-party crates required)
| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| std (`OpenOptions` + `OpenOptionsExt::mode`, `create_new`, `DirBuilderExt::mode`, `MetadataExt::uid`, `PermissionsExt`) | 1.88 | 0700 dirs, 0600 O_EXCL files, ownership/mode checks | Zero deps; `create_new(true)` = O_CREAT\|O_EXCL; avoids following symlink at final component |
| `tokio::net::UnixListener` | tokio 1.44 (`full`) [VERIFIED: Cargo.toml features full] | Unix socket server | Already available; client side already uses `UnixStream` |
| `libc` | 0.2.170 [VERIFIED: Cargo.toml] | `geteuid()`, `umask` | Already a regular dep |

### Supporting
| Library | Version | Purpose | When |
|---------|---------|---------|------|
| `tempfile` | 3.19.1 declared (dev-dep today); promote to `[dependencies]` | `Builder::new().prefix().tempdir()` creates dir mode 0700 with random name, fallback when `$XDG_RUNTIME_DIR` unset (macOS) | Only if planner wants random-name fallback; otherwise hand-roll `~/.cache`/`create_dir` with mode 0700 + ownership check |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| bearer token compare via `==` | constant-time compare (`subtle` crate) | Token is local-file secret, network timing attacks on a line protocol are low-risk; a ~6-line manual constant-time XOR fold avoids a new dep. Recommend manual fold. `[ASSUMED]` acceptable risk |
| `tempfile` fallback | `$XDG_RUNTIME_DIR` else `$TMPDIR/bilterm-<uid>` created 0700 + ownership verify | Latter needs no dep and gives a stable path (needed for stable socket/ControlPath); tempfile gives random unstable names. Recommend: stable `bilterm-<uid>` dir with verify; use `tempfile` only in tests |

**Installation:** none required (optionally move `tempfile` to `[dependencies]`). No new packages => Package Legitimacy Audit: none (no externally new crates; `tempfile`, `libc`, `tokio` already in Cargo.toml and lockfile).

## Architecture Patterns

### Data flow
```
CLI (main.rs) --parse--> ListenConfig{ endpoint: Unix(path)|Tcp(addr), allow_remote, token }
   |                                   |
   | default: unix                     v
   |                      secure_fs::runtime_dir()  (0700, owner==euid)  --> bilterm.sock (chmod 0600)
   v
serve_jsonrpc<L: Listener>(listener, plane, auth: Option<Token>)
   accept -> split stream -> [auth gate: first line must be auth / each request carries token]
          -> req_tx (mpsc) -> plane.handle_jsonrpc_line -> dispatch_method
                                        |-- workspace.snapshot -> confine(ppm_path, out_dir) -> write
                                        |-- remote.* / ssh -> RemoteHostProfile.build_ssh_args (validate, `--`)
```

### Pattern 1: Generic server over stream type
Change `serve_jsonrpc_connection(stream: TcpStream, ...)` to `<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(stream: S, ...)` using `tokio::io::split(stream)` (replaces `into_split`). Make `serve_jsonrpc` take an enum or a small trait/closure `accept()`; simplest: two thin functions `serve_jsonrpc_tcp` / `serve_jsonrpc_unix` sharing a private `serve_loop` that takes an `accept` async closure, or generic `trait Acceptor { async fn accept(&self) -> io::Result<Stream> }`. Existing test `daemon_survives_abusive_clients...` calls `serve_jsonrpc(listener: TcpListener, plane)`; keep that signature working (or update the single test + the one `tests/v110_...` use if it calls it; grep found `run_headless_tcp_server` only in main.rs and api tests).

### Pattern 2: Unix socket 0600 without race
```rust
// Source: std docs / standard practice [ASSUMED API details, verify at implementation]
let dir = secure_fs::private_runtime_dir()?;          // 0700, owner check
let path = dir.join("bilterm.sock");
if path.exists() { /* connect-probe; if stale (ECONNREFUSED) remove; if live -> error */ }
// Prevent a window where socket is 0666: set umask(0o177) around bind, or rely on the 0700 parent dir.
let listener = tokio::net::UnixListener::bind(&path)?;
std::fs::set_permissions(&path, Permissions::from_mode(0o600))?;
```
Since the parent is 0700 owned by us, the umask window is moot, but still chmod 0600 (criterion says the socket is 0600; test via `metadata().permissions().mode() & 0o777 == 0o600`). Also best-effort `remove_file` on shutdown; handle stale socket (second start otherwise fails EADDRINUSE).

Peer check: optionally `stream.peer_cred()` (tokio `UnixStream::peer_cred`) compare uid with `geteuid`; cheap defense-in-depth.

TCP policy: parse addr with `SocketAddr`/resolve; `ip.is_loopback()` -> allowed without token (per criterion only non-loopback requires token; recommend still permitting token on loopback). Non-loopback without `--allow-remote` -> hard error exit. Non-loopback with `--allow-remote` and no `--token-file` -> error. Token file: `--token-file <PATH>` (and/or `$XDG_RUNTIME_DIR/bilterm.token` default); verify `mode & 0o077 == 0` and owner == euid, else refuse (like ssh private keys); trim trailing newline; reject empty/short (<16 bytes recommended). Protocol for bearer on a line-JSON-RPC stream: no HTTP headers exist, so define it: per-request `"auth":"<token>"` field or `params._auth`, OR a first-line handshake `{"method":"auth","params":{"token":..}}` then connection authenticated. Recommend handshake method `auth.login` (connection-scoped state kept in `serve_jsonrpc_connection`, before forwarding to plane; unauthenticated connections get JSON-RPC error `-32001` and are closed after N failures). Update `send_rpc_request` / `bilterm ctl` to send token (`--token-file` or `BILTERM_TOKEN_FILE`). This is a protocol decision -> see Open Questions.

Breaking change: `ctl` default endpoint is `127.0.0.1:7370` (main.rs:~47); with Unix default, change to the socket path default, keep explicit TCP working. `--headless --listen <ADDR>` currently requires an ADDR argument; new behavior: `--headless` alone (or `--listen unix:PATH`/`--listen` omitted) uses the default socket. Keep `--listen 127.0.0.1:PORT` valid. Update `--help`, completions (3 shells at main.rs ~86-92), man page (`--man`), docs/README (use `keeping-docs-in-sync`).

### Pattern 3: ppm_path confinement
In `dispatch_method` "workspace.snapshot": reject absolute paths and any component equal to `..`; join onto an output dir (default `private_runtime_dir()/snapshots` or `$XDG_STATE_HOME/bilterm/snapshots`; configurable via `AutomationControlPlane` field `snapshot_dir`); create it 0700; open with `create_new`/or truncate-existing-regular-file but refuse symlinks (use `OpenOptions::custom_flags(libc::O_NOFOLLOW)` on unix); then after join, canonicalize the PARENT and assert `starts_with(canonical_out_dir)` (catches symlinks planted inside out_dir). Prefer accepting a bare file name only (`Path::file_name() == path`), simplest and strict; return JSON-RPC error text "ppm_path must be a file name within the snapshot directory". Test: traversal (`../x.ppm`, `/tmp/x.ppm`, `a/../../x.ppm`, symlink-in-dir) all rejected and no file created outside; valid name succeeds.
CAUTION: the playbook `export_ppm` step (api/mod.rs:1319-1327) calls `dispatch_method("workspace.snapshot", ppm_path=<arbitrary absolute tempdir path>)` and the existing test `tests/v110_v120_v130_automation_api_mcp_playbooks.rs:146-192` asserts `ppm_path.exists()` for an absolute tempdir path. Playbooks are local, user-authored files (trusted, run via `--playbook`), but they could also be reached by an RPC method if one exists that runs playbooks (check `run_playbook` exposure over RPC/MCP; grep did not show it in dispatch but verify). Recommended: split into `write_snapshot_confined()` (RPC/MCP path) and keep playbook calling an unconfined internal helper, OR add `AutomationControlPlane::snapshot_dir` and have the test configure it to its tempdir and use a relative name. Decision needed (Open Questions). The MCP tool path (`handle_mcp_line` -> dispatch_method) is also remote-reachable via stdio only; confine it too.

### Pattern 4: `secure_fs` module (new file `src/secure_fs.rs`, register in lib.rs)
Functions:
- `private_runtime_dir() -> io::Result<PathBuf>`: base = `$XDG_RUNTIME_DIR` (must be absolute, exist, owned by euid) joined `bilterm`; else fallback `std::env::temp_dir().join(format!("bilterm-{uid}"))`. Create with `DirBuilder::mode(0o700)`; if already exists: `symlink_metadata` -> must be a real dir (not symlink), `uid == geteuid()`, `mode & 0o077 == 0`, else error (do NOT chmod someone else's dir). Non-unix: `temp_dir().join("bilterm")` best effort.
- `create_private_file(path) -> io::Result<File>`: `OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(O_NOFOLLOW)`.
- `write_private_file(dir, name, bytes)`: for rc files, unique per-pid/per-pane subdir; if file exists from a previous run by us, `remove_file` then `create_new` (still O_EXCL, avoids following a swapped symlink).
Call sites (all verified):
1. src/pty/shell_integration.rs:127-174 (`shell_integration_root_dir`, `create_dir_all` + `fs::write` in bash/zsh/fish arms) -> use private dir + per-pid subdir + `create_private_file`. Existing unit tests in that file's `mod tests` (check what they assert on path prefix `bilterm-shell-integration-`; tests may need updating).
2. src/pane/history_store.rs:171 area `default_history_dir` fallback `temp_dir().join("bilterm-history")` (line 230) and export path at ~476 `temp_dir().join("bilterm-commands-pane...sh")` followed by `create_dir_all(parent)` + write; note `private_open_options()` already gives 0600 but not O_EXCL/no-follow and the parent is shared /tmp. Tests at history_store.rs:715-730 assert `base_dir.starts_with(std::env::temp_dir())` — will break/need updating when fallback moves.
3. src/pane/colon_command.rs:889-891 `:record` default path `temp_dir()/bilterm-pane-<id>.cast`; actual file written later in `vt/mod.rs::stop_recording` (line ~772) via plain write — must switch to private-create (O_EXCL; add timestamp/pid suffix to avoid EEXIST on repeat). User-supplied explicit paths stay as-is (user's own intent).
4. src/remote/mod.rs:72,95 `ControlPath=/tmp/bilterm-ssh-%r@%h:%p`. Use the private dir: `ControlPath=<private_dir>/ssh-%C` (`%C` = hash of %l%h%p%r, keeps the path under the ~104-byte sun_path limit; `%r@%h:%p` can overflow with long hostnames) `[ASSUMED: %C available in OpenSSH >= 6.7]`. Builders are pure `&self` methods returning Vec<String> and have a unit test (tests/full_suite_battle_and_perf.rs:44-66) — computing the dir inside the builder introduces I/O; better pass control-dir in (new param or a field) or compute lazily with fallback; keep the existing assertion `contains("ControlMaster=auto")` working.
5. src/wm/mod.rs:479 `temp_dir().join("bilterm-sessions")` session-save fallback (not in the review list but same class; include for consistency).
Sockets/ControlPath macOS note: `$XDG_RUNTIME_DIR` is normally unset on macOS and Linux without systemd-logind -> fallback path must work; the macOS `$TMPDIR` is already per-user 0700 (/var/folders/..) but long, making the ControlPath/unix socket exceed 104 bytes: keep file names short (`ssh-%C`, `bilterm.sock`).

### Pattern 5: ssh destination hardening
Add `pub fn validate_destination(&str) -> Result<(), RemoteError>` rejecting: empty, leading `-`, whitespace/control chars (also newline), maybe NUL. Make both builders emit `args.push("--")` immediately before destination. Builders currently return `Vec<String>` (infallible) — to reject, either change to `Result<Vec<String>, _>` (ripple: wm/mod.rs:215, tests/full_suite_battle_and_perf.rs:44-66 and the tmux variant) or validate at construction/entry points (`RemoteHostProfile::new`, api/mod.rs:1035 `remote.add`-style RPC, colon_command.rs:625/1109, wm connect) AND have builders defensively emit `--`. Recommend: `Result` return on builders (spec says builders "reject"), update the 3-4 callers. Also validate `identity_file` doesn't start with `-` is unnecessary since it follows `-i`. The `tmux -CC new-session -A -s {sess}` remote command is passed as the trailing arg after destination — already sanitized to `[A-Za-z0-9_-]`; with `--` placed before destination, the remote command stays after it (valid: `ssh [opts] -- dest cmd`). Existing test asserts `ssh_args.last() == "ubuntu@10.0.0.42"` for plain ssh (still true: `--` goes before). Add tests: `-oProxyCommand=evil` rejected, `--` position, tmux variant.

### Anti-Patterns to Avoid
- `chmod` after create for secrets (race): create with mode via `OpenOptionsExt::mode`.
- `exists()` then create (TOCTOU): use `create_new`.
- Accepting a pre-existing runtime dir without owner/mode/symlink check.
- Constant `/tmp` fallbacks for socket/token paths.
- Putting token on the command line (visible in `ps`): token comes from file only (spec-compliant).

## Don't Hand-Roll
| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Atomic exclusive create | exists()+write | `OpenOptions::create_new` + `mode(0o600)` | race-free |
| Random private dir | own RNG naming | `tempfile::Builder` (tests; optional prod) | handles collisions, 0700 |
| Unix peer identity | parsing /proc | `UnixStream::peer_cred()` | built into tokio |
| Path normalization for confinement | string-contains("..") | `Path::components()` check + canonicalize parent | handles `a/../..`, symlinks |
| SSH quoting | shell-quoting destination | validate + `--` (args are argv, no shell involved) | portable-pty takes argv |

## Common Pitfalls
1. **Stale socket after crash:** bind fails with AddrInUse. Probe by connecting; remove only if refused.
2. **sun_path limit (108 Linux/104 macOS):** `$XDG_RUNTIME_DIR/bilterm/bilterm.sock` is fine; long fallback paths on macOS are not. Test with a long TMPDIR.
3. **Serve generic refactor breaks `into_split`:** `tokio::io::split` returns different half types; keep the 1 MiB `take` limiter logic intact (guards abusive-client test).
4. **Existing tests assert /tmp prefixes** (history_store.rs:715-730, maybe shell_integration tests): update intentionally.
5. **Playbook ppm path conflict** (see Pattern 3).
6. **Parallel test interference:** tests that set `XDG_RUNTIME_DIR` via `std::env::set_var` are `unsafe` in edition 2024 and racy across threads; instead inject the base dir as a function parameter (`private_dir_in(base: &Path)`) and unit-test that with `tempfile::tempdir()`.
7. **Windows:** `std::os::unix` and `UnixListener` absent: gate with `#[cfg(unix)]`; on non-unix, `--headless` default must fall back to loopback TCP (127.0.0.1:7370) with a token; keep crate compiling (can't CI-verify here; add `cargo check --target x86_64-pc-windows-gnu` only if toolchain installed).
8. **Token compare / length:** do not log the token; do not echo it in errors; tracing must not record request lines on auth failure.
9. **`ctl` with TCP detection:** `send_rpc_request` decides TCP vs Unix via `endpoint.contains(':') && !Path::exists` — a Unix path containing `:` that doesn't exist yet is misrouted; accept explicit `unix://` prefix check first.
10. `ghostty-vt` feature not relevant.

## Code Examples

```rust
// 0600 O_EXCL no-follow create [ASSUMED API; std stable]
#[cfg(unix)]
pub fn create_private_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new().write(true).create_new(true).mode(0o600)
        .custom_flags(libc::O_NOFOLLOW).open(path)
}

// owner + mode verification of an existing dir
#[cfg(unix)]
fn verify_private_dir(p: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let md = fs::symlink_metadata(p)?;
    if !md.is_dir() || md.uid() != unsafe { libc::geteuid() } || md.mode() & 0o077 != 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "unsafe runtime dir"));
    }
    Ok(())
}
```

## State of the Art
Not applicable (stable POSIX idioms). `std::os::unix::net::UnixListener`/tokio equivalents unchanged. OpenSSH `%C` token for ControlPath preferred over `%r@%h:%p` for length.

## Assumptions Log
| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | SEC-R01..04 map in review order (R01 listener, R02 ppm, R03 temp files, R04 ssh) | Summary | Only labeling |
| A2 | Handshake method `auth.login` per-connection is acceptable bearer design for line-JSON-RPC | Pattern 2 | User may want per-request field; affects ctl client |
| A3 | `%C` ControlPath token available on supported OpenSSH (>=6.7) | Pattern 4 | Old ssh fails to expand; fallback `%r@%h-%p` |
| A4 | Manual constant-time compare adequate (no `subtle` dep) | Alternatives | Low |
| A5 | std/tokio API details (`peer_cred`, `custom_flags`, `DirBuilderExt`) as described | Patterns | Compile-time discoverable |

## Open Questions (RESOLVED)
1. **Token wire protocol.** Handshake vs per-request field? Recommendation: connection-level `auth.login`, required whenever a token is configured (always for non-loopback TCP; optional for loopback; Unix socket relies on perms + peer_cred). **RESOLVED → D-01** (auth.login handshake).
2. **Playbook `export_ppm` vs confinement.** Recommend: playbooks use a trusted unconfined helper; RPC/MCP confined. If playbooks reachable via RPC, confine them too and update test to relative name + configured snapshot dir. **RESOLVED → D-02** (CLI `--playbook` unconfined; RPC/MCP-submitted playbooks confined).
3. **Builders: `Result` vs infallible + validate at entry.** Recommend `Result`. **RESOLVED → D-05** (`Result`).
4. **Default snapshot dir location** (`<runtime>/bilterm/snapshots` vs `$XDG_STATE_HOME`). Runtime dir is tmpfs and cleared on logout; acceptable for snapshots; configurable via `--snapshot-dir`? Planner/user to confirm. **RESOLVED → D-03** (`$XDG_STATE_HOME/bilterm/snapshots` + `--snapshot-dir`; bare file names only).
5. **Loopback TCP without token** stays allowed (per criterion) - confirm no desire to require token everywhere. **RESOLVED → D-04** (token optional on loopback).

## Environment Availability
| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo/rustc | build | yes | edition 2024 project builds (127 tests baseline per CHECKPOINT) | - |
| `$XDG_RUNTIME_DIR` | default socket dir | yes on this host (/run/user/1000) | - | `temp_dir()/bilterm-<uid>` with verify |
| ssh client | not needed (argv tests only) | n/a | - | - |
| Windows/macOS toolchains | cross-platform check | not probed | - | rely on cfg gates + review |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]`; `insta` 1.42 snapshots (UI only); `tempfile` dev-dep |
| Config file | none (Cargo.toml); nextest optional |
| Quick run command | `cargo test --lib api:: remote:: secure_fs:: shell_integration:: history_store::` (per module filters) |
| Full suite command | `cargo test && cargo clippy --all-targets -- -D warnings` |

Existing patterns: in-file `mod tests` in `src/api/mod.rs` (TCP server tests using `127.0.0.1:0` + raw `TcpStream`, abusive-client test), integration tests in `tests/*.rs` (e.g. `tests/v110_v120_v130_automation_api_mcp_playbooks.rs` uses `tempfile::tempdir()` and `AutomationControlPlane::new_headless`; `tests/full_suite_battle_and_perf.rs` covers ssh args).

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SEC-R01 | Unix socket created with mode 0600 in 0700 dir; works end-to-end with `send_rpc_request` | tokio integration | `cargo test --lib api::tests::unix_socket` | Wave 0 (new test in src/api/mod.rs) |
| SEC-R01 | Non-loopback bind w/o `--allow-remote` refused; with it but no token refused; token file with 0644 refused | unit (pure fn `validate_listen_config`) | `cargo test --lib api::tests::listen_policy` | Wave 0 |
| SEC-R01 | Unauthenticated request rejected, authed accepted (TCP loopback + token) | tokio | `cargo test --lib api::tests::bearer` | Wave 0 |
| SEC-R01 | Existing abusive-client test still passes after generic refactor | tokio | `cargo test --lib daemon_survives` | yes |
| SEC-R02 | `../`, absolute, nested-`..`, symlink-escape rejected; plain name writes in snapshot dir | unit | `cargo test --lib api::tests::snapshot_confine` | Wave 0 |
| SEC-R02 | Playbook export_ppm still works | integration | `cargo test --test v110_v120_v130_automation_api_mcp_playbooks` | yes (may need edit) |
| SEC-R03 | `private_dir_in(base)` creates 0700; rejects wrong mode/symlink/foreign owner (owner test only if chown possible else mode+symlink) | unit | `cargo test --lib secure_fs` | Wave 0 |
| SEC-R03 | O_EXCL: pre-planted symlink/file at target not followed/overwritten | unit | `cargo test --lib secure_fs::excl` | Wave 0 |
| SEC-R03 | rc files land under private dir with 0600; `.cast`/export default path; ControlPath not under `/tmp` literal | unit | `cargo test --lib shell_integration history_store remote` | partly (update old asserts) |
| SEC-R04 | `-oProxyCommand=x` rejected; `--` precedes destination in both builders | unit | `cargo test remote` / `--test full_suite_battle_and_perf` | partly |

### Sampling Rate
- Per task commit: module-filtered `cargo test --lib <module>`
- Per wave merge: `cargo test`
- Phase gate: `cargo test` + `cargo clippy --all-targets -- -D warnings` green (baseline 127 tests)

### Wave 0 Gaps
- [ ] `src/secure_fs.rs` + unit tests (private dir helpers parameterized by base path, not env)
- [ ] test helpers for Unix-socket serve/connect in api tests
- [ ] update tests asserting `/tmp` prefixes (history_store.rs ~715-730) and ControlPath string

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | Bearer token from 0600 file, constant-time compare |
| V3 Session Management | minor | Connection-scoped auth state |
| V4 Access Control | yes | Socket 0600 + dir 0700 + `peer_cred` uid match; remote opt-in |
| V5 Input Validation | yes | ssh destination validation, ppm path component check |
| V6 Cryptography | no | Token is a shared secret, no custom crypto; TCP is plaintext -> document "use ssh tunnel/loopback" for remote |
| V12 Files | yes | O_EXCL, O_NOFOLLOW, ownership checks |

### Known Threat Patterns
| Pattern | STRIDE | Mitigation |
|---------|--------|------------|
| Predictable /tmp path pre-creation (symlink/dir squat) -> code exec via sourced rc | Tampering/EoP | per-user 0700 dir, owner verify, O_EXCL |
| Unauth network RPC -> arbitrary file write | EoP/Tampering | default Unix socket, remote opt-in + token, path confinement |
| SSH option injection (`-oProxyCommand`) | EoP | reject leading `-`, `--` separator |
| Token leak via argv/logs | Info disclosure | file-only token, no logging |
| History/recording world-readable | Info disclosure | 0600 files in 0700 dir |
| Plaintext token over non-loopback TCP | Info disclosure | document; recommend SSH tunnel; out of scope to add TLS |

## Sources
### Primary (HIGH)
- Repo source: src/api/mod.rs (712-740, 1319-1327, 1465-1495, 1521-1620), src/main.rs (24-80), src/pty/shell_integration.rs (110-175), src/pane/history_store.rs (160-240, 465-490), src/pane/colon_command.rs (885-895), src/remote/mod.rs (50-125), src/wm/mod.rs (205-225, 470-480), Cargo.toml, tests/*.rs, ARCHITECTURE_REVIEW_PLAN.md §5.1, ROADMAP Phase 19
### Tertiary (LOW)
- std/tokio/OpenSSH API specifics from training knowledge (no network lookups performed this session); tagged [ASSUMED]

## Metadata
**Confidence breakdown:**
- Standard stack: HIGH - no new deps, all verified in Cargo.toml
- Architecture: HIGH - call sites read directly
- Pitfalls: MEDIUM - platform behaviors (macOS paths, `%C`) from knowledge

**Research date:** 2026-10-23
**Valid until:** 30 days
