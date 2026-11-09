//! Per-pane Smooth Kinetic Scrolling (`SCRL-01`) and Mainframe/Timeshare Block & Page-Mode Scroller (`SCRL-02`).
//!
//! - **`PaneScrollMode::Smooth`** (default): Velocity-damped kinetic scrollback in TUI mode and
//!   sub-cell pixel-interpolated scrolling in Native GUI mode (`GuiWorkspaceRenderer`).
//! - **`PaneScrollMode::BlockPage`**: Inspired by IBM 3270 / VM/CMS `MORE...` / `HOLDING` and TOPS-20
//!   page-mode terminals. Fills one viewport page (`canvas.height` rows) after a command or page clear
//!   and holds the viewport while continuing to buffer incoming PTY output into scrollback without loss,
//!   until the operator advances a page (`Enter`, `Space`, `PageDown`, `PF8`/`PA2`) or steps back a
//!   block (`b`, `PageUp`, `PF7`).
//! - **`PaneScrollMode::Instant`**: Classic discrete row scrolling without kinetic inertia.

use std::path::{Path, PathBuf};

use crate::pane::history_store::HistoryStore;
use crate::pane::modes::{CanvasSearchState, CopyModeState, HintsModeState};
use crate::pane::{CompositePane, FloatingPromptState, PaneId, PromptMouseDrag};
use crate::pty::PtySession;
use crate::pty::line_discipline::LineDisciplineState;
use crate::vt::VtParser;
pub use crate::vt::scrollback::PaneScrollMode;

/// Runtime per-pane scroll state tracking kinetic velocity, sub-cell pixel interpolation,
/// and IBM 3270 / VM/CMS `MORE...` / `HOLDING` page-gate anchors (`SCRL-01`, `SCRL-02`).
#[derive(Clone, Debug)]
pub struct PaneScrollState {
    pub mode: PaneScrollMode,
    /// Current kinetic velocity in lines per 16ms frame tick (positive = scrolling up into history).
    pub velocity_lines_per_tick: f32,
    /// Sub-cell fractional row offset in `[-1.0, 1.0]` for smooth pixel interpolation in GUI mode.
    pub subcell_phase: f32,
    /// Global row (`scrollback_used_rows - scrollback_offset`) at the top of the current `BlockPage` page.
    pub page_anchor_global_row: usize,
    /// `true` when `BlockPage` mode has filled a viewport page and frozen the visible screen.
    pub page_hold_active: bool,
    /// `true` when the operator stepped backward (`PF7`/`PageUp`/`b`) or entered `CopyMode` on a held page (`HOLDING`).
    pub holding_inspected: bool,
    /// Number of buffered output lines waiting below the held viewport page.
    pub pending_lines: usize,
    /// Whether the user explicitly toggled/configured scroll mode (ensuring the OIA badge is shown even in headless tests).
    pub explicit_badge: bool,
}

impl Default for PaneScrollState {
    fn default() -> Self {
        Self {
            mode: PaneScrollMode::Smooth,
            velocity_lines_per_tick: 0.0,
            subcell_phase: 0.0,
            page_anchor_global_row: 0,
            page_hold_active: false,
            holding_inspected: false,
            pending_lines: 0,
            explicit_badge: false,
        }
    }
}

impl PaneScrollState {
    /// Returns `true` when kinetic velocity or sub-cell interpolation is actively animating.
    pub fn is_kinetic_active(&self) -> bool {
        self.mode == PaneScrollMode::Smooth
            && (self.velocity_lines_per_tick.abs() > 0.02 || self.subcell_phase.abs() > 0.02)
    }

    /// Sub-cell vertical pixel offset for Native GUI rendering (`GuiWorkspaceRenderer`).
    pub fn subcell_offset_px(&self, cell_height_px: u16) -> f32 {
        if self.mode == PaneScrollMode::Smooth {
            (self.subcell_phase * f32::from(cell_height_px.max(1))).clamp(
                -f32::from(cell_height_px.max(1)),
                f32::from(cell_height_px.max(1)),
            )
        } else {
            0.0
        }
    }
}

impl CompositePane {
    /// Active per-pane scrolling discipline (`Smooth`, `Instant`, or `BlockPage`).
    pub fn scroll_mode(&self) -> PaneScrollMode {
        self.scroll.mode
    }

