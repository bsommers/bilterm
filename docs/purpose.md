# Purpose, Motivation & Threat Model (`v1.12.0`)

[← Back to Documentation Hub](README.md) | [← Back to Project README](../README.md) | [Chapter 10: Security & Best Practices →](user-guide/10-security-and-best-practices.md)

## 1. Problem Statement & Motivation

In conventional UNIX/POSIX terminal emulators (`xterm`, `vt100`), the command input line is interleaved directly into the standard output byte stream. This design creates several long-standing usability pain points:

1. **Prompt Loss During Streaming Output**: When a background or streaming command (`tail -f`, build logs, `ping`, `kubectl logs -f`) writes to the terminal, your active command line scrolls away or gets visually mangled by incoming output bytes.
2. **Destructive `SIGWINCH` Reflow**: Splitting or resizing a command area in traditional multiplexers sends `TIOCSWINSZ` (`SIGWINCH`) to the child process on every keystroke or layout toggle, causing `vim`, `htop`, or long wrapped log lines to thrash and misrender.
3. **Lack of Structured Multi-Line Editing, Per-Pane Font Scaling & History Telemetry**: Traditional readline prompts cannot easily float over output, resize their width/height independently, scale font size per-pane (`Ctrl++`/`Ctrl+-`) so dense telemetry (`nmon`) coexists beside readable coding shells, display a persistent running command banner (`⟳ RUNNING:`), or pop up a modal session history inspector without corrupting the underlying terminal grid.

### The Bilterm Solution: IBM 3270 × DOS Borland Turbo Vision

**Bilterm** solves these problems by fusing two classic computing paradigms into a modern, zero-cost Rust + Ratatui architecture (available both inside any host terminal and as a standalone native desktop GUI window via `bilterm --gui`):

- **IBM 3270 Mainframe Composite Panes**: In an IBM 3270 terminal, the operator command entry field is decoupled from the scrolling screen buffer and accompanied by a dedicated **Operator Information Area (OIA)** telemetry bar and **full-row/full-column cross-hair cursor ruler**. Bilterm generalizes this into a **Composite Window Pane** where the command prompt can either **float smoothly over** the scrolling `OutputCanvas` (`Top`, `Bottom`, `Floating`) or **partition the pane with zero overlap** (`SplitTop`, `SplitBottom`).
- **DOS Borland Turbo Vision Windowing (`Turbo C` / `Turbo Pascal` IDE)**: Composite panes can be recursively split (`Alt+H`, `Alt+V`, all-border/corner mouse drag) or stacked in an **overlapping Z-order window deck** (`Alt+M`, `Alt+Tab`, `Alt+1..9`, mouse click-to-raise) complete with selectable box-drawing outlines (`╔═╗`, `╭─╮`, `┏━┓`, dotted, dashed), translucent/tinted character-cell drop shadows, independent per-pane font scaling (`Ctrl++`/`Ctrl+-`), a persistent **Guake/Quake drop-down terminal** (`F10` / `Alt+\``), a **`herdr`-style Slide-Out Fleet Drawer** (`F7`), and an interactive **`<F1>` Help modal**.

---

## 2. Threat Model & Security Boundaries

Because a terminal emulator, remote SSH fleet manager, and AI automation server mediates between untrusted command output (log files, remote SSH streams, binary files) and privileged user input (passwords, shell commands, clipboard data, RPC/MCP requests), Bilterm enforces strict defense-in-depth invariants (see **[Chapter 10: Security Architecture & Operational Best Practices](user-guide/10-security-and-best-practices.md)** for full operational guidance):

### 2.1 Password Cleartext Prevention (`♦ !ECHO LOCK`)
- **Threat**: If a user runs `sudo`, `ssh`, `su`, or `gpg --pinentry-mode loopback` while typing into a local floating editor widget, the floating editor would display the password in cleartext on screen and save it into the command history log on disk.
- **Mitigation (`src/pty/line_discipline.rs`)**: Before routing any keystroke, Bilterm's 3-Signal Line-Discipline Detector inspects the PTY master file descriptor via `tcgetattr(fd)` alongside password-prompt regex heuristics. Whenever `ICANON` is enabled and `ECHO` is disabled (`!ECHO`), Bilterm immediately transitions the pane to `PaneInputMode::RawPtyPassthrough`, hides the floating command prompt, displays **`♦ !ECHO LOCK`** in red on the OIA status bar, and forwards keystrokes directly to the PTY without buffering or logging them to `HistoryStore`.

