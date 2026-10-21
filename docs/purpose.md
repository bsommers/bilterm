# Purpose, Motivation & Threat Model

[← Back to Documentation Hub](README.md) | [← Back to Project README](../README.md)

## 1. Problem Statement & Motivation

In conventional UNIX/POSIX terminal emulators (`xterm`, `vt100`), the command input line is interleaved directly into the standard output byte stream. This design creates several long-standing usability pain points:

1. **Prompt Loss During Streaming Output**: When a background or streaming command (`tail -f`, build logs, `ping`, `kubectl logs -f`) writes to the terminal, your active command line scrolls away or gets visually mangled by incoming output bytes.
2. **Destructive `SIGWINCH` Reflow**: Splitting or resizing a command area in traditional multiplexers sends `TIOCSWINSZ` (`SIGWINCH`) to the child process on every keystroke or layout toggle, causing `vim`, `htop`, or long wrapped log lines to thrash and misrender.
3. **Lack of Structured Multi-Line Editing & History Telemetry**: Traditional readline prompts cannot easily float over output, resize their width/height independently, display a persistent running command banner (`⟳ RUNNING:`), or pop up a modal session history inspector without corrupting the underlying terminal grid.

### The Bilterm Solution: IBM 3270 × DOS Borland Turbo Vision

**Bilterm** solves these problems by fusing two classic computing paradigms into a modern, zero-cost Rust + Ratatui architecture:

- **IBM 3270 Mainframe Composite Panes**: In an IBM 3270 terminal, the operator command entry field is decoupled from the scrolling screen buffer and accompanied by a dedicated **Operator Information Area (OIA)** telemetry bar and **full-row/full-column cross-hair cursor ruler**. Bilterm generalizes this into a **Composite Window Pane** where the command prompt can either **float smoothly over** the scrolling `OutputCanvas` (`Top`, `Bottom`, `Floating`) or **partition the pane with zero overlap** (`SplitTop`, `SplitBottom`).
- **DOS Borland Turbo Vision Windowing (`Turbo C` / `Turbo Pascal` IDE)**: Composite panes can be recursively split (`Alt+H`, `Alt+V`) or stacked in an **overlapping Z-order window deck** (`Alt+M`, `Alt+Tab`, `Alt+1..9`, mouse click-to-raise) complete with selectable box-drawing outlines (`╔═╗`, `╭─╮`, `┏━┓`, dotted, dashed), translucent/tinted character-cell drop shadows, a persistent **Guake/Quake drop-down terminal** (`F10` / `Alt+\``), and an interactive **`<F1>` Help modal**.

---

## 2. Threat Model & Security Boundaries

Because a terminal emulator and shell wrapper mediates between untrusted command output (log files, remote SSH streams, binary files) and privileged user input (passwords, shell commands, clipboard data), Bilterm enforces four strict security and isolation invariants:

### 2.1 Password Cleartext Prevention (`♦ !ECHO LOCK`)
- **Threat**: If a user runs `sudo`, `ssh`, `su`, or `gpg --pinentry-mode loopback` while typing into a local floating editor widget, the floating editor would display the password in cleartext on screen and save it into the command history log on disk.
- **Mitigation (`src/pty/line_discipline.rs`)**: Before routing any keystroke, Bilterm's 3-Signal Line-Discipline Detector inspects the PTY master file descriptor via `tcgetattr(fd)`. Whenever `ICANON` is enabled and `ECHO` is disabled (`!ECHO`), Bilterm immediately transitions the pane to `PaneInputMode::RawPtyPassthrough`, hides the floating command prompt, displays **`♦ !ECHO LOCK`** in red on the OIA status bar, and forwards keystrokes directly to the PTY without buffering or logging them to `HistoryStore`.

### 2.2 Untrusted Binary Stream & Escape-Sequence Flood Immunity (`tail -f /dev/random`)
- **Threat**: Running `cat /dev/urandom` or `tail -f /dev/random` emits random byte sequences that inevitably collide with ANSI/DEC control sequences such as `DECSET ?1049h` (enter alternate screen, which would hide the floating prompt and lock out the user) or `DECSTBM` (`\x1b[<t>;<b>r`, which traps scrolling to a 1-line region).
- **Mitigation (`src/vt/mod.rs`)**:
  1. `VtParser::sanitize_stream_traps` neutralizes `DECSET/DECRST 47/1047/1049` and `DECSTBM` sequences unless the foreground process has genuinely entered raw TUI mode (`!ICANON && !ECHO`, such as `vim` or `htop`).
  2. `LineDisciplineDetector` requires printable UTF-8 text before applying its fallback line-wrap heuristic, preventing random binary bytes from masquerading as an interactive TUI.
  3. The PTY reader thread (`src/pty/mod.rs`) batches reads in bounded 64KB chunks so the 60 FPS Ratatui render loop and floating command prompt remain responsive even under multi-megabyte/sec output floods.

### 2.3 Wide-Character (CJK / Emoji) Border Corruption Prevention
- **Threat**: When a floating window or drop shadow partially overlaps a 2-column wide UTF-8 grapheme (`漢字`, `🚀`, Nerd Font icons) on the underlying canvas, terminal emulators can misalign the entire row and corrupt window borders.
- **Mitigation (`src/ui/occlusion.rs`)**: `occlude_rect()` inspects the `unicode-width` of cells straddling the left (`x - 1`) and right (`x + width`) edges of every overlaid rectangle and replaces bisected wide-character halves with spaces (`" "`) prior to compositing.

### 2.4 Crash-Safe Terminal Restoration (`TerminalRestoreGuard`)
- **Threat**: If a TUI panics while raw mode, alternate screen, SGR mouse capture, and bracketed paste are enabled, the user's host shell is left unusable.
- **Mitigation (`src/error.rs`)**: `TerminalRestoreGuard` installs an idempotent `AtomicBool` panic and drop hook that emits `\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1004l\x1b[?2004l\x1b[?1049l\x1b[?25h` and disables raw mode before printing any backtrace.
