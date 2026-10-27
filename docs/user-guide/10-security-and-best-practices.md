# Chapter 10: Security Architecture & Operational Best Practices

[← Chapter 9: Use Cases & Workflows](09-use-cases-and-workflows.md) | [User Guide Home](README.md) | [Project README →](../../README.md)

Terminal multiplexers and automation daemons sit at the highest-privilege boundary on a developer or production machine: every keystroke, password, SSH session, and AI agent command passes through them. Bilterm enforces **defense-in-depth security invariants (`SEC-R01..04`, `FND-01`, `ADR-002`)** by default.

---

## Table of Contents

1. [3-Signal `!ECHO LOCK` Password & Secret Isolation (`ADR-002`)](#1-3-signal-echo-lock-password--secret-isolation-adr-002)
2. [Paste Sanitization & Bracketed Paste Breakout Defense (`FND-01`)](#2-paste-sanitization--bracketed-paste-breakout-defense-fnd-01)
3. [Filesystem & Credential Confinement (`SEC-R01..04`)](#3-filesystem--credential-confinement-sec-r0104)
4. [JSON-RPC Daemon & MCP Control-Plane Hardening](#4-json-rpc-daemon--mcp-control-plane-hardening)
5. [Production & Multi-Agent Security Best Practices](#5-production--multi-agent-security-best-practices)

---

## 1. 3-Signal `!ECHO LOCK` Password & Secret Isolation (`ADR-002`)

A naive shell wrapper that buffers input inside a TUI text box risks echoing `sudo`, `ssh`, `gpg`, or database passwords in cleartext and saving them to command history.

Bilterm prevents credential leakage using a **3-signal line-discipline detector** (`src/pty/line_discipline.rs`) polled on every PTY output event and before every keystroke:

1. **Signal 1 — Kernel `termios` Inspection (`tcgetattr`)**: Queries the PTY master file descriptor for `ECHO` and `ICANON` flags. When a child process disables `ECHO` (`!ECHO`), the pane immediately transitions to `InputRoutingMode::RawPtyPassthrough` (`🔒 !ECHO LOCK`).
2. **Signal 2 — Foreground Process Group Tracking (`tcgetpgrp`)**: Detects when a foreground child process group differs from the login shell PID.
3. **Signal 3 — Active Canvas Password Prompt Heuristic**: Scans the active cursor row for interactive credential prompts (`Password:`, `[sudo] password`, `Passphrase`, `Enter PIN`, `Verification code:`) to guard remote hops where outer `termios` changes have not yet propagated.

**Security Guarantee**:
- While `🔒 !ECHO LOCK` or an active password prompt is detected, keystrokes bypass the floating command prompt buffer completely and write directly to the PTY master FD.
- Passwords **never** enter `FloatingPromptState::input`, undo history, persistent session history files, `:broadcast` fan-out, or Asciicast `.cast` input logs.

---

## 2. Paste Sanitization & Bracketed Paste Breakout Defense (`FND-01`)

Clipboard hijacking ("pastehacking") embeds hidden control characters, newlines, or `\x1b[201~` bracketed-paste-end sequences inside web snippets to execute arbitrary commands when pasted into a terminal.

Bilterm enforces two layers of paste defense (`src/vt/mod.rs`, `src/input/mod.rs`):

1. **Mandatory Control-Byte Sanitization (`sanitize_paste_bytes`)**: Strips dangerous C0 control characters (`NUL`, `ETX`, `EOT`, `BEL`, `BS`, `VT`, `FF`, `ESC`, `DEL`) from pasted payloads.
2. **Interactive Unsafe-Paste Quarantine**:
   - In **Floating Prompt Mode**, pasted multi-line text is inserted into the editor buffer *without* executing newlines, letting you inspect and edit the command before pressing `Enter`.
   - In **Raw PTY Passthrough Mode**, if a paste contains multiple lines without `DECSET ?2004` bracketed paste active—or contains an embedded `\x1b[201~` breakout sequence—Bilterm blocks immediate execution and opens an **Unsafe Multi-Line Paste Confirmation Modal** (`Press [y/Enter] to confirm or [n/Esc] to cancel`).
3. **Terminal Cluster Sanitization (`is_safe_terminal_cluster`)**: Blocks bidirectional text-spoofing overrides (`U+202A..U+202E`, `U+2066..U+2069` Trojan Source characters) and zero-width joiners (unless `DECSET ?2027` is explicitly active) from corrupting cell rendering.

---

## 3. Filesystem & Credential Confinement (`SEC-R01..04`)

All persistent files written by Bilterm are confined via [`src/util/secure_fs.rs`](../../src/util/secure_fs.rs):

| Rule ID | Security Control | Enforcement Mechanism |
| :--- | :--- | :--- |
| **`SEC-R01`** | **Private State Directories (`0700`)** | `~/.local/state/bilterm/`, `~/.config/bilterm/`, and `$XDG_RUNTIME_DIR/bilterm/` are created and verified with `0700` (`rwx------`) permissions via `ensure_private_dir`. |
| **`SEC-R02`** | **Atomic Private File Writes (`0600`)** | Session layouts, command history exports, `.cast` recordings, and `.ppm`/`.svg` snapshots are written to a `0600` (`rw-------`) temporary file and atomically renamed via `write_private_file_atomic`. |
| **`SEC-R03`** | **Path Traversal Rejection** | Session names, history names, and RPC snapshot filenames pass through `validate_safe_name`, rejecting `..`, `/`, `\`, leading `.`, and NUL bytes. |
| **`SEC-R04`** | **Zero CLI `argv` Secrets** | Bearer tokens are **never** accepted as raw CLI strings (which would be world-readable in `/proc/<pid>/cmdline` and `ps aux`). Bilterm requires `--token-file <PATH>` or `BILTERM_TOKEN_FILE` pointing to a `0600` file. |

---

## 4. JSON-RPC Daemon & MCP Control-Plane Hardening

When running `bilterm --headless` or `bilterm --mcp`:

1. **Private Unix Socket by Default**: `bilterm --headless` listens on `$XDG_RUNTIME_DIR/bilterm/control.sock` inside a `0700` directory with `0600` socket permissions, restricting access to your OS UID.
2. **Fail-Closed Remote TCP Guard**: Binding `--listen` to a non-loopback address (anything other than `127.0.0.1`, `::1`, or `localhost`) is **rejected at startup** unless both `--allow-remote` and a valid `0600` `--token-file <PATH>` are provided.
3. **Constant-Time Token Verification**: JSON-RPC requests over authenticated endpoints verify the `auth_token` using constant-time byte comparison to prevent timing side-channels.
4. **Snapshot Directory Confinement (`--snapshot-dir`)**: Any `:canvas export`, `:history save`, or `.ppm` dump triggered during a JSON-RPC, MCP, or Playbook turn is confined strictly inside the configured `--snapshot-dir`.

---

## 5. Production & Multi-Agent Security Best Practices

### Best Practice 1: Lock Production Monitoring Panes Read-Only (`:lock on` / `Ctrl+X l`)
When connected to a production database (`psql`, `redis-cli`) or watching live logs alongside active development panes, lock the production pane:
```text
:lock on
```
A `🔒` badge appears in the pane's title bar and OIA status bar. All keystrokes, pastes, and `:broadcast` commands to that pane are blocked until you explicitly run `:lock off` (or press `Ctrl+X l`).

### Best Practice 2: Use `0600` Token Files for Headless Automation
Generate a cryptographically random bearer token with strict `0600` permissions before starting a TCP headless daemon:
```bash
(umask 077 && mkdir -p ~/.config/bilterm && head -c 32 /dev/urandom | base64 > ~/.config/bilterm/rpc.token)
bilterm --headless --listen 127.0.0.1:7370 --token-file ~/.config/bilterm/rpc.token
```

### Best Practice 3: Confine AI Agent Snapshots with `--snapshot-dir`
When exposing Bilterm to an LLM or autonomous coding agent via `bilterm --mcp`, always pass a dedicated `--snapshot-dir` so agent-initiated file exports are isolated to a scratch directory:
```bash
bilterm --mcp --snapshot-dir /tmp/bilterm-agent-sandbox
```

### Best Practice 4: Audit Terminal State with `<F9>` (`:inspector`)
If a shell session behaves unexpectedly after an SSH disconnect or a crashed binary, press `<F9>` (`:inspector`) to inspect the active `DECSET` modes (`?1049`, `?1006`, `?2004`) and `termios` flags, or press `Ctrl+G` / `F12` to toggle manual raw passthrough override.
