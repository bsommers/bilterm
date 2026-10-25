pub mod canvas_scene;
pub mod completion;
pub mod fuzzy_history;
pub mod highlight;
pub mod history_store;
pub mod modes;
pub mod prompt_editor;

pub use canvas_scene::{
    CandlestickBar, CanvasRenderBackend, CanvasSubcellMode, DeclarativePlot, InteractiveImageView,
    PaneCanvasScene, PlotKind, Sprite2D, VectorPrimitive,
};
pub use modes::{CanvasSearchState, CopyModeState, CopySelectionKind, HintsModeState};
pub use prompt_editor::{FloatingPromptState, PromptAnchor, PromptMode, PromptMouseDrag};
pub use crate::ui::typography::{FontFamilyPreset, PaneFontOptions};

use std::path::PathBuf;

use ratatui::layout::Rect;

use crate::error::Result;
use crate::pane::history_store::HistoryStore;
#[cfg(unix)]
use crate::pty::line_discipline::inspect_pty_fd;
use crate::pty::line_discipline::{InputRoutingMode, LineDisciplineState};
use crate::pty::{PtyConfig, PtyEvent, PtySession};
use crate::vt::VtParser;

/// Unique identifier for a `CompositePane`.
pub type PaneId = usize;

/// A single composite terminal pane combining:
/// 1. Full-height `OutputCanvas` (`VtParser` + `PtySession`)
/// 2. Overlaid `FloatingCommandPrompt` (`FloatingPromptState`)
/// 3. Bottom 1-row Operator Information Area (`OiaStatusBar`)
pub struct CompositePane {
    id: PaneId,
    title: String,
    pub(crate) vt: VtParser,
    pty: Option<PtySession>,
    prompt: FloatingPromptState,
    history_store: HistoryStore,
    line_discipline: LineDisciplineState,
    mouse_drag: Option<PromptMouseDrag>,
    /// Interactive scrollback search & match highlighting state (`VAL-02`).
    canvas_search: CanvasSearchState,
    /// Keyboard-driven Vi Copy Mode & rectangular block selection state (`VAL-04`).
    copy_mode: CopyModeState,
    /// Keyboard-driven QuickSelect 3270 Field Hints state (`ECO-01`).
    hints_mode: HintsModeState,
    /// Modern 2026/2027 retained vector, 2D sprite, declarative plotting & image scene-graph (`CANV-01`).
    pub(crate) canvas_scene: PaneCanvasScene,
    /// Per-pane / per-window selectable font family, size, scale, and style options (`FONT-02`).
    pub(crate) font_options: PaneFontOptions,
    /// Read-Only / Protected 3270 Field Lock (`:lock` / `Alt+L`, `ECO-04`).
    read_only: bool,
    /// Optional remote SSH / `tmux -CC` destination badge (`🌐 SSH:<host>`, `v1.4.0`).
    remote_target: Option<String>,
    /// Pending multi-line or escape-containing raw PTY paste awaiting user confirmation (`FND-01`).
    pending_unsafe_paste: Option<String>,
    /// Optional status feedback message displayed in the OIA bar (e.g. `:command` result or yank feedback).
    status_message: Option<String>,
    /// Last outbound `OSC 52` clipboard escape sequence emitted by `:yank` or child PTY passthrough.
    last_osc52_sequence: Option<String>,
    last_canvas_size: (u16, u16),
    exited: bool,
}

impl CompositePane {
    fn new_base(
        id: PaneId,
        title: String,
        canvas_rows: u16,
        canvas_cols: u16,
        scrollback_lines: usize,
        pty: Option<PtySession>,
        history_store: HistoryStore,
    ) -> Self {
        Self {
            id,
            title,
            vt: VtParser::new(canvas_rows, canvas_cols, scrollback_lines),
            pty,
            prompt: FloatingPromptState::default(),
            history_store,
            line_discipline: LineDisciplineState::default(),
            mouse_drag: None,
            canvas_search: CanvasSearchState::default(),
            copy_mode: CopyModeState::default(),
            hints_mode: HintsModeState::default(),
            canvas_scene: PaneCanvasScene::default(),
            font_options: PaneFontOptions::default(),
            read_only: false,
            remote_target: None,
            pending_unsafe_paste: None,
            status_message: None,
            last_osc52_sequence: None,
            last_canvas_size: (canvas_rows, canvas_cols),
            exited: false,
        }
    }

