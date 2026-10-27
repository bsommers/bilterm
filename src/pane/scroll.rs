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

use crate::pane::CompositePane;
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
}