    /// Immutable reference to the pane's [`PaneScrollState`].
    pub fn scroll_state(&self) -> &PaneScrollState {
        &self.scroll
    }

    /// Mutable reference to the pane's [`PaneScrollState`].
    pub fn scroll_state_mut(&mut self) -> &mut PaneScrollState {
        &mut self.scroll
    }

    /// Returns `true` when `BlockPage` mode is actively holding the viewport on a full page (`MORE...` or `HOLDING`).
    pub fn is_page_holding(&self) -> bool {
        self.scroll.mode == PaneScrollMode::BlockPage
            && !self.vt.is_alternate_screen()
            && self.scroll.page_hold_active
    }

    /// Returns `true` when a held page is in `HOLDING` inspection state (`PF7`/`PageUp`/`b` or `CopyMode`).
    pub fn is_holding_inspected(&self) -> bool {
        self.is_page_holding() && (self.scroll.holding_inspected || self.copy_mode.active)
    }

    /// Number of buffered output lines queued below the currently held page.
    pub fn pending_page_lines(&self) -> usize {
        if self.is_page_holding() {
            self.scroll.pending_lines.max(self.vt.scrollback_offset())
        } else {
            0
        }
    }

    /// Formatted OIA status bar badge for this pane's scroll state.
    pub fn oia_scroll_badge(&self) -> String {
        self.scroll.mode.oia_badge(
            self.is_page_holding(),
            self.is_holding_inspected(),
            self.pending_page_lines(),
        )
    }

    /// Sets the pane's [`PaneScrollMode`], initializing page-gate anchors when entering `BlockPage`.
    pub fn set_scroll_mode(&mut self, mode: PaneScrollMode) {
        self.scroll.mode = mode;
        self.scroll.explicit_badge = true;
        self.scroll.velocity_lines_per_tick = 0.0;
        self.scroll.subcell_phase = 0.0;

        match mode {
            PaneScrollMode::BlockPage => {
                let used = self.vt.scrollback_used_rows();
                let off = self.vt.scrollback_offset();
                if off > 0 {
                    self.scroll.page_anchor_global_row = used.saturating_sub(off);
                    self.scroll.page_hold_active = true;
                    self.scroll.holding_inspected = true;
                    self.scroll.pending_lines = off;
                } else {
                    let cur_r = usize::from(self.vt.cursor_position().0);
                    self.scroll.page_anchor_global_row = if cur_r == 0 && used == 0 {
                        0
                    } else {
                        used
                    };
                    self.scroll.page_hold_active = false;
                    self.scroll.holding_inspected = false;
                    self.scroll.pending_lines = 0;
                }
                self.status_message = Some(
                    "SCROLL MODE: PAGE/BLOCK (3270 MORE...) — pauses at full page (Space/Enter/PgDn/PF8 next, b/PgUp/PF7 back)"
                        .to_string(),
                );
            }
            PaneScrollMode::Smooth => {
                self.scroll.page_hold_active = false;
                self.scroll.holding_inspected = false;
                self.scroll.pending_lines = 0;
                self.status_message = Some(
                    "SCROLL MODE: SMOOTH — kinetic velocity & sub-cell interpolation active"
                        .to_string(),
                );
            }
            PaneScrollMode::Instant => {
                self.scroll.page_hold_active = false;
                self.scroll.holding_inspected = false;
                self.scroll.pending_lines = 0;
                self.status_message =
                    Some("SCROLL MODE: INSTANT — discrete line scrolling active".to_string());
            }
        }
    }

    /// Sets the pane's [`PaneScrollMode`] from configuration or session restore without forcing explicit badge display.
    pub fn set_scroll_mode_from_config(&mut self, mode: PaneScrollMode) {
        self.set_scroll_mode(mode);
        self.scroll.explicit_badge = false;
        self.status_message = None;
    }

    /// Cycles `Smooth -> BlockPage -> Instant -> Smooth`.
    pub fn cycle_scroll_mode(&mut self) -> PaneScrollMode {
        let next = self.scroll.mode.next();
        self.set_scroll_mode(next);
        next
    }