    /// Creates a headless `CompositePane` (without spawning an OS process) for deterministic unit/snapshot testing.
    pub fn new_headless(id: PaneId, rows: u16, cols: u16) -> Self {
        let canvas_rows = rows.saturating_sub(1).max(1);
        let canvas_cols = cols.max(1);
        Self::new_base(
            id,
            format!("PANE #{id}"),
            canvas_rows,
            canvas_cols,
            2000,
            None,
            HistoryStore::new_headless(id),
        )
    }

    /// Spawns a live `CompositePane` backed by a `portable-pty` shell process.
    pub fn spawn(id: PaneId, mut pty_config: PtyConfig) -> Result<Self> {
        let canvas_rows = pty_config.rows.saturating_sub(1).max(1);
        let canvas_cols = pty_config.cols.max(1);
        pty_config.rows = canvas_rows;
        pty_config.cols = canvas_cols;

        let title = format!("{} [#{id}]", pty_config.program);
        let session = PtySession::spawn(pty_config)?;
        let mut pane = Self::new_base(
            id,
            title,
            canvas_rows,
            canvas_cols,
            crate::vt::scrollback::DEFAULT_INITIAL_LINES,
            Some(session),
            HistoryStore::new_live(id),
        );
        pane.restore_persisted_history();
        Ok(pane)
    }

    /// Submits `cmd` to this pane: enforces the read-only guard, updates `running_command` and
    /// in-memory `prompt.history`, resets scrollback, records the command block in `VtParser`
    /// and `HistoryStore`, and writes `cmd + "\n"` to the PTY.
    pub fn submit_command(&mut self, cmd: &str) -> Result<()> {
        if self.read_only {
            self.status_message = Some(
                "🔒 PROTECTED: Pane is locked read-only (:lock off or Alt+L to unlock)"
                    .to_string(),
            );
            return Err(crate::error::BiltermError::Pty(format!(
                "pane #{} is locked read-only (🔒 PROT)",
                self.id
            )));
        }
        let cmd_str = cmd.to_string();
        if !cmd_str.trim().is_empty() {
            if self.prompt.history.last() != Some(&cmd_str) {
                self.prompt.history.push(cmd_str.clone());
            }
            if !cmd_str.trim().starts_with(':') {
                self.prompt.running_command = Some(cmd_str.replace('\n', " ; "));
                self.prompt.force_command_prompt = false;
                // Auto-dismiss interactive `:tek demo` / `:canvas demo` overlay when a shell command is executed
                if cmd_str.trim() == "clear"
                    || self
                        .vt
                        .tektronix()
                        .labels
                        .iter()
                        .any(|l| l.text == "TEK 4014 STORAGE TUBE")
                {
                    self.vt.tektronix_mut().clear_all();
                    self.canvas_scene.clear();
                    self.canvas_scene.active = false;
                }
            }
        }
        self.vt.reset_scrollback();
        self.vt.record_command_submission(cmd_str.clone());
        let _ = self.history_store.record_command(&cmd_str);
        let mut payload = cmd_str.into_bytes();
        payload.push(b'\n');
        self.write_to_pty(&payload)
    }

    /// Takes the current text from `prompt` and submits it via [`Self::submit_command`].
    pub fn submit_prompt(&mut self) -> Result<String> {
        if self.read_only {
            self.status_message = Some(
                "🔒 PROTECTED: Pane is locked read-only (:lock off or Alt+L to unlock)"
                    .to_string(),
            );
            return Err(crate::error::BiltermError::Pty(format!(
                "pane #{} is locked read-only (🔒 PROT)",
                self.id
            )));
        }
        let submitted = self.prompt.take_submission();
        self.submit_command(&submitted)?;
        Ok(submitted)
    }

