# Phase 2: Floating Prompt Ergonomics, Triple-Modality Intra-Pane Controls & Command Blocks - Research

**Researched:** 2026-10-20
**Status:** Complete
**Requirements Covered:** `PTY-04`, `PANE-02`, `PANE-03`, `PANE-04`, `PANE-05`

---

## 1. Phase 2 Technical Architecture

Phase 2 upgrades the single-line Phase 1 `FloatingPromptState` into a full multi-line, syntax-highlighted, fuzzy-history-capable shell composer with **triple-modality positioning/resizing** (keyboard, mouse drag/click, and `:command` execution), **clipboard & bracketed paste integration** (`DECSET 2004` + `OSC 52`), and **`OSC 133` command block scrollback navigation**.

### Key Subsystems
1. **Multi-Line Editor & Shell Syntax Highlighter (`src/pane/highlight.rs` + `src/pane/mod.rs`) (`PANE-04`)**:
   - Multi-line input buffer supporting `\n` line breaks (`Shift+Enter`, `Alt+Enter`, or Ctrl+J), line-aware cursor movement (`cursor_line_col()`, `move_cursor_up_or_history()`, `move_cursor_down_or_history()`), word deletion (`Ctrl+W`), kill-to-end (`Ctrl+K`), kill-line (`Ctrl+U`), and undo/redo history stack (`Ctrl+Z` / `Ctrl+Y`).
   - Deterministic, zero-C-build-overhead shell lexical/AST highlighter (`highlight_shell_input(input: &str, bg: Color) -> Vec<Line<'static>>`) classifying tokens into:
     - `TokenKind::Command` (first word of pipeline/subcommand, bold phosphor green)
     - `TokenKind::Flag` (`-f`, `--flag`, bright yellow)
     - `TokenKind::StringLiteral` (`'...'`, `"..."`, light cyan/amber)
     - `TokenKind::Variable` (`$VAR`, `${VAR}`, `$?`, bright cyan)
     - `TokenKind::Operator` (`|`, `||`, `&&`, `;`, `>`, `>>`, `<`, `2>&1`, bold magenta)
     - `TokenKind::Comment` (`#...`, dim gray)
     - `TokenKind::SyntaxError` (unclosed quote or trailing pipe error, red + underlined)
2. **Fuzzy History Search (`src/pane/fuzzy_history.rs`) (`PANE-04`)**:
   - Uses `nucleo_matcher::{Matcher, Config, Utf32Str}` to fuzzy-score prior commands in `FloatingPromptState::history` against the active search query (`Ctrl+R`).
   - Supports cycling through ranked matches (`Up`/`Down` or repeated `Ctrl+R`), accepting the match into the editor (`Enter` or `Tab`), or cancelling (`Esc`).
3. **Positioning Modes (`PANE-02`) & Triple-Modality Controls (`PANE-03`)**:
   - `PromptAnchor::Top`, `PromptAnchor::Bottom`, and `PromptAnchor::Floating { y_offset }`, plus `prompt.height` (`3..=12`, or auto-sized to `lines.len() + 2` up to `max_height`).
   - **Keyboard**: `Alt+T` (pin Top), `Alt+B` (pin Bottom), `Alt+Up`/`Alt+Down` (move floating prompt), `Alt+=` / `Alt+-` (grow/shrink prompt height), `Alt+[` / `Alt+]` (jump prev/next command block).
   - **Mouse (`src/input/mouse.rs` / `src/input/mod.rs`)**:
     - Clicking `[▲]` in the floating prompt title bar pins `Top`; clicking `[▼]` pins `Bottom`.
     - Dragging the top border of the floating prompt updates `PromptAnchor::Floating { y_offset }` in real time (snapping to `Top` when dragged to row `0`).
     - Dragging the bottom border of the floating prompt resizes `prompt.height` in real time.
     - Scroll wheel over `OutputCanvas` scrolls `VtParser` scrollback up/down.
   - **`:command` Mode (`src/pane/colon_command.rs`)**:
     - Submitting a command starting with `:` (e.g., `:prompt top`, `:prompt bottom`, `:prompt float 8`, `:prompt height 5`, `:block prev`, `:block next`, `:yank`, `:clear`, `:raw`, `:quit`) executes the internal window/pane control command without sending it to the shell PTY (unless escaped or in raw mode).
4. **Clipboard & Bracketed Paste (`PTY-04`)**:
   - `VtParser` tracks `DECSET 2004` (`\x1b[?2004h` / `\x1b[?2004l` -> `is_bracketed_paste_enabled()`) and parses incoming `OSC 52` clipboard sequences (`\x1b]52;c;<base64>\x07` or `\x1b\\`) into `last_osc52_clipboard()`.
   - Provides base64 encoder/decoder and `format_osc52_sequence(text: &str) -> Vec<u8>` so yanked/copied text (`:yank` or `Alt+W`) updates both `CompositePane::clipboard` and emits an `OSC 52` sequence.
   - Handles `crossterm::event::Event::Paste(text)`: inserts multi-line text into `FloatingPromptState` in `CommandPrompt` mode, or writes `\x1b[200~{text}\x1b[201~` (when `DECSET 2004` is active) to the PTY in `RawPtyPassthrough` mode.
5. **OSC 133 Command Block Separation & Jumping (`PANE-05`)**:
   - `VtParser` records `CommandBlockRecord`s with line positions so `CompositePane::jump_to_prev_command_block()` and `jump_to_next_command_block()` compute the corresponding scrollback offset and focus the selected command block (`selected_block_index()`).
