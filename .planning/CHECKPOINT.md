# Context Checkpoint — Bilterm v1.0.0

**Updated:** 2026-10-20
**Skill:** `compacting-context-safely`

## 1. Goal & Scope
- **Completed:** Bilterm v1.0.0 (Phases 1–5, 15/15 plans, 26/26 requirements, 48/48 tests passing, 0 clippy warnings).
- **Non-goals for v1.0:** Custom GPU shader pipeline (relying on host terminal emulator + Ratatui cell buffer compositing), serial/TN3270 mainframe network protocol client.

## 2. Settled Architectural Decisions (Do Not Re-litigate)
- **Zero-`SIGWINCH` Floating Prompt ([ADR-001](file:///home/bill/src/ai/bilterm/docs/architecture/adr/ADR-001-composite-pane-zero-sigwinch.md)):** `CompositePane::compute_geometry()` always returns `pty_rect == inner_rect`. Moving, resizing, or pinning the floating prompt never sends `TIOCSWINSZ` (`SIGWINCH`) to the child PTY.
- **3-Signal Line-Discipline Detector ([ADR-002](file:///home/bill/src/ai/bilterm/docs/architecture/adr/ADR-002-three-signal-line-discipline.md)):** Combines `DECSET 1049/1047/47` (`ALT_SCREEN`), `OSC 133;A..D` shell markers, and POSIX `tcgetpgrp` + `tcgetattr` (`!ECHO` / `!ICANON`) to auto-switch between `FloatingPrompt` and `RawPassthrough` (preventing password leakage in `sudo`/`ssh`).
- **2-Column Wide-Char Boundary Sanitizer ([ADR-003](file:///home/bill/src/ai/bilterm/docs/architecture/adr/ADR-003-wide-char-occlusion-sanitizer.md)):** `occlude_rect` clears orphaned half-cells at `rect.x` and `rect.right() - 1` before rendering any floating prompt, stacked window, drop shadow, or Guake drop-down overlay.
- **Borland Turbo Vision Chrome & IBM 3270 Cross-Hair ([ADR-004](file:///home/bill/src/ai/bilterm/docs/architecture/adr/ADR-004-borland-turbo-vision-and-3270-cursor.md)):** 5 color themes (`turbo-blue`, `turbo-cyan`, `ibm-3278-green`, `ibm-3279-amber`, `ibm-mono`) and 4 glyph profiles (`unicode`, `cp437`, `nerd-font`, `ascii`).
- **Dual-Mode Tmux Integration ([ADR-005](file:///home/bill/src/ai/bilterm/docs/architecture/adr/ADR-005-dual-mode-tmux-integration.md)):** Mode A (`$TMUX` coexistence + `DCS tmux;` passthrough + `Ctrl+A` prefix adjustment) and Mode B (`tmux -CC` control-mode parser & `send-keys -H` router).

## 3. Working-Tree State
- **Branch:** `main`
- **Remote:** `origin` (`https://github.com/bsommers/bilterm.git`)
- **Key Entry Points:**
  - [src/lib.rs](file:///home/bill/src/ai/bilterm/src/lib.rs), [src/main.rs](file:///home/bill/src/ai/bilterm/src/main.rs)
  - [src/pty/mod.rs](file:///home/bill/src/ai/bilterm/src/pty/mod.rs), [src/pty/line_discipline.rs](file:///home/bill/src/ai/bilterm/src/pty/line_discipline.rs)
  - [src/vt/mod.rs](file:///home/bill/src/ai/bilterm/src/vt/mod.rs)
  - [src/pane/mod.rs](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [src/pane/highlight.rs](file:///home/bill/src/ai/bilterm/src/pane/highlight.rs), [src/pane/fuzzy_history.rs](file:///home/bill/src/ai/bilterm/src/pane/fuzzy_history.rs), [src/pane/colon_command.rs](file:///home/bill/src/ai/bilterm/src/pane/colon_command.rs)
  - [src/wm/mod.rs](file:///home/bill/src/ai/bilterm/src/wm/mod.rs), [src/wm/split_tree.rs](file:///home/bill/src/ai/bilterm/src/wm/split_tree.rs), [src/wm/z_stack.rs](file:///home/bill/src/ai/bilterm/src/wm/z_stack.rs), [src/wm/quick_terminal.rs](file:///home/bill/src/ai/bilterm/src/wm/quick_terminal.rs)
  - [src/ui/mod.rs](file:///home/bill/src/ai/bilterm/src/ui/mod.rs), [src/ui/theme.rs](file:///home/bill/src/ai/bilterm/src/ui/theme.rs), [src/ui/glyphs.rs](file:///home/bill/src/ai/bilterm/src/ui/glyphs.rs), [src/ui/chrome.rs](file:///home/bill/src/ai/bilterm/src/ui/chrome.rs), [src/ui/cursor.rs](file:///home/bill/src/ai/bilterm/src/ui/cursor.rs), [src/ui/status_bar.rs](file:///home/bill/src/ai/bilterm/src/ui/status_bar.rs), [src/ui/occlusion.rs](file:///home/bill/src/ai/bilterm/src/ui/occlusion.rs)
  - [src/config/mod.rs](file:///home/bill/src/ai/bilterm/src/config/mod.rs)
  - [src/tmux/mod.rs](file:///home/bill/src/ai/bilterm/src/tmux/mod.rs)

## 4. Verification Command
```bash
cargo test && cargo clippy --all-targets -- -D warnings
```
- **Expected:** 48 passed (9 unit + 39 integration/snapshot across 5 test suites), 0 failed, 0 clippy warnings.

## 5. Next Steps (Resuming in a Fresh Session)
1. Read [.planning/STATE.md](file:///home/bill/src/ai/bilterm/.planning/STATE.md), [.planning/CHECKPOINT.md](file:///home/bill/src/ai/bilterm/.planning/CHECKPOINT.md), and [docs/architecture/OVERVIEW.md](file:///home/bill/src/ai/bilterm/docs/architecture/OVERVIEW.md).
2. Run `cargo test` to confirm baseline health.
3. For v1.1 milestone work (e.g., interactive mouse border dragging for split/stacked windows, shell completion menu popup inside the floating prompt, or custom keybinding remapping in `config.toml`), start via `/gsd-new-milestone` + Superpowers `brainstorming` and `test-driven-development`.