    /// Replaces the prompt's in-memory history with the newest persisted commands of the
    /// pane's history session (so Up/Down, `Ctrl+R`, and the history popup see prior runs).
    pub fn restore_persisted_history(&mut self) {
        self.prompt.history = self.history_store.load_recent_commands();
        self.prompt.history_index = None;
        self.prompt.history_popup_selected = 0;
    }

    /// Computes the `OutputCanvas` rectangle.
    ///
    /// - In floating modes (`Top`, `Bottom`, `Floating`) or when the prompt is hidden (`ALT_SCREEN` / `!ECHO`),
    ///   occupies `100%` of pane height minus 1 row for the OIA status bar.
    /// - In non-overlapping split modes (`SplitTop`, `SplitBottom`) while the prompt is visible,
    ///   occupies the dedicated remaining height so there is zero overlap with `prompt_rect`.
    pub fn canvas_rect(&self, pane_area: Rect) -> Rect {
        let usable_h = pane_area.height.saturating_sub(1).max(1);
        if self.is_prompt_visible() && self.prompt.anchor.is_split() && usable_h >= 4 {
            let prompt_h = self
                .prompt
                .effective_height(usable_h.saturating_sub(1))
                .min(usable_h.saturating_sub(1));
            let canvas_h = usable_h.saturating_sub(prompt_h).max(1);
            match self.prompt.anchor {
                PromptAnchor::SplitTop => Rect {
                    x: pane_area.x,
                    y: pane_area.y + prompt_h,
                    width: pane_area.width,
                    height: canvas_h,
                },
                PromptAnchor::SplitBottom => Rect {
                    x: pane_area.x,
                    y: pane_area.y,
                    width: pane_area.width,
                    height: canvas_h,
                },
                _ => unreachable!(),
            }
        } else {
            Rect {
                x: pane_area.x,
                y: pane_area.y,
                width: pane_area.width,
                height: usable_h,
            }
        }
    }

    /// Computes the bottom 1-row Operator Information Area (OIA) status bar rectangle.
    pub fn status_bar_rect(&self, pane_area: Rect) -> Rect {
        let y = pane_area.y + pane_area.height.saturating_sub(1);
        Rect {
            x: pane_area.x,
            y,
            width: pane_area.width,
            height: 1.min(pane_area.height),
        }
    }

    /// Computes the rectangle of the command prompt (either overlaid in floating modes or
    /// partitioned with zero overlap in `SplitTop` / `SplitBottom` modes).
    pub fn prompt_rect(&self, pane_area: Rect) -> Rect {
        let usable_h = pane_area.height.saturating_sub(1).max(1);
        if self.prompt.anchor.is_split() && usable_h >= 4 {
            let prompt_h = self
                .prompt
                .effective_height(usable_h.saturating_sub(1))
                .min(usable_h.saturating_sub(1));
            let canvas_h = usable_h.saturating_sub(prompt_h).max(1);
            let y = match self.prompt.anchor {
                PromptAnchor::SplitTop => pane_area.y,
                PromptAnchor::SplitBottom => pane_area.y + canvas_h,
                _ => pane_area.y,
            };
            return Rect {
                x: pane_area.x,
                y,
                width: pane_area.width.max(1),
                height: prompt_h,
            };
        }

        let canvas = self.canvas_rect(pane_area);
        let height = self.prompt.effective_height(canvas.height);
        let max_y_offset = canvas.height.saturating_sub(height);
        let y_offset = match self.prompt.anchor {
            PromptAnchor::Top | PromptAnchor::SplitTop => 0,
            PromptAnchor::Bottom | PromptAnchor::SplitBottom => max_y_offset,
            PromptAnchor::Floating { y_offset } => y_offset.min(max_y_offset),
        };
        let default_margin_x = if canvas.width > 6 { 1 } else { 0 };
        let default_w = canvas.width.saturating_sub(default_margin_x * 2).max(1);
        let width = self
            .prompt
            .width_cols
            .map(|w| w.clamp(16.min(canvas.width), canvas.width.max(1)))
            .unwrap_or(default_w);
        let max_x_off = canvas.width.saturating_sub(width);
        let x_off = self
            .prompt
            .x_offset
            .map(|x| x.min(max_x_off))
            .unwrap_or_else(|| default_margin_x.min(max_x_off));
        Rect {
            x: canvas.x + x_off,
            y: canvas.y + y_offset,
            width,
            height,
        }
    }

