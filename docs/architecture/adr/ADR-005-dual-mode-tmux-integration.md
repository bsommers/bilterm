# ADR-005: Dual-Mode `tmux` Coexistence & `tmux -CC` Control Mode Bridge

- **Status:** Accepted
- **Date:** 2026-10-20

## Context
Users run Bilterm both **inside** an existing outer `tmux` session (`$TMUX` set) and as a **frontend** attached to persistent `tmux` sessions via Control Mode (`tmux -CC`).

## Decision
`src/tmux/mod.rs` implements a dual-mode architecture:
- **Mode A (Outer `$TMUX` Coexistence)**: `TmuxEnv::detect()` inspects `$TMUX`, avoids `Ctrl+B` prefix collisions by binding Bilterm window management to `Alt+` chords and `:command`s, and wraps outbound `OSC 52` clipboard sequences in `\x1bPtmux;\x1b...\x1b\\` via `wrap_tmux_dcs_passthrough`.
- **Mode B (`tmux -CC` Control Mode Bridge)**: `parse_tmux_control_line` and `unescape_tmux_output` decode `%output %<id> <octal>` notifications and pipe byte streams directly into `WindowManager` `CompositePane`s via `TmuxControlBridge`, while `format_send_keys_hex` dispatches floating prompt submissions via `send-keys -t %<id> -H <hex>`.

## Consequences
- Bilterm coexists cleanly inside nested `tmux` sessions and can attach its floating command prompts to persistent `tmux -CC` panes.