### 2.2 Untrusted Binary Stream & Escape-Sequence Flood Immunity (`tail -f /dev/random`)
- **Threat**: Running `cat /dev/urandom` or `tail -f /dev/random` emits random byte sequences that inevitably collide with ANSI/DEC control sequences such as `DECSET ?1049h` (enter alternate screen, which would hide the floating prompt and lock out the user) or `DECSTBM` (`\x1b[<t>;<b>r`, which traps scrolling to a 1-line region).
- **Mitigation (`src/vt/mod.rs`)**:
  1. `VtParser::sanitize_stream_traps` neutralizes `DECSET/DECRST 47/1047/1049` and `DECSTBM` sequences unless the foreground process has genuinely entered raw TUI mode (`!ICANON && !ECHO`, such as `vim`, `htop`, or `nmon`).
  2. `LineDisciplineDetector` requires printable UTF-8 text before applying its fallback line-wrap heuristic, preventing random binary bytes from masquerading as an interactive TUI.
  3. The PTY worker thread (`src/vt/worker.rs`) reads in bounded 64KB chunks and publishes immutable `Arc<GridSnapshot>` frames via `tokio::sync::watch` so the 60 FPS Ratatui render loop and floating command prompt remain responsive even under multi-megabyte/sec output floods.

### 2.3 Wide-Character (CJK / Emoji) Border Corruption Prevention
- **Threat**: When a floating window or drop shadow partially overlaps a 2-column wide UTF-8 grapheme (`漢字`, `🚀`, Nerd Font icons) on the underlying canvas, terminal emulators can misalign the entire row and corrupt window borders.
- **Mitigation (`src/ui/occlusion.rs`)**: `occlude_rect()` inspects the `unicode-width` of cells straddling the left (`x - 1`) and right (`x + width`) edges of every overlaid rectangle and replaces bisected wide-character halves with spaces (`" "`) prior to compositing.

### 2.4 Crash-Safe Terminal Restoration (`TerminalRestoreGuard`)
- **Threat**: If a TUI panics while raw mode, alternate screen, SGR mouse capture, and bracketed paste are enabled, the user's host shell is left unusable.
- **Mitigation (`src/error.rs`)**: `TerminalRestoreGuard` installs an idempotent `AtomicBool` panic and drop hook that emits `\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1004l\x1b[?2004l\x1b[?1049l\x1b[?25h` and disables raw mode before printing any backtrace.

### 2.5 Clipboard Paste Injection & Bracketed-Paste Breakout Defense (`FND-01`)
- **Threat**: Malicious web pages or terminal payloads copied to the system clipboard may embed hidden C0 control characters (`NUL`, `ESC`, `DEL`, `VINTR 0x03`, `VQUIT 0x1C`, `VSUSP 0x1A`), premature bracketed-paste closing sequences (`\x1b[201~`), or newline-terminated shell commands when pasted into a raw PTY.
- **Mitigation (`src/vt/mod.rs`, `src/input/mod.rs`, `src/ui/mod.rs`)**:
  1. `sanitize_paste_bytes()` strips dangerous C0/DEL control bytes from all clipboard pastes before they reach either the floating command editor or the child PTY.
  2. `is_unsafe_paste()` intercepts unbracketed multi-line pastes and embedded `\x1b[201~` breakout payloads in `INPUT: RAW PTY` mode and opens the Borland Turbo Vision **`[!] UNSAFE PASTE GUARD`** confirmation modal (`y`/`Enter` to confirm, `n`/`Esc` to abort).

### 2.6 Read-Only Protected 3270 Field Lock (`🔒 PROT`, `ECO-04`)
- **Threat**: Accidental keystrokes or `:broadcast` fan-out into a sensitive production SSH or monitoring pane can disrupt critical jobs.
- **Mitigation (`src/pane/mod.rs`, `src/input/mod.rs`)**: Pressing **`Alt+L`** or running **`:lock on`** places the pane in Read-Only Protected mode (`🔒 PROT` badge in OIA), blocking all shell submissions and raw PTY keystrokes (and excluding the pane from `:broadcast` fan-out) while still allowing local `:colon` inspection commands.

### 2.7 Filesystem Confinement, Private `0700`/`0600` Permissions & Zero-Trust RPC (`SEC-R01..04`)
- **Threat**: Shared multi-user systems, path-traversal payloads (`../../`), symlink attacks, or unauthenticated localhost/LAN sockets could expose terminal sessions.
- **Mitigation (`src/util/secure_fs.rs`, `src/api/rpc.rs`, `src/remote/mod.rs`)**:
  1. All runtime directories (`$XDG_RUNTIME_DIR/bilterm`) are created with mode `0700`, and all history logs, session TOMLs, `.cast` recordings, and `.ppm` snapshots are written atomically with mode `0600` (`O_NOFOLLOW`) and validated with `validate_safe_name` (rejecting `..`, `/`, `\`, NUL).
  2. `bilterm --headless` defaults to a private `0600` Unix domain socket with kernel `SO_PEERCRED` UID verification. Non-loopback TCP requires `--allow-remote` and a mode `0600` `--token-file` (`auth.login` constant-time handshake; secrets are never accepted on CLI `argv`).
  3. Remote `:ssh` destinations reject leading `-` characters and separate arguments with `--` to prevent OpenSSH `ProxyCommand` argument injection.