    /// Toggles directly between `PaneScrollMode::Smooth` and `PaneScrollMode::BlockPage` (`Ctrl+X PageDown` / OIA click).
    pub fn toggle_block_page_scroll_mode(&mut self) -> PaneScrollMode {
        let next = if self.scroll.mode == PaneScrollMode::BlockPage {
            PaneScrollMode::Smooth
        } else {
            PaneScrollMode::BlockPage
        };
        self.set_scroll_mode(next);
        next
    }

    /// Resets the `BlockPage` anchor to the current cursor row (called on command submission or `:clear`).
    pub fn reset_page_anchor(&mut self) {
        let used = self.vt.scrollback_used_rows();
        let cur_r = usize::from(self.vt.cursor_position().0);
        self.scroll.page_anchor_global_row = used + cur_r;
        self.scroll.page_hold_active = false;
        self.scroll.holding_inspected = false;
        self.scroll.pending_lines = 0;
        self.scroll.velocity_lines_per_tick = 0.0;
        self.scroll.subcell_phase = 0.0;
    }

    /// Updates `BlockPage` page-gate state after new PTY output bytes have been processed into `self.vt`.
    ///
    /// When `PaneScrollMode::BlockPage` is active and output since `page_anchor_global_row` fills or
    /// exceeds one viewport page (`canvas.height` rows), locks `vt` scrollback to `page_anchor_global_row`
    /// and sets `page_hold_active = true` (`MORE...`), while allowing `vt` to continue buffering all
    /// subsequent PTY bytes at the bottom of scrollback.
    pub fn sync_page_hold_after_output(&mut self) {
        if self.scroll.mode != PaneScrollMode::BlockPage || self.vt.is_alternate_screen() {
            return;
        }
        let page_rows = usize::from(self.vt.size().0.max(1));
        let used = self.vt.scrollback_used_rows();
        let cur_r = usize::from(self.vt.cursor_position().0);
        let total_written = used + cur_r;
        let anchor = self.scroll.page_anchor_global_row;

        if self.scroll.page_hold_active || total_written.saturating_sub(anchor) >= page_rows {
            if used > anchor {
                self.scroll.page_hold_active = true;
                let hold_offset = used.saturating_sub(anchor).min(self.vt.scrollback_capacity());
                self.vt.set_scrollback_offset(hold_offset);
                self.scroll.pending_lines = self.vt.scrollback_offset();
            } else if cur_r >= page_rows.saturating_sub(1)
                && total_written.saturating_sub(anchor) >= page_rows.saturating_sub(1)
            {
                self.scroll.page_hold_active = true;
                self.scroll.pending_lines = 0;
            }
        }
    }

    /// Advances forward by one full viewport page (`Enter`, `Space`, `PageDown`, `PF8`/`PA2` in `BlockPage` mode).
    /// Returns to the live tail and clears `MORE...`/`HOLDING` once caught up.
    pub fn scroll_page_advance(&mut self) -> bool {
        let page_rows = usize::from(self.vt.size().0.max(1));
        if self.scroll.mode == PaneScrollMode::BlockPage {
            let used = self.vt.scrollback_used_rows();
            let cur_r = usize::from(self.vt.cursor_position().0);
            if self.scroll.page_hold_active || self.vt.scrollback_offset() > 0 {
                let next_anchor = self.scroll.page_anchor_global_row.saturating_add(page_rows);
                if next_anchor < used {
                    self.scroll.page_anchor_global_row = next_anchor;
                    let new_off = used - next_anchor;
                    self.vt.set_scrollback_offset(new_off);
                    self.scroll.page_hold_active = true;
                    self.scroll.holding_inspected = false;
                    self.scroll.pending_lines = self.vt.scrollback_offset();
                    self.status_message = Some(format!(
                        "3270 PAGE ADVANCE — MORE... (+{} lines buffered)",
                        self.scroll.pending_lines
                    ));
                } else {
                    self.vt.reset_scrollback();
                    self.scroll.page_anchor_global_row = used + cur_r;
                    self.scroll.page_hold_active = false;
                    self.scroll.holding_inspected = false;
                    self.scroll.pending_lines = 0;
                    self.status_message =
                        Some("3270 PAGE ADVANCE — READY (live output tail)".to_string());
                }
                return true;
            }
            return false;
        }

        if self.vt.scrollback_offset() > 0 {
            self.scroll_down_in_mode(page_rows);
            return true;
        }
        false
    }

