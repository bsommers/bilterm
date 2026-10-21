---
phase: 02-floating-prompt-ergonomics-triple-modality-intra-pane-controls-command-blocks
plan: 02
subsystem: clipboard-colon-commands-mouse-block-jumping
tags: [rust, decset-2004, osc-52, osc-133, mouse-drag, colon-commands]
provides:
  - `DECSET 2004` bracketed paste tracking (`VtParser::is_bracketed_paste`) and `handle_paste_event`
  - `OSC 52` clipboard encoding (`encode_osc52_clipboard`) and child PTY passthrough capture (`take_outbound_osc52`)
  - `OSC 133` command block scrollback jumping (`jump_to_prev_command_block`, `jump_to_next_command_block`)
  - `:command` parser and executor (`src/pane/colon_command.rs`)
  - SGR mouse drag/resize/pin routing (`handle_mouse_event`)
affects: [src/vt/mod.rs, src/pane/colon_command.rs, src/pane/mod.rs, src/input/mod.rs]
tech-stack:
  added: []
  patterns: [DECSET 2004 Bracketed Paste Framing, OSC 52 Base64 Clipboard Codec, Triple-Modality Intra-Pane Controls]
key-files:
  created: [src/pane/colon_command.rs]
  modified: [src/vt/mod.rs, src/pane/mod.rs, src/input/mod.rs]
key-decisions:
  - "Routed Event::Paste directly into FloatingPromptState::insert_str without auto-executing newlines in CommandPrompt mode, while wrapping in \\x1b[200~...\\x1b[201~ in RawPtyPassthrough mode when DECSET 2004 is active"
  - "Implemented triple-modality intra-pane controls across keyboard hotkeys (Alt+T/B/Up/Down/+/-/PageUp/PageDown), SGR mouse drag/click on prompt borders, and :command execution"
requirements-completed: [PTY-04, PANE-02, PANE-03, PANE-05]
duration: 4min
completed: 2026-10-20
---

# Phase 2 Plan 02: Bracketed Paste, OSC 52 Clipboard, OSC 133 Block Jumping & Triple-Modality Controls Summary

**Completed `DECSET 2004` bracketed paste, `OSC 52` clipboard encoding/passthrough, `OSC 133` command block scrollback jumping, `:command` parser, and mouse drag/resize/pin handlers.**

## Accomplishments
- Added `DECSET 2004` bracketed paste state tracking, `encode_osc52_clipboard`, `take_outbound_osc52`, and `jump_to_prev_command_block` / `jump_to_next_command_block` to `VtParser` (`src/vt/mod.rs`).
- Implemented `src/pane/colon_command.rs` (`:prompt top|bottom|float|height`, `:block prev|next`, `:yank`, `:clear`, `:raw`, `:quit`).
- Extended `src/input/mod.rs` with `handle_paste_event`, `handle_mouse_event` (`[▲][▼]` pin badges, top-border vertical drag, bottom-border height resize drag, wheel scrollback), and `Ctrl+R` fuzzy history keyboard routing.
