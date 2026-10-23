# Chapter 5: First-Class `tmux` Integration

[← Back to Project README](../../README.md) | [User Guide Home](README.md) | [← Chapter 4: Themes & Config](04-themes-glyphs-and-config.md) | [Chapter 6: Reference →](06-keyboard-mouse-and-command-reference.md)

## Table of Contents

- [1. Mode A: Running Bilterm *Inside* an Outer `tmux` Session (`TMUX-01`)](#1-mode-a-running-bilterm-inside-an-outer-tmux-session-tmux-01)
- [2. Mode B: Controlling `tmux` via `tmux -CC` Control Mode (`TMUX-02`)](#2-mode-b-controlling-tmux-via-tmux--cc-control-mode-tmux-02)

---

Bilterm supports two complementary `tmux` operating modes (`src/tmux/mod.rs`):

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

## 3. Mode C: Remote SSH Domains, Persistent Remote `tmux -CC` & `herdr` Fleet Federation (`v1.4.0`)

Bilterm `v1.4.0` extends `tmux` and composite pane multiplexing across remote SSH hosts (`src/remote/mod.rs`):
- **First-Class SSH Composite Panes (`:ssh [split h|v] <user@host[:port]> [cmd]`)**: Spawns an OpenSSH session with `ServerAliveInterval=15` inside the focused or newly split pane and displays `SSH:<user@host>` in the 3270 OIA status bar while keeping the floating command prompt completely local and zero-latency. Destinations are strictly validated (no leading `-`, no whitespace/control characters) and arguments are separated with `--` before the destination. SSH connection multiplexing sockets are placed safely in Bilterm's private runtime directory (`ControlPath=<private_dir>/ssh-%C`).
- **Persistent Remote `tmux -CC` Over SSH (`:ssh-tmux <user@host[:port]> [session]`)**: Launches `ssh -tt <user@host> tmux -CC new-session -A -s <session>` via `TmuxControlSubprocess::spawn_remote_ssh_attach_or_new`, hydrating remote `tmux` panes directly into local Bilterm Composite Panes that survive laptop sleep or network disconnects.
- **`herdr`-Style Central Fleet Federation (`:remote add|list|connect|exec|disconnect`)**: Connects to remote `bilterm --headless` daemons across your infrastructure (remote daemons require `--allow-remote --token-file <path>`) and forwards JSON-RPC 2.0 requests (`remote.exec` / `:remote exec <name> <method> [json]`) from a single central Bilterm dashboard.

---

[← Chapter 4: Themes & Config](04-themes-glyphs-and-config.md) | [User Guide Home](README.md) | [Chapter 6: Reference →](06-keyboard-mouse-and-command-reference.md)