    /// Resizes the pane's outer viewport.
    ///
    /// Crucial `PANE-01` invariant: Only resizes the underlying `VtParser` and `PtySession`
    /// (`SIGWINCH`) if the `OutputCanvas` `(rows, cols)` actually changed. Moving, pinning,
    /// resizing, or hiding the `FloatingCommandPrompt` never triggers `SIGWINCH`.
    pub fn resize_viewport(&mut self, pane_area: Rect) -> Result<bool> {
        let canvas = self.canvas_rect(pane_area);
        let new_size = (canvas.height.max(1), canvas.width.max(1));
        if new_size == self.last_canvas_size {
            return Ok(false);
        }
        self.last_canvas_size = new_size;
        self.vt.set_size(new_size.0, new_size.1);
        if let Some(pty) = &self.pty {
            return pty.resize(new_size.0, new_size.1);
        }
        Ok(false)
    }

    /// Changes the prompt's anchor (`Top`, `Bottom`, `Floating`, `SplitTop`, or `SplitBottom`)
    /// and ensures the command prompt is visible (exiting Regular Terminal mode if active).
    pub fn set_prompt_anchor(&mut self, anchor: PromptAnchor) {
        self.prompt.manual_raw_override = false;
        self.prompt.force_command_prompt = true;
        self.prompt.anchor = anchor;
    }

    /// Toggles the command prompt between floating mode and non-overlapping fixed/split mode (`Alt+S` / `:prompt split`).
    pub fn toggle_split_prompt(&mut self) {
        self.prompt.manual_raw_override = false;
        self.prompt.force_command_prompt = true;
        self.prompt.anchor = match self.prompt.anchor {
            PromptAnchor::Top | PromptAnchor::Floating { .. } => PromptAnchor::SplitTop,
            PromptAnchor::Bottom => PromptAnchor::SplitBottom,
            PromptAnchor::SplitTop => PromptAnchor::Top,
            PromptAnchor::SplitBottom => PromptAnchor::Bottom,
        };
    }

    /// Returns the active high-level `PromptMode` (`Floating`, `Fixed`, or `RegularTerminal`).
    pub fn prompt_mode(&self) -> PromptMode {
        if self.prompt.manual_raw_override
            || (!self.vt.is_alternate_screen()
                && self.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough)
        {
            PromptMode::RegularTerminal
        } else if self.prompt.anchor.is_split() {
            PromptMode::Fixed
        } else {
            PromptMode::Floating
        }
    }

    /// Explicitly switches the pane to `Floating` prompt, `Fixed` (docked/split) prompt, or `RegularTerminal` mode.
    pub fn set_prompt_mode(&mut self, mode: PromptMode) {
        match mode {
            PromptMode::Floating => {
                self.prompt.manual_raw_override = false;
                self.prompt.force_command_prompt = true;
                if self.prompt.anchor.is_split() {
                    self.prompt.anchor = match self.prompt.anchor {
                        PromptAnchor::SplitBottom => PromptAnchor::Bottom,
                        _ => PromptAnchor::Top,
                    };
                }
                self.status_message = Some(
                    "MODE: FLOATING PROMPT — overlays terminal canvas (F4/Alt+E to cycle, F12 for Regular Terminal)"
                        .to_string(),
                );
            }
            PromptMode::Fixed => {
                self.prompt.manual_raw_override = false;
                self.prompt.force_command_prompt = true;
                if !self.prompt.anchor.is_split() {
                    self.prompt.anchor = match self.prompt.anchor {
                        PromptAnchor::Bottom => PromptAnchor::SplitBottom,
                        _ => PromptAnchor::SplitTop,
                    };
                }
                self.status_message = Some(
                    "MODE: FIXED (DOCKED) PROMPT — zero overlap with terminal output (F4/Alt+E to cycle)"
                        .to_string(),
                );
            }
            PromptMode::RegularTerminal => {
                self.prompt.manual_raw_override = true;
                self.prompt.force_command_prompt = false;
                self.status_message = Some(
                    "MODE: REGULAR TERMINAL — direct shell input (press F4, Alt+E, or F12/Ctrl+G to restore Command Prompt)"
                        .to_string(),
                );
            }
        }
    }

