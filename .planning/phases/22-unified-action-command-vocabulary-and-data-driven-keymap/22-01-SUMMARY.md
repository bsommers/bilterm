# Phase 22 Plan 01 Summary — Unified `Action` Vocabulary, `WindowManager::apply`, and Whole-Workspace `.cast` Recorder

- **Plan:** `22-01-PLAN.md` (`ARCH-R01`, `REC-02`)
- **Status:** Completed

## Changes Delivered

1. **Unified `crate::action::Action` & `WindowManager::apply(Action)` (`ARCH-R01.1`):**
   - Created `src/action.rs` defining `pub enum Action` across all workspace and pane operations, aliased as `ColonCommand` in `src/colon_command/mod.rs`.
   - Added `WindowManager::apply(&mut self, action: Action) -> Result<AppAction>` in `src/wm/mod.rs`.

2. **Whole-Workspace Asciicast v2 (`.cast`) Compositor Frame Recorder (`REC-02`):**
   - Implemented `WorkspaceRecorder` in `src/vt/recording.rs` that diffs rendered `ratatui::buffer::Buffer` frames into compact ANSI escape sequences (`\x1b[{row};{col}H`, 16-color/256-color/24-bit TrueColor SGR, and cell glyphs) and flushes standard `asciinema` v2 `.cast` files (`0600` private permissions when using default paths).
   - Wired `self.wm.record_workspace_frame(buf)` into `WorkspaceWidget::render` in `src/ui/mod.rs` and updated `:record [start|workspace|pane] [path]` and `:record stop` in `src/colon_command/{parse,execute}.rs`.
