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

## 2. Mode B: Controlling `tmux` via `tmux -CC` Control Mode (`TMUX-02`)

Bilterm's `TmuxControlBridge` parses `tmux -CC` Control Mode notifications:
- **`%output %<pane_id> <octal-escaped-bytes>`**: Unescapes `tmux` octal sequences (`\033`, `\015`, `\012`) and feeds each `%pane_id` stream into its mapped Bilterm `CompositePane` `VtParser`.
- **`%window-add`, `%layout-change`, `%session-changed`**: Synchronizes `tmux` windows and panes with Bilterm's `WindowManager`.
- **Hex `send-keys` Dispatch**: Commands submitted from Bilterm's `FloatingCommandPrompt` are formatted via `format_send_keys_hex` (`send-keys -t %<id> -H <hex>`) so every `tmux` pane gains Bilterm's local floating prompt overlay while retaining `tmux` session persistence across disconnects.

---

[← Chapter 4: Themes & Config](04-themes-glyphs-and-config.md) | [User Guide Home](README.md) | [Chapter 6: Reference →](06-keyboard-mouse-and-command-reference.md)
