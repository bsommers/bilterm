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
6. [Persistent Remote SSH & `tmux` Security Invariants (Phase 51.1)](#6-persistent-remote-ssh--tmux-security-invariants-phase-511)
7. [Key Binding Safety: Reserved Chords & the Exit Guard (`KEY-02`)](#7-key-binding-safety-reserved-chords--the-exit-guard-key-02)

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

All persistent files written by Bilterm are confined via [`src/secure_fs.rs`](../../src/secure_fs.rs):

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

---

## 6. Persistent Remote SSH & `tmux` Security Invariants (Phase 51.1)

Bilterm's persistent remote `tmux -CC` integration (`:ssh-tmux`, `src/tmux/link.rs`, `src/remote/mod.rs`) enforces strict transport security boundaries:

1. **No Secrets on Argv or in Logs**:
   - Credentials (passwords, passphrases, tokens) are never accepted or passed via command-line arguments.
   - All tracing and log emissions in `bilterm::link` pass argv vectors through `redact_ssh_argv`, which masks identity key paths (`-i <key>`) and control paths (`ControlPath=<private>`), ensuring zero credential or private path leakage in `./bilterm.log`.
2. **Host Keys Are Never Auto-Accepted**:
   - Bilterm never auto-accepts untrusted host keys or disables host key verification.
   - Any OpenSSH host key verification question (`The authenticity of host ... can't be established`) is rendered directly into the primary pane for explicit user review and confirmation before connection continues.
3. **Fresh TCP Connection per Reconnect**:
   - Persistent links explicitly pass `-o ControlMaster=no` and `-o ControlPath=none`.
   - Each reconnect attempt initiates a clean TCP handshake from scratch. Stale, hung, or corrupted OpenSSH master control sockets cannot deadlock auto-reconnect loops or poison subsequent sessions.
4. **Typed Passwords Bypass Prompt & History Buffers**:
   - During the initial connection phase before the `tmux -CC` control sentinel (`ESC P 1000 p`) is detected, the pane operates in raw PTY passthrough mode (`set_link_raw_input`).
   - While the link is `Connecting` and waiting for a password or MFA response, every keystroke goes only to the remote `ssh` process. The command prompt is hidden and nothing is echoed on screen, so a password cannot be seen on the display, typed into the command line, or recorded in the prompt buffer, `history.log`, or the unified Find index.
   - **Recommendation**: Prefer key-based authentication with `ssh-agent` (and a hardware-backed or passphrase-protected key) for `:ssh-tmux` links. With a loaded agent, no password is typed at all, which also keeps the reconnect loop from stalling on a prompt.
5. **Strict Shell Wrapper Sanitization**:
   - Remote execution commands are wrapped in `exec $SHELL -lc 'exec env PATH="..." tmux -CC new-session -A -s NAME'`, where the session `NAME` is strictly validated against `[A-Za-z0-9_-]`. The argument contains exactly two single quotes and prohibits user-controlled quote escaping, preventing remote command injection across the SSH domain.

---

## 7. Key Binding Safety: Reserved Chords & the Exit Guard (`KEY-02`)

User key bindings (`[keys.bind]`, and the older `[keybindings]` table) are checked against a table of chords that are not Bilterm's to take. The same check runs at config load, after `:set keys...`, and on demand through `:keys check`, the `keys.check` RPC and the `bilterm_keys` MCP tool.

| Class | Meaning | What happens |
| :--- | :--- | :--- |
| **Block** | The OS, desktop or terminal host consumes the chord before Bilterm sees it (`Cmd+Space/Tab/H/M` on macOS, `Alt+F4` / `Alt+Tab` / `Ctrl+Alt+Delete` and every Win/Super chord on Windows and Linux, `Ctrl+Alt+F1..F7` and `Alt+F2` on Linux), or the chord is Bilterm's own (`Ctrl+Q`). | The binding is **rejected at config load** with a named error such as `config error: key binding 'Alt+F4' is reserved by Windows (close window) (using default keys)`. Bilterm starts with the built-in `[keys]` and `[keybindings]`; the rest of your config is kept. `:set` refuses the same bindings. |
| **Warn** | The chord reaches Bilterm but shadows something a program in your panes expects (readline `Ctrl+A/E/K/...`, vim `Ctrl+W`, tmux/screen/herdr prefixes, zellij and Claude Code shortcuts) or types a symbol on common layouts (macOS Option, Windows AltGr). | The binding loads. The status bar shows `N key conflicts: :keys check`; each finding is also written to the log at `warn` level. |
| **Info** | An override of a shipped binding, or a shipped default whose clash is accepted with a recorded reason. | Listed by `:keys check` only. |

**`Ctrl+Q` cannot be shadowed.** Quit always goes through the Exit Guard, so no user binding, `[keybindings]` entry or `:set` can capture `Ctrl+Q` and strand the workspace without a way out. A binding on the leader key is reported as a Warn because it makes the leader prefix unreachable.

**Shipped profiles are clean.** `mac`, `windows`, `linux` and `portable` carry no Block findings on any OS and surface; a unit test enforces this, and every shipped chord that still overlaps a common consumer is listed with its reason in the accepted-shadows table.

**Caps.** `[keys.bind]` and `[keys.window_mode]` accept at most 512 entries, and a chord string may be at most 64 characters. Larger tables or longer chords are rejected at load, so a hostile or generated config cannot inflate the keymap.

**API clients.** Over JSON-RPC or MCP, switching the key profile, entering Window mode and opening the menu bar are refused unless `[api] allow_config_write = true`; listing, checking and explaining chords are always read-only (see [API reference](../API.md#keyboard-surface-keys-menuopen-and-bilterm_keys-phase-63)).

The `[keys]` table itself (profiles, `legacy_alt`, `kitty_protocol`, `bind`, `unbind`) is described in [Chapter 6](06-keyboard-mouse-and-command-reference.md).