    /// Steps backward by one full viewport block/page (`b`, `PageUp`, `PF7` in `BlockPage` mode),
    /// switching the OIA indicator to `HOLDING`.
    pub fn scroll_page_back(&mut self) -> bool {
        let page_rows = usize::from(self.vt.size().0.max(1));
        let used = self.vt.scrollback_used_rows();
        let curr_off = self.vt.scrollback_offset();
        let new_off = curr_off.saturating_add(page_rows).min(used);

        if self.scroll.mode == PaneScrollMode::BlockPage {
            self.vt.set_scrollback_offset(new_off);
            self.scroll.page_anchor_global_row = used.saturating_sub(new_off);
            self.scroll.page_hold_active = true;
            self.scroll.holding_inspected = true;
            self.scroll.pending_lines = new_off;
            self.status_message = Some(format!(
                "3270 PAGE BACK — HOLDING (-{} lines from live tail)",
                new_off
            ));
            return true;
        }

        self.scroll_up_in_mode(page_rows);
        true
    }

    /// Immediately releases any active `BlockPage` hold and drains to the live bottom (`c` / `Clear`).
    pub fn clear_page_hold(&mut self) {
        self.vt.reset_scrollback();
        self.reset_page_anchor();
        self.status_message = Some("Released 3270 Page Hold -> live output tail".to_string());
    }

    /// Scrolls up into history according to the pane's active [`PaneScrollMode`].
    pub fn scroll_up_in_mode(&mut self, lines: usize) {
        let step = lines.max(1);
        match self.scroll.mode {
            PaneScrollMode::Smooth => {
                self.vt.scroll_up(step);
                let used = self.vt.scrollback_used_rows();
                if used > 0 {
                    self.scroll.velocity_lines_per_tick =
                        (self.scroll.velocity_lines_per_tick + (step as f32) * 0.45).clamp(-12.0, 12.0);
                    self.scroll.subcell_phase = 0.42;
                }
            }
            PaneScrollMode::Instant => {
                self.vt.scroll_up(step);
                self.scroll.velocity_lines_per_tick = 0.0;
                self.scroll.subcell_phase = 0.0;
            }
            PaneScrollMode::BlockPage => {
                self.vt.scroll_up(step);
                let used = self.vt.scrollback_used_rows();
                let off = self.vt.scrollback_offset();
                self.scroll.page_anchor_global_row = used.saturating_sub(off);
                self.scroll.page_hold_active = true;
                self.scroll.holding_inspected = true;
                self.scroll.pending_lines = off;
            }
        }
    }

    /// Scrolls down toward the live tail according to the pane's active [`PaneScrollMode`].
    pub fn scroll_down_in_mode(&mut self, lines: usize) {
        let step = lines.max(1);
        match self.scroll.mode {
            PaneScrollMode::Smooth => {
                let prev_off = self.vt.scrollback_offset();
                self.vt.scroll_down(step);
                if prev_off > 0 {
                    self.scroll.velocity_lines_per_tick =
                        (self.scroll.velocity_lines_per_tick - (step as f32) * 0.45).clamp(-12.0, 12.0);
                    self.scroll.subcell_phase = if self.vt.scrollback_offset() > 0 {
                        -0.42
                    } else {
                        0.0
                    };
                }
            }
            PaneScrollMode::Instant => {
                self.vt.scroll_down(step);
                self.scroll.velocity_lines_per_tick = 0.0;
                self.scroll.subcell_phase = 0.0;
            }
            PaneScrollMode::BlockPage => {
                self.vt.scroll_down(step);
                let used = self.vt.scrollback_used_rows();
                let off = self.vt.scrollback_offset();
                if off == 0 {
                    let cur_r = usize::from(self.vt.cursor_position().0);
                    self.scroll.page_anchor_global_row = used + cur_r;
                    self.scroll.page_hold_active = false;
                    self.scroll.holding_inspected = false;
                    self.scroll.pending_lines = 0;
                } else {
                    self.scroll.page_anchor_global_row = used.saturating_sub(off);
                    self.scroll.page_hold_active = true;
                    self.scroll.pending_lines = off;
                }
            }
        }
    }

