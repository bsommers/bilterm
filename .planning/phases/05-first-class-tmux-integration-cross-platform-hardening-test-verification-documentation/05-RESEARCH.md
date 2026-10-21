# Phase 5: First-Class `tmux` Integration, Cross-Platform Hardening, Test Verification & Documentation - Research

**Researched:** 2026-10-20
**Status:** Complete

## 1. Outer `$TMUX` Detection & `DCS tmux;` Passthrough (`TMUX-01`)
- `$TMUX` has the format `<socket_path>,<pid>,<session_index>`. `TmuxEnv::from_env_str(val: Option<&str>)` parses `socket_path`, `server_pid`, and `session_id`.
- Wrapping an escape sequence (`seq`, such as `\x1b]52;c;...\x07`) for `tmux` passthrough (`allow-passthrough on`) requires doubling every `\x1b` byte inside `seq` and enclosing it in `\x1bPtmux;...\x1b\\`.

## 2. `tmux -CC` Control Mode Protocol (`TMUX-02`)
- In `tmux -CC` control mode, `tmux` emits line-oriented notifications:
  - `%output %<pane_id> <octal_escaped_bytes>`: characters with ASCII `< 32` or `\` are octal-escaped as `\ooo` (3 octal digits, e.g. `\033` for `ESC`, `\015` for `\r`, `\012` for `\n`, `\134` for `\`).
  - `%window-add @<window_id>`
  - `%layout-change @<window_id> <layout_string>`
  - `%session-changed $<session_id> <name>`
  - `%begin <time> <num> <flags>` / `%end <time> <num> <flags>`
- Commands sent to `tmux -CC` stdin:
  - `send-keys -t %<pane_id> -H <hex_bytes>\n` (sends exact command bytes + `0a` newline from Bilterm's `FloatingCommandPrompt` to `%pane_id` without shell quoting hazards!)
  - `split-window -h -t %<pane_id>\n` / `split-window -v -t %<pane_id>\n`
  - `refresh-client -C <cols>,<rows>\n`
