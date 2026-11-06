# Chapter 7: Troubleshooting & FAQ

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 6: Reference](06-keyboard-mouse-and-command-reference.md) | [Chapter 8: Extended CLI & GUI Examples →](08-extended-cli-and-gui-examples.md)

## Table of Contents

- [1. Common Symptoms & Solutions](#1-common-symptoms--solutions)
  - [Floating Command Prompt Disappeared](#floating-command-prompt-disappeared)
  - [Per-Pane Font Zoom (`Ctrl++` / `Ctrl+-`) Inside `nmon`, `htop`, or `vim`](#per-pane-font-zoom-ctrl--ctrl--inside-nmon-htop-or-vim)
  - [Resizing Panes in 3-Way or Multi-Pane Splits by Mouse](#resizing-panes-in-3-way-or-multi-pane-splits-by-mouse)
  - [Top Lines of Output Are Hidden Behind the Floating Prompt](#top-lines-of-output-are-hidden-behind-the-floating-prompt)
  - [Streaming Binary Output (`tail -f /dev/random` / `cat binary`)](#streaming-binary-output-tail--f-devrandom--cat-binary)
  - [Wide Characters (CJK / Emoji / Nerd Fonts) Near Window Borders](#wide-characters-cjk--emoji--nerd-fonts-near-window-borders)
  - [Clipboard Copy (`:yank`) Not Reaching Host Clipboard Inside `tmux`](#clipboard-copy-yank-not-reaching-host-clipboard-inside-tmux)
  - [Remote tmux over SSH Quits Early or Reconnect is Odd](#remote-tmux-over-ssh-quits-early-or-reconnect-is-odd)
- [2. Diagnostics & Logging](#2-diagnostics--logging)
- [3. Terminal State Recovery After Forced Kill (`SIGKILL`)](#3-terminal-state-recovery-after-forced-kill-sigkill)
- [4. Frequently Asked Questions (FAQ)](#4-frequently-asked-questions-faq)

---

## 1. Common Symptoms & Solutions

### Floating Command Prompt Disappeared

- **Symptom**: You started `vim`, `nvim`, `htop`, `nmon`, `less`, `sudo`, or `ssh`, or pressed `F12` / `Ctrl+G`, and the floating command prompt box (`╔═╣ BILTERM 3270 ... ╠═╗`) vanished.
- **Cause**:
  1. **Alternate Screen (`▣ ALT SCR`)**: The foreground program entered `DECSET 47/1047/1049`. Bilterm automatically hides the floating prompt and switches to `INPUT: RAW PTY` so full-screen TUIs receive every keystroke and mouse event directly.
  2. **Password Lock (`♦ !ECHO LOCK`)**: The foreground process (`sudo`, `ssh`, `gpg`) disabled terminal `ECHO` (`termios !ECHO`). Bilterm hides the prompt so your password is never echoed or recorded in command history.
  3. **Manual Raw Mode (`F12` / `Ctrl+G`)**: You toggled `INPUT: RAW PTY` manually.
- **Solution**:
  - Exit the interactive TUI or finish entering your password — the floating prompt automatically reappears when the shell returns to normal canonical/echo mode.
  - Or press **`F12`** / **`Ctrl+G`** (or **`F4`** / `:raw` / `:mode float`) to toggle back to `INPUT: CMD BOX`.

### Per-Pane Font Zoom (`Ctrl++` / `Ctrl+-`) Inside `nmon`, `htop`, or `vim`

- **How it works (`v1.12.0+`)**:
  - Pressing **`Ctrl++`** (`Ctrl+=`), **`Ctrl+-`** (`Ctrl+_`), or **`Ctrl+0`**, or holding **`Ctrl` + Mouse Scroll Wheel** scales **only the focused (or hovered) pane's font** (`50%..=300%`) and dynamically recalculates that pane's `cols × rows` PTY grid (`TIOCSWINSZ`).
  - Even when `nmon`, `htop`, or `vim` is running in `INPUT: RAW PTY` / `DECSET 1049` mode, Bilterm intercepts `Ctrl++`, `Ctrl+-`, `Ctrl+0`, and `Ctrl+Wheel` at the Window Manager layer so interactive TUI dashboards immediately resize and reflow to the new font scale.

### Resizing Panes in 3-Way or Multi-Pane Splits by Mouse

- **How it works (`v1.12.0+`)**:
  - Every border (`Top`, `Bottom`, `Left`, `Right`) and every corner (`TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`) of every pane is draggable with the mouse in both `Tiled` and `Stacked` modes.
  - If a pane's geometry ever feels skewed after heavy resizing, press **`Ctrl+B =`** (or **`Ctrl+X =`**) or run **`:equalize`** (`:resize reset`) to restore balanced 50/50 split ratios.

### Top Lines of Output Are Hidden Behind the Floating Prompt

- **Symptom**: In default `PIN:TOP` mode, the 3-row floating prompt overlays the top 3 rows of the underlying PTY canvas.
- **Solution**:
  - Switch to **Non-Overlapping Split Mode** by pressing **`Alt+S`** or running **`:prompt split-top`** (or **`:prompt split-bottom`**). In split mode, the command prompt and output canvas occupy disjoint vertical regions with **zero overlap**.
  - Alternatively, pin the floating prompt to the bottom (`Alt+B` / `:prompt bottom`), drag its top border to free-float it elsewhere, or narrow its width (`Alt+[` / `:prompt width 50`).

### Streaming Binary Output (`tail -f /dev/random` / `cat binary`)

- **Symptom**: A command floods the terminal with random binary bytes containing arbitrary `\x1b[...` sequences.
- **How Bilterm Protects You**:
  - Bilterm's `VtParser::sanitize_stream_traps` strips accidental `DECSET 47/1047/1049` (alternate-screen traps) and `DECSTBM` (`\x1b[r` scroll-region traps) while `allow_alt_screen_transitions` is false, keeping the floating prompt visible and responsive.
  - To stop the foreground binary flood, press **`Ctrl+C`** directly inside the command prompt (which forwards `0x03` `SIGINT` to the PTY) and then run `:clear` to reset the canvas.

### Wide Characters (CJK / Emoji / Nerd Fonts) Near Window Borders

- **Symptom**: Some terminal emulators corrupt box-drawing borders when a 2-column wide character is bisected by an overlapping window or drop shadow.
- **How Bilterm Protects You**:
  - Bilterm's `occlude_rect` compositor pass (`src/ui/occlusion.rs`) inspects `unicode-width` cell boundaries on both the left and right edges of every floating prompt, stacked window, and drop shadow, replacing bisected 2-column cells with clean spaces (`" "`) before drawing borders.
  - If your host terminal font lacks Nerd Font or CP437 glyphs, switch to standard Unicode or pure 7-bit ASCII glyphs:
    ```text
    ==> :glyphs unicode
    ==> :glyphs ascii
    ```

### Clipboard Copy (`:yank`) Not Reaching Host Clipboard Inside `tmux`

- **Symptom**: Running `:yank` inside an outer `tmux` session does not update your local system clipboard.
- **Cause**: Outer `tmux` blocks `DCS tmux;` passthrough unless `allow-passthrough` is enabled in `~/.tmux.conf`.
- **Solution**: Run the following in your host shell or add it to `~/.tmux.conf`:
  ```bash
  tmux set -g allow-passthrough on
  tmux set -s set-clipboard on
  ```

### Remote tmux over SSH Quits Early or Reconnect is Odd

- **Symptom**: You ran `:ssh-tmux <user@host> [session]`, but the remote session seems to drop, the pane shows `LINK DOWN` or `LINK FAILED`, reconnect backoff loops repeatedly, or after restarting Bilterm a restored session shows `(not connected)`.
- **Diagnosis Checklist**:
  1. **Read the Link Banner & Check `:ssh-tmux info`**:
     - When a connection drops or reconnects, an interactive link banner appears at the bottom of the pane canvas showing current link state and exit cause:
       `LINK DOWN (<cause>) | retry <attempt>/20 in <secs>s | Enter/r: now | x: stop | d: detach`
     - Run `:ssh-tmux info` to view the full link status, retry deadline, attempt counter, mapped panes, and last exit reason and diagnostic tail lines.
  2. **Inspect Structured Tracing Logs (`bilterm.log`)**:
     - Run Bilterm with debug tracing enabled:
       ```bash
       RUST_LOG=bilterm=debug bilterm 2>bilterm-debug.log
       ```
     - Logs are written to `./bilterm.log` (or `$XDG_STATE_HOME/bilterm/bilterm.log`, usually `~/.local/state/bilterm/bilterm.log`).
     - Look for structured `bilterm::link` events:
       - `link.connect`: Connection attempt started with redacted arguments (`dest`, `session`, `attempt`).
       - `link.up`: Control stream handshake succeeded (`ESC P 1000 p` sentinel seen).
       - `link.down`: Connection dropped. Check `cause`, `code`, `reason`, and `tail_lines` for remote error text.
       - `link.retry`: Exponential backoff timer scheduled (`attempt`, `delay_ms`).
       - `link.give_up`: 20-attempt retry limit reached.
       - `link.control_exit`: Remote `tmux` process issued `%exit <reason>`.
       - `link.spawn_failed`: Local `ssh` executable failed to spawn.
       - `link.hydrate`: Statistics on pane capture and screen hydration.
       - `pane.exit`: Child process exited with return code.
  3. **Verify the Remote `tmux` Session**:
     - Log in directly to the remote host (e.g. `ssh user@mac`) and check running sessions:
       ```bash
       tmux ls
       ```
     - If `tmux ls` shows no session (or "no server running"), the remote `tmux` server terminated.
  4. **Check Remote `~/.tmux.conf` Settings**:
     - Remote `tmux` options can cause sessions to terminate automatically:
       - `set -g exit-empty on`: Exits the `tmux` server when no sessions or windows remain.
       - `set -g destroy-unattached on`: Destroys a session as soon as the client disconnects! Ensure this is set to `off` (or omitted) in `~/.tmux.conf` if you want persistent sessions.
       - `set -g detach-on-destroy on`: Determines whether `tmux` detaches or switches sessions when destroyed.
  5. **Check Mac Sleep & Power Management Settings**:
     - macOS aggressively suspends network connections during system sleep.
     - On the remote Mac host, inspect sleep parameters:
       ```bash
       pmset -g
       pmset -g | grep -i -E "sleep|womp|tcpkeepalive"
       ```
     - If `tcpkeepalive 0` or sleep triggers, TCP connections are dropped when the display sleeps. Enable network keepalives with `sudo pmset -a tcpkeepalive 1`, or prevent sleep during long jobs using `caffeinate -s`.
  6. **SSH Keepalives & Network Timeouts**:
     - Bilterm automatically applies `-o ServerAliveInterval=15 -o ServerAliveCountMax=3 -o TCPKeepAlive=yes -o ConnectTimeout=15` to both `:ssh` and `:ssh-tmux` connections.
     - If stateful NAT gateways or firewalls drop idle connections faster than 15 seconds, adjust your local `~/.ssh/config` or remote `sshd_config` `ClientAliveInterval`.
  7. **Restored Workspace Shows `(not connected)`**:
     - When restoring a saved workspace session, Bilterm honestly labels previously remote panes as `(not connected)` if they are running a local shell rather than a live remote transport.
     - Run `:ssh-tmux <dest> [session]` (or `Ctrl+X u`) to reconnect to the remote session.

---

## 2. Diagnostics & Logging

Because Bilterm runs inside the host terminal's alternate screen in raw mode, writing diagnostic messages to `stdout` or `stderr` would corrupt the TUI frame buffer. Instead, Bilterm uses `tracing` + `tracing-appender` to write non-blocking logs to **`./bilterm.log`**.

To run Bilterm with debug-level tracing enabled:

```bash
RUST_LOG=bilterm=debug bilterm
```

In a second terminal (or inside a Bilterm composite pane!), inspect the live log stream:

```bash
tail -f bilterm.log
```

---

## 3. Terminal State Recovery After Forced Kill (`SIGKILL`)

Bilterm installs a `TerminalRestoreGuard` (`src/error.rs`) that automatically leaves the alternate screen, disables SGR mouse capture, disables bracketed paste, and restores cooked `termios` mode on normal exit, errors, and Rust panics.

If the `bilterm` process is forcibly terminated by the OS kernel (`kill -9` / `SIGKILL`, which cannot be caught by user-space hooks), run the following in your host shell to restore your terminal:

```bash
printf '\033[?1000l\033[?1002l\033[?1003l\033[?1006l\033[?2004l\033[?1049l\033[?25h' && reset
```

---

## 4. Frequently Asked Questions (FAQ)

- **Q: How do I launch Bilterm as a standalone desktop GUI window with a custom font and size?**
  - **A:** Run `bilterm --gui --gui-font "JetBrains Mono" --gui-font-size 16` (or pass `--gui-font-file /path/to/font.ttf`). See **[Chapter 8: Extended CLI, Native `--gui` & Typography Examples](08-extended-cli-and-gui-examples.md)** for full details.
- **Q: How do I zoom the font size of a single pane without changing the rest of the workspace?**
  - **A:** Focus the pane and press **`Ctrl++`** / **`Ctrl+-`** / **`Ctrl+0`**, or hold **`Ctrl` + Mouse Wheel** over that pane, or run `:fontsize 130%` / `:font pane 2 jetbrains 125%`.
- **Q: How do I turn on full-screen horizontal and vertical cursor cross-hairs?**
  - **A:** Press **`Alt+X`** twice (1st press = Pane-Local cross-hairs, 2nd press = Full-Screen cross-hairs, 3rd press = Off), or run **`:crosshair full`**. To enable by default on startup, set `crosshair_cursor = true` and `crosshair_fullscreen = true` in `~/.config/bilterm/config.toml`.
- **Q: Where are my session command history files saved, and how do I turn history persistence off?**
  - **A:** By default (`history_enabled = true`), commands are saved to `$XDG_STATE_HOME/bilterm/history/sessions/<name>/history.log` (or `~/.local/state/bilterm/history/`), where `<name>` is `history_session` / `--session` (default `default`), and restored into new panes on the next run. Run **`:history off`** to disable persistence at runtime, **`:history runs`** to list recorded runs, **`:history max-files <N>`** to limit retained session directories, or **`:history save /tmp/commands.sh`** (or **`Alt+P`** then **`s`**) to export the current pane's commands to a shell script.
- **Q: Why does `bilterm ctl` fail with error code -32001 ("authentication required: send auth.login first")?**
  - **A:** The target Bilterm daemon has a bearer token configured. Supply the token file using `bilterm ctl --token-file <path>` or set the `BILTERM_TOKEN_FILE` environment variable. The token file must have `0600` permissions (read/write by owner only). See **[Chapter 10: Security Architecture & Operational Best Practices](10-security-and-best-practices.md)**.
- **Q: Why does `bilterm --headless --listen <addr>` exit with "refusing to listen on non-loopback address"?**
  - **A:** Bilterm restricts TCP listeners to loopback (`127.0.0.1`, `::1`, `localhost`) by default. To listen on all interfaces or a LAN IP, pass `--allow-remote` along with `--token-file <path>`. Note that tokens travel over plaintext TCP, so an SSH tunnel is strongly recommended.
- **Q: Can I use different border styles for workspace windows vs. the floating command prompt?**
  - **A:** Yes! Use `:outline window <style>` and `:outline prompt <style>` (choosing from `double`, `solid`, `rounded`, `thick`, `dotted`, `dashed`, `quadruple`, or `ascii`), or set `window_outline` and `prompt_outline` in `~/.config/bilterm/config.toml`.

---

[← Chapter 6: Reference](06-keyboard-mouse-and-command-reference.md) | [User Guide Home](README.md) | [Chapter 8: Extended CLI & GUI Examples →](08-extended-cli-and-gui-examples.md)