    /// Injects a fractional kinetic scroll impulse (`delta_lines > 0.0` scrolls up into history;
    /// `< 0.0` scrolls down toward live tail) for smooth trackpad/wheel/kinetic scrolling (`SCRL-01`).
    pub fn push_kinetic_scroll(&mut self, delta_lines: f32) {
        if self.scroll.mode != PaneScrollMode::Smooth {
            let discrete = delta_lines.round() as isize;
            if discrete > 0 {
                self.scroll_up_in_mode(discrete as usize);
            } else if discrete < 0 {
                self.scroll_down_in_mode(discrete.unsigned_abs());
            }
            return;
        }
        self.scroll.velocity_lines_per_tick =
            (self.scroll.velocity_lines_per_tick + delta_lines * 0.55).clamp(-16.0, 16.0);
        self.scroll.subcell_phase += delta_lines * 0.45;
        self.drain_subcell_row_crossings();
    }

    /// Advances one 16ms kinetic smooth-scrolling physics tick (`SCRL-01`).
    /// Returns `true` if the viewport offset or sub-cell pixel phase changed and requires a redraw.
    pub fn tick_smooth_scroll(&mut self) -> bool {
        if self.scroll.mode != PaneScrollMode::Smooth {
            return false;
        }
        if self.scroll.velocity_lines_per_tick.abs() <= 0.02
            && self.scroll.subcell_phase.abs() <= 0.02
        {
            if self.scroll.velocity_lines_per_tick != 0.0 || self.scroll.subcell_phase != 0.0 {
                self.scroll.velocity_lines_per_tick = 0.0;
                self.scroll.subcell_phase = 0.0;
                return true;
            }
            return false;
        }

        self.scroll.subcell_phase += self.scroll.velocity_lines_per_tick;
        self.drain_subcell_row_crossings();

        // Apply exponential velocity damping
        self.scroll.velocity_lines_per_tick *= 0.74;
        if self.scroll.velocity_lines_per_tick.abs() <= 0.05 {
            self.scroll.velocity_lines_per_tick = 0.0;
            self.scroll.subcell_phase *= 0.62;
            if self.scroll.subcell_phase.abs() <= 0.03 {
                self.scroll.subcell_phase = 0.0;
            }
        }

        if self.vt.scrollback_offset() == 0 && self.scroll.subcell_phase < 0.0 {
            self.scroll.subcell_phase = 0.0;
            self.scroll.velocity_lines_per_tick = 0.0;
        }

        true
    }

    fn drain_subcell_row_crossings(&mut self) {
        while self.scroll.subcell_phase >= 1.0 {
            self.scroll.subcell_phase -= 1.0;
            self.vt.scroll_up(1);
        }
        while self.scroll.subcell_phase <= -1.0 {
            self.scroll.subcell_phase += 1.0;
            self.vt.scroll_down(1);
            if self.vt.scrollback_offset() == 0 {
                self.scroll.subcell_phase = 0.0;
                self.scroll.velocity_lines_per_tick = 0.0;
                break;
            }
        }
    }

    pub fn id(&self) -> PaneId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn vt(&self) -> &VtParser {
        &self.vt
    }

    pub fn vt_mut(&mut self) -> &mut VtParser {
        &mut self.vt
    }

    pub fn has_pty(&self) -> bool {
        self.pty.is_some()
    }

    pub fn pty(&self) -> Option<&PtySession> {
        self.pty.as_ref()
    }

    pub fn prompt(&self) -> &FloatingPromptState {
        &self.prompt
    }

    pub fn prompt_mut(&mut self) -> &mut FloatingPromptState {
        &mut self.prompt
    }

    pub fn history_store(&self) -> &HistoryStore {
        &self.history_store
    }

    pub fn history_store_mut(&mut self) -> &mut HistoryStore {
        &mut self.history_store
    }

    pub fn export_history_to_temp_file(
        &mut self,
        custom_path: Option<&Path>,
    ) -> std::io::Result<PathBuf> {
        self.history_store
            .export_to_temp_file(&self.prompt.history, custom_path)
    }

