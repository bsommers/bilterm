---
id: 20261020-prompt-width-scroll-running-cmd-history-help-docs
type: quick
status: in_progress
created_at: 2026-10-20
---

# Quick Plan: Resizable/Scrollable Prompt, Running Command Banner, Rich Status Bar & R/C/Output Metrics, History Popup & Session Persistence, Full-Screen Cross-Hairs, `<F1>` Help Modal & Full User Documentation

## Tasks

1. **VT100 Output Byte/Line Counters ([`src/vt/mod.rs`](file:///home/bill/src/ai/bilterm/src/vt/mod.rs))**:
   - Track `total_bytes_processed: usize` alongside `total_lines_scrolled: usize` in `VtParser`, exposing `total_bytes_processed()`, `total_lines_scrolled()`, and `output_line_count()`.

2. **Persistent Command History Store & Temp-File Export ([`src/pane/history_store.rs`](file:///home/bill/src/ai/bilterm/src/pane/history_store.rs), [`src/config/mod.rs`](file:///home/bill/src/ai/bilterm/src/config/mod.rs))**:
   - Add `history_enabled: bool` (default `true`), `history_max_files: usize` (default `25`), `history_dir: Option<String>`, and `crosshair_fullscreen: bool` (default `false`) to `BiltermConfig`.
   - Implement `HistoryStore`:
     - Generates a unique `session_id` (`<epoch_secs>-<pid>`) and per-pane session file `history-session-<session_id>-pane-<pane_id>.log` with `[<epoch_ts> | session=<session_id> | pane=<pane_id>] <command>` entries.
     - Automatically rotates/prunes oldest session files when count exceeds `history_max_files`.
     - Provides `export_commands_to_temp_file(pane_id, session_id, commands) -> std::io::Result<PathBuf>` for `:history save [path]`.

3. **Resizable Width, Scrollable Content, Running Command Banner & History Popup in Prompt ([`src/pane/mod.rs`](file:///home/bill/src/ai/bilterm/src/pane/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs))**:
   - Add `width_cols: Option<u16>`, `x_offset: Option<u16>`, `scroll_offset: usize`, `h_scroll_offset: usize`, `running_command: Option<String>`, `history_popup_open: bool`, and `history_popup_selected: usize` to `FloatingPromptState`.
   - Support mouse dragging of left/right borders (`ResizePromptLeft`, `ResizePromptRight`) and bottom-right corner (`ResizePromptCorner`), plus `Alt+[` / `Alt+]` and `:prompt width <cols|full>`.
   - Support vertical and horizontal scrolling inside the prompt (`keep_cursor_in_view`, `scroll_prompt_up`, `scroll_prompt_down`, scrollbar thumb `█`/`░` on right border).
   - Keep `running_command` displayed on a dedicated `⟳ RUN:` banner inside the command prompt after `Enter` while a foreground command (`tail -f /dev/null`, etc.) is running.
   - Render a Borland-style **Command History Pop-up Window** (`Alt+H` or `:history` / `:hist`) with selectable history entries and file-persistence status.

4. **Rich Status Bar Glyphs, Accurate `R:`/`C:` Counter & Output Length Indicator ([`src/ui/glyphs.rs`](file:///home/bill/src/ai/bilterm/src/ui/glyphs.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs))**:
   - Add status bar icons (`status_ready_icon`, `status_run_icon`, `status_lock_icon`, `status_alt_icon`, `status_scroll_icon`, `output_len_icon`, `cursor_pos_icon`) to `GlyphSet`.
   - Make `R:xx C:xxx` reflect the active editor cursor `(cursor_row + 1, cursor_col + 1)` when in `CommandPrompt` mode and the live VT100 cursor `(vt_row + 1, vt_col + 1)` when in `RawPtyPassthrough` mode.
   - Display `OUT:<lines>L/<bytes>` (e.g., `≣ OUT:42L/1.8K`) on the OIA status bar.

5. **Full-Screen vs Pane-Local Cross-Hairs & `<F1>` Interactive Help Modal ([`src/wm/mod.rs`](file:///home/bill/src/ai/bilterm/src/wm/mod.rs), [`src/ui/mod.rs`](file:///home/bill/src/ai/bilterm/src/ui/mod.rs), [`src/input/mod.rs`](file:///home/bill/src/ai/bilterm/src/input/mod.rs))**:
   - Support `:crosshair on | off | full | pane` and `Alt+X` to toggle the IBM 3270 cross-hair cursor ruler (either pane-local or full-screen across the entire terminal buffer).
   - Add `<F1>` / `:help` interactive Borland Turbo Vision Help Modal (`help_visible`, `help_scroll` on `WindowManager`), scrollable with `Up`/`Down`/`PageUp`/`PageDown` and dismissible with `F1` or `Esc`.

6. **Comprehensive User Documentation ([`docs/user-guide/`](file:///home/bill/src/ai/bilterm/docs/user-guide/index.md), [`README.md`](file:///home/bill/src/ai/bilterm/README.md))**:
   - Update all user-guide chapters (`index.md`, `01`..`06`) and `README.md` with full documentation for every feature, shortcut, `:colon` command, cross-hair mode, history persistence config, and `<F1>` help system.