    /// Cycles through the 3 terminal modes: `Floating Prompt` -> `Fixed Prompt` -> `Regular Terminal` -> `Floating Prompt` (`F4` / `Alt+E` / `:mode`).
    pub fn cycle_prompt_mode(&mut self) -> PromptMode {
        let next = match self.prompt_mode() {
            PromptMode::Floating => PromptMode::Fixed,
            PromptMode::Fixed => PromptMode::RegularTerminal,
            PromptMode::RegularTerminal => PromptMode::Floating,
        };
        self.set_prompt_mode(next);
        next
    }

    /// Sets the base height (`3..=12` rows) of the floating command prompt without resizing the PTY (`PANE-03`).
    pub fn set_prompt_height(&mut self, rows: u16) {
        self.prompt.height = rows.clamp(3, 12);
    }

    /// Adjusts the base height of the floating command prompt by `delta_rows` (`Alt+Plus` / `Alt+Minus`).
    pub fn resize_prompt_height(&mut self, delta_rows: i16) {
        let next = (i32::from(self.prompt.height) + i32::from(delta_rows)).clamp(3, 12) as u16;
        self.prompt.height = next;
    }

    /// Sets the custom width in columns (`Some(cols)` or `None` for full-width) of the floating command prompt.
    pub fn set_prompt_width(&mut self, cols: Option<u16>) {
        let max_w = self.vt.size().1.max(16);
        self.prompt.width_cols = cols.map(|c| c.clamp(16, max_w));
    }

    /// Adjusts the floating command prompt width by `delta_cols` (`Alt+[` / `Alt+]`).
    pub fn resize_prompt_width(&mut self, delta_cols: i16) {
        let canvas_w = self.vt.size().1.max(16);
        let default_w = canvas_w.saturating_sub(2).max(16);
        let current = self.prompt.width_cols.unwrap_or(default_w);
        let next = (i32::from(current) + i32::from(delta_cols)).clamp(16, i32::from(canvas_w)) as u16;
        self.prompt.width_cols = Some(next);
    }

    /// Moves the floating prompt vertically by `delta_rows` (switching to `PromptAnchor::Floating`).
    pub fn move_prompt_vertical(&mut self, delta_rows: i16, pane_height: u16) {
        let canvas_h = pane_height.saturating_sub(1).max(1);
        let eff_h = self.prompt.effective_height(canvas_h);
        let max_y = canvas_h.saturating_sub(eff_h);
        let current_y = match self.prompt.anchor {
            PromptAnchor::Top | PromptAnchor::SplitTop => 0,
            PromptAnchor::Bottom | PromptAnchor::SplitBottom => max_y,
            PromptAnchor::Floating { y_offset } => y_offset.min(max_y),
        };
        let next_y = (i32::from(current_y) + i32::from(delta_rows)).clamp(0, i32::from(max_y)) as u16;
        self.prompt.anchor = PromptAnchor::Floating { y_offset: next_y };
    }

    /// Toggles manual Raw PTY passthrough (`Ctrl+G` / `F12`).
    pub fn toggle_manual_raw_override(&mut self) {
        if !self.prompt.manual_raw_override
            && !self.vt.is_alternate_screen()
            && self.effective_routing_mode() == InputRoutingMode::RawPtyPassthrough
        {
            self.prompt.force_command_prompt = true;
        } else {
            self.prompt.manual_raw_override = !self.prompt.manual_raw_override;
            self.prompt.force_command_prompt = !self.prompt.manual_raw_override;
        }
    }