    pub fn line_discipline(&self) -> &LineDisciplineState {
        &self.line_discipline
    }

    pub fn line_discipline_mut(&mut self) -> &mut LineDisciplineState {
        &mut self.line_discipline
    }

    pub fn is_password_prompt(&self) -> bool {
        self.line_discipline.is_password_prompt() || self.is_active_canvas_password_prompt()
    }

    pub fn has_foreground_subcommand(&self) -> bool {
        self.line_discipline.has_foreground_subcommand()
    }

    pub fn mouse_drag(&self) -> Option<PromptMouseDrag> {
        self.mouse_drag
    }

    pub fn set_mouse_drag(&mut self, drag: Option<PromptMouseDrag>) {
        self.mouse_drag = drag;
    }

    pub fn take_mouse_drag(&mut self) -> Option<PromptMouseDrag> {
        self.mouse_drag.take()
    }

    pub fn canvas_search(&self) -> &CanvasSearchState {
        &self.canvas_search
    }

    pub fn copy_mode(&self) -> &CopyModeState {
        &self.copy_mode
    }

    pub fn copy_mode_mut(&mut self) -> &mut CopyModeState {
        &mut self.copy_mode
    }

    pub fn hints_mode(&self) -> &HintsModeState {
        &self.hints_mode
    }

