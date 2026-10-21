---
id: 20261020-prompt-width-scroll-running-cmd-history-help-docs
type: quick
status: complete
completed_at: 2026-10-20
---

# Quick Task Summary: Resizable/Scrollable Prompt, Running Command Banner, Rich Status Bar & R/C/Output Metrics, History Popup & Session Persistence, Full-Screen Cross-Hairs, `<F1>` Help Modal & Full User Documentation

## Delivered Changes

1. **Resizable Command Prompt Width & Scrollable Content ([`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [`src/input/mod.rs`](file:///home/bill/src/ai/bilterm/src/input/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs))**:
   - Added `width_cols`, `x_offset`, `scroll_offset`, and `h_scroll_offset` to `FloatingPromptState`.
   - Resizable width via keyboard (`Alt+]` / `Alt+[`), `:prompt width <cols|full>`, and mouse dragging of the left border, right border, or bottom-right corner (`◢`).
   - Vertical and horizontal scrolling inside the prompt (`Shift+PageUp` / `Shift+PageDown`, mouse wheel over the prompt box, and right-border scrollbar indicator `█`/`░`).

2. **Pending / Running Command Banner ([`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs))**:
   - Submitting a command with `Enter` (e.g., `tail -f /dev/null`) records `running_command` on `FloatingPromptState` and renders a dedicated `⟳ RUNNING:` / `● LAST CMD:` banner row inside the command prompt box while keeping a fresh input line below it.

3. **Rich Status Bar Icons, Accurate `R:xx C:xxx` Counter & Output Length Telemetry ([`src/ui/glyphs.rs`](file:///home/bill/src/ai/bilterm/src/ui/glyphs.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs), [`src/vt/mod.rs`](file:///home/bill/src/ai/bilterm/src/vt/mod.rs))**:
   - Added status icons (`● READY`, `⟳ RUNNING`, `⇳ SCROLL`, `▣ ALT SCR`, `♦ !ECHO LOCK`, `⌖ R:xx C:xxx`, `≣ OUT:<lines>L/<bytes>`) across all 4 glyph packs.
   - `R:xx C:xxx` dynamically reflects the active command prompt cursor `(cursor_row + 1, cursor_col + 1)` in `INPUT: CMD BOX` mode and the VT100 cursor `(vt_row + 1, vt_col + 1)` in `INPUT: RAW PTY` mode.

4. **Command History Pop-up Window, Session Persistence & Temp-File Export ([`src/pane/history_store.rs`](file:///home/bill/src/ai/bilterm/src/pane/history_store.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs))**:
   - Implemented `HistoryStore` persisting commands by default with ISO-8601 UTC timestamp, unique `session_id` (`<epoch>-<pid>-<seq>`), and `pane_id`, plus automatic file rotation (`history_max_files`).
   - Added Borland Command History Pop-up Window (`Alt+P` or `:history`) with `Up`/`Down` selection, `Enter` recall, and `s` / `:history save [path]` export to a temporary `.sh` script file.

5. **Full-Screen vs. Pane-Local Cross-Hairs & `<F1>` Help Modal ([`src/wm/mod.rs`](file:///home/bill/src/ai/bilterm/src/wm/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs))**:
   - Added `Alt+X` and `:crosshair full | pane | on | off` (`crosshair_fullscreen` in `config.toml`) to toggle full-screen vs. pane-local IBM 3270 cross-hairs.
   - Added `<F1>` / `:help` scrollable Borland Turbo Vision Help Modal.

6. **Comprehensive User Documentation ([`docs/user-guide/`](file:///home/bill/src/ai/bilterm/docs/user-guide/index.md), [`README.md`](file:///home/bill/src/ai/bilterm/README.md))**:
   - Updated the User Manual and reference tables covering all features.
