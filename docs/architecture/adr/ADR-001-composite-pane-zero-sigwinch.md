# ADR-001: CompositePane Zero-SIGWINCH Overlay Geometry

- **Status:** Accepted
- **Date:** 2026-10-20

## Context
If the `FloatingCommandPrompt` and `OutputCanvas` were laid out as two adjacent tiles that resized the child PTY (`TIOCSWINSZ` / `SIGWINCH`) whenever the user moved, resized, or hid the prompt, full-screen and line-oriented programs (`vim`, `htop`, `git log`) would suffer constant geometry thrashing, text reflow jitter, and corrupted scrollback.

## Decision
Every `CompositePane` sizes its `VtParser` and `PtySession` strictly to `canvas_rect` (`pane_area.height - 1` rows by `pane_area.width` columns, reserving only the bottom 1 row for the `OiaStatusBar`). The `FloatingCommandPrompt` is rendered strictly as a second-pass overlay (`occlude_rect` + `render_floating_prompt`) on top of `OutputCanvas`.

## Consequences
- Moving (`Alt+T`, `Alt+B`, `Alt+Up`/`Down`, mouse drag), resizing (`Alt+Plus`/`Minus`), or auto-hiding (`DECSET 1049`) the floating prompt emits **zero `SIGWINCH` signals** to the child PTY.
- Terminal output scrolls continuously across 100% of the canvas height underneath the floating command prompt.