    /// Refreshes three-signal line-discipline telemetry (`OSC 133` + `tcgetpgrp` + `termios !ECHO`).
    pub fn refresh_line_discipline(&mut self) {
        let osc = self.vt.osc133_phase();
        let osc_finished = matches!(
            osc,
            Some(crate::vt::Osc133Phase::CommandFinished(_) | crate::vt::Osc133Phase::PromptStart)
        );
        #[cfg(unix)]
        if let Some(pty) = &self.pty
            && let Some(fd) = pty.master_raw_fd()
        {
            let prev_sub = self.line_discipline.has_foreground_subcommand();
            self.line_discipline = inspect_pty_fd(fd, pty.child_pid(), osc);
            if (prev_sub && !self.line_discipline.has_foreground_subcommand()) || osc_finished {
                self.prompt.running_command = None;
            }
            // Fallback defense-in-depth: if the cursor line on the OutputCanvas is a password/passphrase
            // prompt (even if a subshell/WSL/use_pty didn't change outer pgrp or ECHO), enforce !ECHO lock.
            if self.canvas_cursor_line_looks_like_password_prompt() {
                self.line_discipline.echo_enabled = false;
                self.line_discipline.canonical_mode = true;
            }
            return;
        }
        self.line_discipline.osc133_executing =
            matches!(osc, Some(crate::vt::Osc133Phase::CommandExecuted));
        if osc_finished {
            self.prompt.running_command = None;
        }
        if (self.pty.is_some() || self.prompt.running_command.is_some())
            && self.canvas_cursor_line_looks_like_password_prompt()
        {
            self.line_discipline.echo_enabled = false;
            self.line_discipline.canonical_mode = true;
        } else if self.pty.is_some() {
            self.line_discipline.echo_enabled = true;
        }
    }

    /// Checks whether the text on the active `OutputCanvas` cursor row looks like an interactive
    /// password/passphrase prompt (`[sudo] password for ...:`, `[sudo: authenticate] Password:`,
    /// `Password:`, `Enter passphrase for key ...:`).
    pub fn canvas_cursor_line_looks_like_password_prompt(&self) -> bool {
        let (cur_r, cur_c) = self.vt.cursor_position();
        if cur_c == 0 {
            return false;
        }
        let full_row = self.vt.row_text(cur_r);
        if !full_row.contains(':') && !full_row.contains('：') {
            return false;
        }
        let (_, cols) = self.vt.size();
        let mut line = String::with_capacity(usize::from(cols));
        if cur_r > 0 {
            let prev = self.vt.row_text(cur_r - 1);
            if prev.to_ascii_lowercase().contains("[sudo") {
                line.push_str(prev.trim());
                line.push(' ');
            }
        }
        for c in 0..(cur_c + 1).min(cols) {
            if let Some(cell) = self.vt.cell_view(cur_r, c) {
                line.push_str(if cell.grapheme.is_empty() { " " } else { cell.grapheme });
            }
        }
        [line.trim(), full_row.trim()].into_iter().any(|cand| {
            let lower = cand.to_ascii_lowercase();
            (lower.ends_with(':') || lower.ends_with('：'))
                && ["password", "passphrase", "[sudo", "doas (", "verification code"]
                    .iter()
                    .any(|kw| lower.contains(kw))
        })
    }

    /// Returns `true` when an interactive password prompt (`[sudo] password...`, `Password:`)
    /// is actively running on the canvas and must never be bypassed by `force_command_prompt`.
    pub fn is_active_canvas_password_prompt(&self) -> bool {
        !self.vt.is_alternate_screen()
            && (!self.line_discipline.echo_enabled
                || self.prompt.running_command.is_some()
                || self.line_discipline.has_foreground_subcommand()
                || self.line_discipline.osc133_executing)
            && self.canvas_cursor_line_looks_like_password_prompt()
    }

    /// Determines the active keyboard routing mode (`PTY-02`, `PTY-03`, `REC-02`).
    pub fn effective_routing_mode(&self) -> InputRoutingMode {
        if self.vt.is_alternate_screen()
            || self.prompt.manual_raw_override
            || self.is_active_canvas_password_prompt()
            || (self.line_discipline.is_password_prompt() && !self.prompt.force_command_prompt)
        {
            InputRoutingMode::RawPtyPassthrough
        } else {
            InputRoutingMode::CommandPrompt
        }
    }

