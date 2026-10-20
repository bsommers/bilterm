# Phase 2: Floating Prompt Ergonomics, Triple-Modality Intra-Pane Controls & Command Blocks - Context

**Gathered:** 2026-10-20
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped via workflow.skip_discuss)

<domain>
## Phase Boundary

As a terminal user, I want to edit multi-line syntax-highlighted commands, search fuzzy history, position or resize the floating prompt via keyboard/mouse/`:command`, use system clipboards, and jump between `OSC 133` command blocks, so that I can compose and navigate shell commands ergonomically inside any composite pane.

**Requirements covered:** `PTY-04`, `PANE-02`, `PANE-03`, `PANE-04`, `PANE-05`

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — discuss phase was skipped per user setting. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions:
- **Multi-Line Command Editing & Live Shell Syntax Highlighting (`PANE-04`)**:
  - Support multi-line command editing inside `FloatingPromptState` (`Shift+Enter` or `Alt+Enter` or trailing `\` continuation inserts a newline, `Up`/`Down` navigates lines when multi-line or cycles history on top/bottom line, `Left`/`Right`/`Home`/`End`/`Ctrl+W`/`Ctrl+U`/`Ctrl+K` edit across lines, plus undo/redo stack).
  - Implement a fast, zero-external-C-toolchain-fragility lexical/AST shell syntax highlighter (`src/pane/highlight.rs`) that tokenizes and styles bash/shell commands (`Command` in bold green/cyan, `Flags` `--flag`/`-f` in yellow, `Strings` `'...'`/`"..."` in light green/amber, `Variables` `$VAR`/`${VAR}` in cyan, `Operators/Pipes` `|`, `&&`, `||`, `;`, `>`, `>>`, `<` in magenta/yellow, `Comments` `#...` in dim gray, and unclosed quote syntax errors in red underline) into `ratatui::text::Line` spans in sub-millisecond time.
- **Fuzzy History Search (`Ctrl+R` + `nucleo-matcher`) (`PANE-04`)**:
  - Pressing `Ctrl+R` inside `FloatingCommandPrompt` opens/activates an interactive fuzzy history search mode powered by `nucleo-matcher 0.3`. Typing filters and ranks command history by fuzzy score; `Up`/`Down` or `Ctrl+R` selects matches; `Enter` accepts the match into the prompt; `Esc` cancels.
- **Positioning Modes (`PANE-02`) & Triple-Modality Controls (`PANE-03`)**:
  - `PromptAnchor::Top` (IBM 3270 default), `PromptAnchor::Bottom`, and `PromptAnchor::Floating { y_offset }`, plus customizable prompt `height` (`3..=12` rows, auto-expanding with multi-line input up to `max_height`).
  - **Modality 1 (Keyboard)**: `Alt+T` (`Top`), `Alt+B` (`Bottom`), `Alt+Up`/`Alt+Down` (move floating prompt up/down), `Alt+PageUp`/`Alt+PageDown` or `Alt+Plus`/`Alt+Minus` (resize floating prompt height).
  - **Modality 2 (Mouse)**: Clicking and dragging the floating prompt's top border / title bar moves it vertically (`PromptAnchor::Floating { y_offset }`, snapping to `Top` at `y=0` or `Bottom` at bottom row); dragging the bottom border of the prompt resizes `prompt.height`; clicking `[▲]` / `[▼]` border widgets pins `Top` / `Bottom`; mouse wheel over `OutputCanvas` scrolls scrollback history.
  - **Modality 3 (`:command` mode)**: Entering `:prompt top`, `:prompt bottom`, `:prompt float <row>`, `:prompt height <rows>`, `:clear`, `:raw`, `:block prev`, `:block next`, or `:q` / `:quit` inside the floating prompt executes the corresponding Bilterm control command directly.
- **Clipboard & Bracketed Paste (`PTY-04`)**:
  - Track `DECSET 2004` (`\x1b[?2004h` / `\x1b[?2004l`) bracketed paste mode in `VtParser`.
  - Handle `crossterm::event::Event::Paste(String)`: in `CommandPrompt` mode, insert the pasted text (preserving multi-line newlines) into `FloatingPromptState`; in `RawPtyPassthrough` mode, wrap in `\x1b[200~...\x1b[201~` if `DECSET 2004` is enabled on the child PTY (or raw bytes otherwise).
  - Support `OSC 52` clipboard sequence parsing (`\x1b]52;c;<base64>\x07` / `\x1b\\`) in `VtParser` and provide an internal clipboard buffer + `OSC 52` sequence emitter (`format_osc52_clipboard_sequence`) for host terminal copy/paste (`Ctrl+Shift+C` / `Ctrl+Y` yank / `:yank`).
- **Command Block Separation & Scrollback Jumping (`PANE-05`)**:
  - Track every command submission and `OSC 133;A/B/C/D` boundary as a `CommandBlockRecord` with `index`, `command`, `scrollback_line_Estimate` / `absolute_row`, and `exit_code`.
  - Provide `jump_to_prev_command_block()` (`Alt+[` or `:block prev`) and `jump_to_next_command_block()` (`Alt+]` or `:block next`) on `CompositePane` to jump the `VtParser` scrollback viewport directly between prior commands and display the active command block index in the OIA status bar.

</decisions>

<code_context>
## Existing Code Insights

- `src/vt/mod.rs`: Already tracks `Osc133Phase` and `CommandBlockRecord`. Needs `DECSET 2004` (`bracketed_paste_mode`) tracking, `OSC 52` clipboard extraction, and block-indexed scrollback offset calculation.
- `src/pane/mod.rs`: Already defines `PromptAnchor`, `FloatingPromptState`, and `CompositePane`. Needs multi-line cursor/editing, fuzzy history (`nucleo-matcher`), `:command` dispatcher, mouse drag/resize state machine, and command block jump methods.
- `src/ui/mod.rs`: Renders `CompositePaneWidget`. Needs syntax-highlighted multi-line prompt rendering, `[▲][▼]` border pin widgets, fuzzy history overlay/bar when `Ctrl+R` is active, and command-block telemetry in OIA.

</code_context>

<specifics>
## Specific Ideas

All 5 Phase 2 requirements (`PTY-04`, `PANE-02`, `PANE-03`, `PANE-04`, `PANE-05`) must be covered with unit tests and `insta` snapshots.

</specifics>

<deferred>
## Deferred Ideas

Multi-pane H/V splits, z-order stacking, and Guake drop-down terminal are in Phase 3.

</deferred>