    pub fn hints_mode_mut(&mut self) -> &mut HintsModeState {
        &mut self.hints_mode
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn show_version_badge(&self) -> bool {
        self.show_version_badge
    }

    pub fn set_show_version_badge(&mut self, enabled: bool) {
        self.show_version_badge = enabled;
    }

    pub fn remote_target(&self) -> Option<&str> {
        self.remote_target.as_deref()
    }

    pub fn set_remote_target(&mut self, target: Option<String>) {
        self.remote_target = target;
    }

    /// Returns the shell executable configured/running in this pane (`$SHELL`, `/bin/zsh`, `fish`, `pwsh`, etc.).
    pub fn shell_program(&self) -> &str {
        &self.shell_program
    }

    /// Returns the command-line arguments passed to this pane's shell executable.
    pub fn shell_args(&self) -> &[String] {
        &self.shell_args
    }

    /// Sets the shell executable and arguments for this pane.
    pub fn set_shell_program(&mut self, program: impl Into<String>, args: Vec<String>) {
        let prog = program.into();
        if !prog.trim().is_empty() {
            self.shell_program = prog.trim().to_string();
        }
        self.shell_args = args;
    }

    /// Returns the explicitly tracked working directory for this pane, if set.
    pub fn tracked_cwd(&self) -> Option<&Path> {
        self.tracked_cwd.as_deref()
    }

    /// Sets the explicitly tracked working directory for this pane.
    pub fn set_tracked_cwd(&mut self, cwd: Option<PathBuf>) {
        self.tracked_cwd = cwd;
    }

    /// Resolves the live working directory (`OSC 7` -> `/proc/<fg_or_shell_pid>/cwd` -> `tracked_cwd` -> `current_dir()`).
    pub(crate) fn resolve_pane_cwd(&self) -> PathBuf {
        if let Some(osc7_cwd) = self.vt.reported_cwd() {
            return osc7_cwd.to_path_buf();
        }
        #[cfg(target_os = "linux")]
        if let Some(pty) = &self.pty {
            if let Some(fg_pid) = self.line_discipline.foreground_pgid.filter(|&p| p > 0)
                && let Ok(path) = std::fs::read_link(format!("/proc/{fg_pid}/cwd"))
            {
                return path;
            }
            if let Some(pid) = pty.child_pid()
                && let Ok(path) = std::fs::read_link(format!("/proc/{pid}/cwd"))
            {
                return path;
            }
        }
        if let Some(tracked) = &self.tracked_cwd {
            return tracked.clone();
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Resolves the active software/command running in this pane (`prompt.running_command` or `/proc/<fg_pgid>/cmdline`).
    pub fn resolve_running_command(&self) -> Option<String> {
        if let Some(cmd) = self
            .prompt
            .running_command
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let first = cmd.split_whitespace().next().unwrap_or("");
            if !matches!(first, "cd" | "clear" | "cls" | "exit" | "logout") {
                return Some(cmd.to_string());
            }
        }
        #[cfg(target_os = "linux")]
        if self.pty.is_some()
            && self.line_discipline.has_foreground_subcommand()
            && let Some(fg_pid) = self.line_discipline.foreground_pgid.filter(|&p| p > 0)
            && let Ok(raw) = std::fs::read(format!("/proc/{fg_pid}/cmdline"))
        {
            let parts: Vec<String> = raw
                .split(|&b| b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| {
                    let arg = String::from_utf8_lossy(s).into_owned();
                    if arg.contains(char::is_whitespace) {
                        format!("'{arg}'")
                    } else {
                        arg
                    }
                })
                .collect();
            if !parts.is_empty() {
                return Some(parts.join(" "));
            }
        }
        None
    }

    /// Updates `tracked_cwd` and `prompt.running_command` when a shell command is submitted.
    pub(crate) fn track_submitted_command_state(&mut self, cmd_str: &str) {
        let normalized = cmd_str.replace('\n', " ; ");
        let trimmed = normalized.trim();
        for segment in trimmed.split(&[';', '&'][..]) {
            let seg = segment.trim();
            if seg == "cd" || seg == "cd ~" {
                if let Ok(home) = std::env::var("HOME") {
                    self.tracked_cwd = Some(PathBuf::from(home));
                }
            } else if let Some(rest) = seg.strip_prefix("cd ") {
                let target_raw = rest.trim().trim_matches(&['"', '\''][..]);
                if !target_raw.is_empty() && target_raw != "-" {
                    let expanded = if let Some(after_tilde) = target_raw.strip_prefix("~/")
                        && let Ok(home) = std::env::var("HOME")
                    {
                        PathBuf::from(home).join(after_tilde)
                    } else {
                        PathBuf::from(target_raw)
                    };
                    let base = self.resolve_pane_cwd();
                    let joined = if expanded.is_absolute() {
                        expanded
                    } else {
                        base.join(expanded)
                    };
                    self.tracked_cwd = Some(clean_relative_path(joined));
                }
            }
        }
        self.prompt.running_command = Some(normalized);
    }

    pub fn pending_unsafe_paste(&self) -> Option<&str> {
        self.pending_unsafe_paste.as_deref()
    }

    pub fn set_pending_unsafe_paste(&mut self, paste: Option<String>) {
        self.pending_unsafe_paste = paste;
    }

    pub fn take_pending_unsafe_paste(&mut self) -> Option<String> {
        self.pending_unsafe_paste.take()
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub fn set_status_message(&mut self, msg: Option<String>) {
        self.status_message = msg;
        self.toast_expires_at = None;
    }

    pub fn set_status_toast(&mut self, msg: String, ttl_ms: u64) {
        self.status_message = Some(msg);
        self.toast_expires_at = Some(self.last_tick_ms.saturating_add(ttl_ms));
    }

    pub fn tick_toast(&mut self, now_ms: u64) {
        self.last_tick_ms = now_ms;
        if let Some(exp) = self.toast_expires_at
            && now_ms >= exp
        {
            self.status_message = None;
            self.toast_expires_at = None;
        }
    }

    pub fn mouse_select_sticky(&self) -> bool {
        self.mouse_select_sticky
    }

    pub fn set_mouse_select_sticky(&mut self, sticky: bool) {
        self.mouse_select_sticky = sticky;
    }

    pub fn last_osc52_sequence(&self) -> Option<&str> {
        self.last_osc52_sequence.as_deref()
    }

    pub fn set_last_osc52_sequence(&mut self, seq: Option<String>) {
        self.last_osc52_sequence = seq;
    }

    pub fn take_last_osc52_sequence(&mut self) -> Option<String> {
        self.last_osc52_sequence.take()
    }

    pub fn take_all_outbound_osc52(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(seq) = self.last_osc52_sequence.take() {
            out.push(seq);
        }
        out.extend(self.vt.take_outbound_osc52());
        out
    }

    pub fn flush_query_responses(&mut self) {
        if let Some(pty) = &self.pty {
            for resp in self.vt.take_query_responses() {
                let _ = pty.write_all(&resp);
            }
        }
    }
}

fn clean_relative_path(path: PathBuf) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}