    /// Returns `true` when the `FloatingCommandPrompt` overlay should be rendered over the `OutputCanvas`.
    ///
    /// Automatically hides during `DECSET 47/1047/1049` alternate screen (`vim`, `htop`, `less`),
    /// `!ECHO` password prompts (`sudo`, `ssh`), foreground sub-REPLs, or manual raw override.
    pub fn is_prompt_visible(&self) -> bool {
        self.effective_routing_mode() == InputRoutingMode::CommandPrompt
    }

    /// Resolves the live working directory of the child shell process (`OSC 7` reported path first,
    /// then `/proc/<pid>/cwd` on Linux, falling back to `std::env::current_dir()`).
    pub fn resolve_cwd(&self) -> PathBuf {
        if let Some(osc7_cwd) = self.vt.reported_cwd() {
            return osc7_cwd.to_path_buf();
        }
        #[cfg(target_os = "linux")]
        if let Some(pty) = &self.pty
            && let Some(pid) = pty.child_pid()
            && let Ok(path) = std::fs::read_link(format!("/proc/{pid}/cwd"))
        {
            return path;
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Triggers or cycles `Tab` / `Shift+Tab` completion inside the floating prompt using the pane's live `cwd`.
    pub fn complete_prompt_tab(&mut self, reverse: bool) -> bool {
        let cwd = self.resolve_cwd();
        self.prompt.complete_tab(&cwd, reverse)
    }

    /// Feeds raw PTY output bytes into the pane's `VtParser` and updates line-discipline state.
    pub fn process_pty_output(&mut self, bytes: &[u8]) {
        self.vt.process(bytes);
        self.canvas_scene
            .scan_osc_canvas_sequences(self.vt.tektronix_mut(), bytes);
        if let Some(wt) = self.vt.window_title() {
            self.title = format!("{wt} [#{}]", self.id);
        }
        if let Some(pty) = &self.pty {
            for resp in self.vt.take_query_responses() {
                let _ = pty.write_all(&resp);
            }
        }
        self.refresh_line_discipline();
    }

    /// Drains pending events from the live `PtySession` with a bounded per-frame budget so
    /// high-throughput streams (`tail -f /dev/random`, `yes`) never starve the UI loop.
    pub fn poll_pty_events(&mut self) {
        let events = if let Some(pty) = &mut self.pty {
            pty.drain_events()
        } else {
            Vec::new()
        };
        self.ingest_pty_events(events);
    }

    /// Feeds a batch of drained `PtyEvent`s into the VT state machine in order.
    pub fn ingest_pty_events(&mut self, events: Vec<PtyEvent>) {
        let mut coalesced = Vec::new();
        for ev in events {
            match ev {
                PtyEvent::Output(bytes) => {
                    coalesced.extend_from_slice(&bytes);
                }
                PtyEvent::Exited(_) => {
                    self.exited = true;
                }
            }
        }
        if !coalesced.is_empty() {
            // Every byte must reach the parser in order: truncating the stream splits escape
            // sequences and loses mode changes (DECSET 1049/2004, OSC 133). Per-frame work is
            // already bounded by `PtySession::drain_events` plus the bounded reader channel.
            self.vt.process(&coalesced);
            self.canvas_scene
                .scan_osc_canvas_sequences(self.vt.tektronix_mut(), &coalesced);
            if let Some(wt) = self.vt.window_title() {
                self.title = format!("{wt} [#{}]", self.id);
            }
            if let Some(pty) = &self.pty {
                for resp in self.vt.take_query_responses() {
                    let _ = pty.write_all(&resp);
                }
            }
            self.refresh_line_discipline();
        }
    }

    /// Writes raw bytes to the underlying `PtySession` if one is attached (`v1.1.0` / `v1.4.0`).
    pub fn write_to_pty(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(pty) = &self.pty {
            pty.write_all(bytes)?;
        }
        Ok(())
    }

    pub fn has_exited(&self) -> bool {
        self.exited
    }

    /// Returns `true` when this pane is headless (not backed by a live OS `PtySession`).
    pub fn is_headless(&self) -> bool {
        self.pty.is_none()
    }
}

#[cfg(test)]
mod tests;
