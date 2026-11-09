# Chapter 5: First-Class `tmux` Integration

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 4: Themes & Config](04-themes-glyphs-and-config.md) | [Chapter 6: Reference →](06-keyboard-mouse-and-command-reference.md)

## Table of Contents

- [1. Mode A: Running Bilterm *Inside* an Outer `tmux` Session (`TMUX-01`)](#1-mode-a-running-bilterm-inside-an-outer-tmux-session-tmux-01)
- [2. Mode B: Controlling `tmux` via `tmux -CC` Control Mode (`TMUX-02` & `TMUX-03`)](#2-mode-b-controlling-tmux-via-tmux--cc-control-mode-tmux-02--tmux-03)
- [3. Mode C: Remote SSH Domains & `herdr` Fleet Federation](#3-mode-c-remote-ssh-domains--herdr-fleet-federation)
- [4. Mode D: Persistent Remote `tmux -CC` Sessions (`:ssh-tmux`)](#4-mode-d-persistent-remote-tmux--cc-sessions-ssh-tmux)
  - [4.1 Real PTY Transport and Frozen SSH Options](#41-real-pty-transport-and-frozen-ssh-options)
  - [4.2 Login-Shell Wrapper and Remote macOS PATH](#42-login-shell-wrapper-and-remote-macos-path)
  - [4.3 Interactive Connect Phase and Password Safety](#43-interactive-connect-phase-and-password-safety)
  - [4.4 Link States and Banners](#44-link-states-and-banners)
  - [4.5 Auto-Reconnect with Exponential Backoff](#45-auto-reconnect-with-exponential-backoff)
  - [4.6 Topology Hydration with `capture-pane`](#46-topology-hydration-with-capture-pane)
  - [4.7 Single Link Guard and Fresh TCP Policy](#47-single-link-guard-and-fresh-tcp-policy)
  - [4.8 Remote `tmux` Options That End a Session](#48-remote-tmux-options-that-end-a-session)
  - [4.9 Known Limits](#49-known-limits)

---

Bilterm supports four complementary `tmux` operating modes (`src/tmux/mod.rs`):

## 1. Mode A: Running Bilterm *Inside* an Outer `tmux` Session (`TMUX-01`)

When launched inside an existing `tmux` session (`$TMUX` environment variable detected):
- **Zero Prefix Collisions**: Bilterm binds its window manager shortcuts to `Alt+` chords (`Alt+H`, `Alt+V`, `Alt+Tab`, `Alt+1..9`, `Alt+\``), `F1`/`F2`/`F5`/`F6`/`F10`/`F12`, and `:command`s, leaving `Ctrl+B` completely free for your outer `tmux` multiplexer.
- **Automatic `DCS tmux;` Passthrough (`OSC 52`)**: When you yank text via `:yank` or a child process emits an `OSC 52` clipboard sequence, Bilterm automatically wraps the sequence in `\x1bPtmux;\x1b...\x1b\\` (doubling internal `ESC` bytes) so clipboard operations reach your local terminal across nested `tmux` over SSH.

---

## 2. Mode B: Controlling `tmux` via `tmux -CC` Control Mode (`TMUX-02` & `TMUX-03`)

Bilterm's `TmuxControlBridge` and `TmuxControlSubprocess` (`src/tmux/mod.rs`) provide a full bi-directional `tmux -CC` Control Mode bridge:
- **`%output %<pane_id> <octal-escaped-bytes>`**: Unescapes `tmux` octal sequences (`\033`, `\015`, `\012`) and feeds each `%pane_id` stream into its mapped Bilterm `CompositePane` `VtParser`.
- **`%layout-change` Topology Hydration (`parse_tmux_layout_panes`)**: Parses `tmux` layout strings (`<checksum>,<WxH>,<X>,<Y>{...}[...]`) to extract `%pane_id` geometries and automatically creates 1:1 local `CompositePane` instances on the fly.
- **`%window-add`, `%window-pane-changed`, `%window-close`, `%session-changed`, `%exit`**: Synchronizes `tmux` windows, focus changes, closures, and session lifecycles with Bilterm's `WindowManager`.
- **Bi-Directional Command & Split Dispatch**:
  - Commands submitted from Bilterm's `FloatingCommandPrompt` are routed via `TmuxControlBridge::route_prompt_submission` (`send-keys -t %<id> -H <hex>`).
  - Local pane splits and client resizes queue `split-window -v/-h -t %<id>` (`queue_split`) and `refresh-client -C <cols>x<rows>` (`queue_resize_client`) to the live `tmux -CC` subprocess (`TmuxControlSubprocess::spawn_attach_or_new`).

---

## 3. Mode C: Remote SSH Domains & `herdr` Fleet Federation

Bilterm extends composite pane multiplexing across remote SSH hosts (`src/remote/mod.rs`):
- **First-Class SSH Composite Panes (`:ssh [split h|v] <user@host[:port]> [cmd]`)**: Spawns an OpenSSH session with keepalive flags (`ServerAliveInterval=15`, `ServerAliveCountMax=3`, `TCPKeepAlive=yes`, `ConnectTimeout=15`, and shortened `ControlPersist=10s`) inside the focused or newly split pane. Displays `SSH:<user@host>` in the 3270 OIA status bar while keeping the floating command prompt completely local and zero-latency. Destinations are strictly validated (no leading `-`, no whitespace/control characters) and arguments are separated with `--` before the destination. SSH connection multiplexing sockets are placed safely in Bilterm's private runtime directory (`ControlPath=<private_dir>/ssh-%C`).
- **`herdr`-Style Central Fleet Federation (`:remote add|list|connect|exec|disconnect`)**: Connects to remote `bilterm --headless` daemons across your infrastructure (remote daemons require `--allow-remote --token-file <path>`) and forwards JSON-RPC 2.0 requests (`remote.exec` / `:remote exec <name> <method> [json]`) from a single central Bilterm dashboard. Note that headless daemons have no live PTYs for local terminal rendering; automation operates via JSON-RPC / MCP.

---

## 4. Mode D: Persistent Remote `tmux -CC` Sessions (`:ssh-tmux`)

Persistent remote terminal sessions are managed via `:ssh-tmux` (`src/tmux/link.rs`, `src/tmux/reconnect.rs`, `src/wm/tmux_link.rs`). Bilterm runs a real PTY-backed `ssh -tt` process into `tmux -CC`, tracks connection health, renders interactive banners, and automatically reconnects with exponential backoff when networks drop.

### 4.1 Real PTY Transport and Frozen SSH Options

When `:ssh-tmux <user@host> [session]` is invoked, Bilterm spawns a dedicated PTY child running `ssh -tt` with frozen connection options (`src/remote/mod.rs`):
- `-tt`: Allocates a pseudo-terminal on the remote host so `tmux -CC` has an underlying tty for ioctls.
- `-o ServerAliveInterval=15`: Sends keepalive probes every 15 seconds to prevent NAT or stateful firewall timeouts.
- `-o ServerAliveCountMax=3`: Drops dead connections after 3 unanswered keepalives (45 seconds).
- `-o TCPKeepAlive=yes`: Enables kernel-level TCP keepalives.
- `-o ConnectTimeout=15`: Fails connection attempts quickly when the remote host is unreachable.
- `-o ControlMaster=no` and `-o ControlPath=none`: Bypasses OpenSSH socket multiplexing. Every connection and reconnection opens a completely fresh TCP connection, preventing corrupted or stale control sockets from stalling reconnection loops.

### 4.2 Login-Shell Wrapper and Remote macOS PATH

Remote systems (especially macOS) often place `tmux` in non-standard directories like Homebrew (`/opt/homebrew/bin`, `/usr/local/bin`) or MacPorts (`/opt/local/bin`), which are omitted from non-login remote shell `$PATH` environments.

Bilterm wraps remote invocation in a sanitized login-shell command:
```bash
exec $SHELL -lc 'exec env PATH="$PATH:/opt/homebrew/bin:/usr/local/bin:/opt/local/bin" tmux -CC new-session -A -s NAME'
```
The session `NAME` is strictly validated against `[A-Za-z0-9_-]` (defaulting to `bilterm`), ensuring the remote command line contains exactly two single quotes and prevents command injection.

### 4.3 Interactive Connect Phase and Password Safety

Before `tmux -CC` starts, the SSH connection stream contains raw terminal text: OpenSSH host key verification questions, PAM password prompts, or 2FA challenge dialogues.

Bilterm handles this phase seamlessly:
1. **Primary Pane Display**: Before the DCS control sentinel (`ESC P 1000 p`) arrives, raw output lines from `ssh -tt` are rendered directly into the primary pane.
2. **Raw Keystroke Pass-Through**: While the link is `Connecting` and waiting for a password, host-key answer, or 2FA response, every keystroke goes only to the remote `ssh` process's PTY stdin (`set_link_raw_input`). The floating command prompt is hidden and nothing is echoed on screen.
3. **Password Security**: Typed passwords and passphrase responses never enter Bilterm's floating prompt buffer, undo/redo buffers, command blocks, history store (`history.log`), or Find search index. For unattended use, prefer key-based authentication with `ssh-agent` so no password is typed at all.
4. **Transition to Control Mode**: Once `ESC P 1000 p` is encountered (or fallback `%begin` / `%output`), Bilterm's `ControlLineSplitter` switches the stream to control lines and adopts the primary pane into the `tmux` layout.
5. **Remote-Only Panes**: A `:ssh-tmux` pane never spawns a local shell. Each keystroke is delivered to the remote host exactly once, so characters are not doubled and nothing runs locally.

### 4.4 Link States and Banners

The persistent link follows a formal finite state machine (`LinkState` in `src/tmux/link.rs`):

| State | Name (`as_str()`) | Banner Displayed | Interactive |
|-------|-------------------|------------------|-------------|
| `Idle` | `idle` | *(none)* | No |
| `Connecting` | `connecting` | `LINK CONNECTING <dest>/<session> \| type here for ssh prompts \| Ctrl+Alt+R: restart` | No (passwords pass through raw) |
| `Connected` | `connected` | *(none)* | No |
| `Backoff` | `reconnecting` | `LINK DOWN (<cause>) \| retry <attempt>/<max> in <secs>s \| Enter/r: now \| x: stop \| d: detach` | Yes |
| `Detached` | `detached` | `LINK STOPPED \| Enter/r: reconnect \| d: detach` | Yes |
| `Ended` | `ended` | `REMOTE SESSION ENDED \| Enter/r: start a new one \| d: detach` | Yes |
| `Failed` | `failed` | `LINK FAILED (<cause>) \| Enter/r: retry \| d: detach` | Yes |

Interactive banners allow single-key actions when the active pane has focus:
- `Enter` or `r` / `R`: Reconnect immediately (`:ssh-tmux reconnect`).
- `x` / `X`: Stop reconnecting and detach the link (`:ssh-tmux disconnect`).
- `d` / `D`: Detach Bilterm client (`:detach`).
- Left-click on the banner row: Triggers immediate reconnection.

### 4.5 Auto-Reconnect with Exponential Backoff

When a network blip or SSH disconnect occurs:
1. **Backoff Schedule**: Bilterm enters the `Backoff` state with a deterministic, capped exponential backoff:
   - Attempt 1: 1 s
   - Attempt 2: 2 s
   - Attempt 3: 4 s
   - Attempt 4: 8 s
   - Attempt 5: 16 s
   - Attempts 6–20: 30 s (capped)
2. **Attempt Reset**: After 30 seconds of stable `Connected` operation, the attempt counter resets to 0.
3. **Maximum Attempts**: If the link fails to reconnect after 20 attempts, it transitions to `Failed`.
4. **Never Retried**: Bilterm will not automatically reconnect if:
   - The user deliberately disconnected via `:ssh-tmux disconnect`, `Ctrl+X Q`, or `x` on the banner (`Detached`).
   - A fatal authentication failure occurred (wrong password, host key verification failed, or `tmux` binary missing on remote host).
   - The remote session cleanly exited or was destroyed (`Ended`).
5. **`%exit` Classification**:
   - Transient reasons (`server exited unexpectedly`, `lost tty`, `too far behind`, `connection`) trigger backoff reconnect.
   - Normal session exits (user typed `exit` in the last shell, or session was killed) transition to `Ended`.

### 4.6 Topology Hydration with `capture-pane`

This applies both to a first `:ssh-tmux <user@host> <session>` that attaches to an existing session (`tmux -CC new-session -A -s <session>`) and to reconnects:
1. **First pane immediately**: Bilterm maps the first remote pane to the local pane as soon as the control stream is up.
2. **Pane discovery**: It discovers the remaining panes with `list-panes`.
3. Bilterm waits 300 ms after attach greeting for pane layouts to settle.
4. It sends `refresh-client -C <cols>x<rows>` followed by `capture-pane -p -e -J -S -2000 -t %N` for each mapped pane.
5. Live `%output` lines are held during capture to prevent interleaved text corruption.
6. Screen lines and scrollback are parsed via `render_capture` and fed into each pane's `VtParser`.
7. Normal output is released. Stale panes that no longer exist on the remote host are flagged with orphan banners rather than being abruptly removed.
8. **Idle sessions**: Typed commands are delivered right away, even if the remote session has not produced any output yet (for example, an idle shell).

### 4.7 Single Link Guard and Fresh TCP Policy

- **One Link at a Time**: Bilterm currently supports one active remote `tmux -CC` link. Attempting to run `:ssh-tmux` while a link is `Connecting`, `Connected`, or in `Backoff` returns an error: `"a tmux link to <dest>/<session> is already active; run :ssh-tmux disconnect first"`.
- **Fresh TCP per Reconnect**: Because OpenSSH `ControlMaster` multiplexing is explicitly disabled (`ControlMaster=no`, `ControlPath=none`), each reconnect attempt opens an entirely fresh TCP connection. A dead socket on the host or local network cannot block subsequent retries.

### 4.8 Remote `tmux` Options That End a Session

If your remote session terminates immediately upon disconnect, verify your remote `~/.tmux.conf` on the host machine:
- `set -g exit-empty on`: By default `on`. If all windows close, the `tmux` server exits.
- `set -g destroy-unattached on`: If `on`, `tmux` destroys the session the instant Bilterm disconnects. Ensure this is `off` if you want sessions to persist across SSH disconnects!
- `set -g detach-on-destroy on`: Controls what happens when a destroyed session is exited.

### 4.9 Known Limits

- **`csh` / `tcsh` shells**: The remote wrapper relies on POSIX `sh`, `bash`, or `zsh` login shell syntax. C-shell variants are not supported.
- **Raw-mode capture**: `capture-pane` captures terminal grid text and colors, but cannot capture internal TUI cursor states or alternate-screen state machines of running programs (`vim`, `htop`).
- **Multi-link**: Only one remote `tmux -CC` bridge is active at a time in this version.
- **Window names**: Remote window names are hydrated when `%window-renamed` events arrive (gap G10 from Chapter 12).

---

## 5. Summary of Commands and Chords

| Command / Action | Keybinding | Menu Item | Purpose |
|------------------|------------|-----------|---------|
| `:ssh-tmux <user@host> [session]` | — | — | Spawn or attach persistent remote `tmux -CC` session over `ssh -tt` |
| `:ssh-tmux reconnect` | `Ctrl+Alt+R` / `Ctrl+X U` | `Reconnect link (Ctrl+Alt+R)` | Reconnect persistent remote tmux link immediately (skips backoff timer) |
| `:ssh-tmux disconnect` | `Ctrl+X Q` | `Disconnect link (Ctrl+X Q)` | Disconnect link; remote `tmux` session keeps running |
| `:ssh-tmux info` | — | `Link info` | Show link state, retry timer, last exit reason and mapped panes |
| `:ssh-tmux prefill` | `Ctrl+X u` | `SSH tmux link... (Ctrl+X u)` | Open prompt prefilled with `:ssh-tmux ` and the hint `Type <user@host> [session] and press Enter`. Works from any state: closes modals, leaves copy mode, and works while a link is waiting for a password |

---

[← Chapter 4: Themes & Config](04-themes-glyphs-and-config.md) | [User Guide Home](README.md) | [Chapter 6: Reference →](06-keyboard-mouse-and-command-reference.md)
