---
id: 20261020-split-prompt-borders-shadows-vt-torture-coverage
type: quick
status: complete
completed_at: 2026-10-20
---

# Quick Task Summary: Split Command/Output Panes, Selectable Border Outlines, Themeable DOS Drop Shadows, Full VT Control Sequence Compliance & Test Coverage / Torture Suite

## Delivered Changes

1. **Non-Overlapping Split Command & Output Pane Modes ([`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs), [`src/pane/colon_command.rs`](file:///home/bill/src/ai/bilterm/src/pane/colon_command.rs))**:
   - Added [`PromptAnchor::SplitTop`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs#L17-L28) and [`PromptAnchor::SplitBottom`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs#L17-L28) alongside floating modes (`Top`, `Bottom`, `Floating { y_offset }`).
   - In split modes, [`CompositePane::canvas_rect`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs#L368-L406) and [`CompositePane::prompt_rect`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs#L416-L462) partition the composite viewport into two strictly disjoint, zero-overlap rectangles (`prompt_rect.intersection(canvas_rect).area() == 0`) and resize the underlying PTY/VT100 grid to match the dedicated output region.
   - Entering `DECSET 1049` alternate screen (`vim`, `htop`) automatically expands the canvas back to full height and restores the split when exiting.
   - Accessible via `Alt+S`, `:prompt split-top`, `:prompt split-bottom`, `:prompt split`, or `default_prompt_anchor = "split-top" | "split-bottom"` in `config.toml`.

2. **Selectable Window & Pane Outline Styles ([`src/ui/chrome.rs`](file:///home/bill/src/ai/bilterm/src/ui/chrome.rs), [`src/config/mod.rs`](file:///home/bill/src/ai/bilterm/src/config/mod.rs))**:
   - Added [`OutlineStyle`](file:///home/bill/src/ai/bilterm/src/ui/chrome.rs#L8-L78) supporting 8 outline styles: `Double` (`╔═╗`), `Solid` (`┌─┐`), `Rounded` (`╭─╮`), `Thick` (`┏━┓`), `Dotted` (`┌┈┐`), `Dashed` (`┌┄┐`), `Quadruple` (`▛▀▜`), and `Ascii` (`+-+`).
   - Configurable independently for workspace windows (`window_outline`) and command prompts (`prompt_outline`) in `config.toml` or live via `:outline [window|prompt] <style>` and `:border <style>`.

3. **Themeable & Selectable DOS-Style Drop Shadows ([`src/ui/chrome.rs`](file:///home/bill/src/ai/bilterm/src/ui/chrome.rs), [`src/ui/theme.rs`](file:///home/bill/src/ai/bilterm/src/ui/theme.rs))**:
   - Added [`ShadowStyle`](file:///home/bill/src/ai/bilterm/src/ui/chrome.rs#L80-L115): `None`, `Translucent` (dims underlying cells without erasing text), `ThemeTinted` (tints underlying cells using theme `shadow_fg` / `shadow_bg`), and `SolidHalfBlock` (classic DOS `█` solid shadow).
   - Added `shadow_fg` and `shadow_bg` to [`ThemePalette`](file:///home/bill/src/ai/bilterm/src/ui/theme.rs#L40-L56) and `theme.ini` parser.
   - Configurable via `shadow_style` in `config.toml` or live via `:shadows <on|off|none|translucent|tinted|solid>`.

4. **Full Control Sequence Compliance ([`src/vt/mod.rs`](file:///home/bill/src/ai/bilterm/src/vt/mod.rs), [`src/input/mod.rs`](file:///home/bill/src/ai/bilterm/src/input/mod.rs))**:
   - Added tracking and handling for `DECCKM` (`?1h`/`?1l` application cursor keys `\x1bOA..D`, `\x1bOH`, `\x1bOF` in [`encode_key_to_pty_bytes_with_modes`](file:///home/bill/src/ai/bilterm/src/input/mod.rs#L588-L671)), mouse tracking modes (`?1000`, `?1002`, `?1003`, `?1006`), focus reporting (`?1004`), and synchronized output (`?2026`).
   - Added `OSC 0` / `OSC 2` dynamic window title updates, `OSC 7` working directory reporting (`\x1b]7;file://host/path\x07` integrated into tab completion CWD resolution), and `DSR` (`CSI 5 n` -> `\x1b[0n`), `CPR` (`CSI 6 n` -> `\x1b[row;colR`), and `DA1` (`CSI c` -> `\x1b[?62;22c`) query response writeback.

5. **Comprehensive Test Coverage Audit & Terminal Torture / Stress Suite ([`tests/terminal_torture_and_stress.rs`](file:///home/bill/src/ai/bilterm/tests/terminal_torture_and_stress.rs), [`docs/TEST_COVERAGE.md`](file:///home/bill/src/ai/bilterm/docs/TEST_COVERAGE.md))**:
   - Increased line coverage from `73.22%` to **`81.83%`** (`3913/4782` lines) and function coverage from `75.16%` to **`83.00%`** (`288/347` functions) across 53 total tests.
