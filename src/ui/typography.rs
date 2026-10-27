//! Dynamic Terminal Text & Font Size Scaling (`FONT-01`).
//!
//! Provides:
//! 1. **DEC Line-Size Attributes (`DECSWL` / `DECDWL` / `DECDHL`)**:
//!    - `\x1b#5` (`DECSWL`): Single-width, single-height line (`LineSizeAttr::Single`).
//!    - `\x1b#6` (`DECDWL`): Double-width, single-height line (`LineSizeAttr::DoubleWidth`).
//!    - `\x1b#3` (`DECDHL` top): Double-height, double-width top half (`LineSizeAttr::DoubleHeightTop`).
//!    - `\x1b#4` (`DECDHL` bottom): Double-height, double-width bottom half (`LineSizeAttr::DoubleHeightBottom`).
//! 2. **Multi-Cell Scaled Row Rendering (`render_double_width_row`)**:
//!    Expands each logical character cell across 2 horizontal terminal cells (`2x1`) in Ratatui's
//!    `Buffer` so TUI panes visibly render double-width/double-height banners and headings.
//! 3. **Host Terminal Font Passthrough (`encode_host_font_passthrough`)**:
//!    Emits an `OSC 50` font-scaling control sequence (`\x1b]50;fontsize=<pt>;scale=<pct>%\x07`)
//!    when `config.host_font_passthrough` is enabled.
//! 4. **Workspace Font Scaling & Pixel-Window Grid Recalculation (`impl WindowManager`)**:
//!    Handles `Ctrl++`/`Ctrl+=`, `Ctrl+-`, `Ctrl+0`, `:fontsize`, and dynamic `(rows, cols)`
//!    viewport recalculation when font scale changes inside a fixed pixel window.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::error::Result;
pub use crate::pane::typography::{FontFamilyPreset, PaneFontOptions};
use crate::vt::VtParser;
pub use crate::vt::{LineSizeAttr, scan_dec_line_size_escapes};
use crate::wm::WindowManager;

/// Formats an `OSC 50` host-terminal font scaling passthrough sequence (`FONT-01`).
pub fn encode_host_font_passthrough(font_size_pt: u16, scale_pct: u16) -> String {
    format!("\x1b]50;fontsize={font_size_pt};scale={scale_pct}%\x07")
}

/// Renders a `DECDWL` / `DECDHL` double-width row into Ratatui's `Buffer`, expanding each logical
/// source cell at `src_c` across two horizontal buffer cells (`2 * src_c` and `2 * src_c + 1`) (`FONT-01`).
pub fn render_double_width_row(
    vt: &VtParser,
    row: u16,
    canvas: Rect,
    render_cols: u16,
    attr: LineSizeAttr,
    buf: &mut Buffer,
) {
    let dst_y = canvas.y + row;
    let max_src_cols = render_cols.div_ceil(2);
    for src_c in 0..max_src_cols {
        let dst_c0 = src_c.saturating_mul(2);
        if dst_c0 >= render_cols {
            break;
        }
        let Some(vt_cell) = vt.cell_view(row, src_c) else {
            continue;
        };
        let mut style = vt_cell.to_ratatui_style().add_modifier(Modifier::BOLD);
        if attr == LineSizeAttr::DoubleHeightBottom {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        let sym = if vt_cell.grapheme.is_empty() || vt_cell.is_wide_continuation() {
            " "
        } else {
            vt_cell.grapheme
        };
        if let Some(cell0) = buf.cell_mut((canvas.x + dst_c0, dst_y)) {
            cell0.set_symbol(sym);
            cell0.set_style(style);
        }
        let dst_c1 = dst_c0 + 1;
        if dst_c1 < render_cols
            && let Some(cell1) = buf.cell_mut((canvas.x + dst_c1, dst_y))
        {
            cell1.set_symbol(" ");
            cell1.set_style(style);
        }
    }
}

impl WindowManager {
    /// Returns the scale percentage at which the workspace's outer viewport was computed (`100` by default).
    pub fn viewport_scale_pct(&self) -> u16 {
        self.viewport_scale_pct.clamp(50, 400)
    }

    /// Sets the scale percentage at which the workspace's outer viewport was computed.
    pub fn set_viewport_scale_pct(&mut self, pct: u16) {
        self.viewport_scale_pct = pct.clamp(50, 400);
    }

    /// Returns the effective font scale percentage (`50..=400`) for pane `id` (independent per pane).
    pub fn pane_effective_scale_pct(&self, id: usize) -> u16 {
        let Some(pane) = self.pane(id) else {
            return self.config().font_scale_pct.clamp(50, 400);
        };
        let opts = pane.font_options();
        let base_pt = self.config().font_size_pt.clamp(6, 72);
        let pct = opts.scale_pct.unwrap_or(100).clamp(50, 400);
        if let Some(custom_pt) = opts.size_pt {
            let eff_pt = opts.effective_size_pt_with_defaults(custom_pt, pct);
            ((u32::from(eff_pt) * 100 + u32::from(base_pt) / 2) / u32::from(base_pt)).clamp(50, 400)
                as u16
        } else if self.panes().len() == 1 && opts.scale_pct.is_none() {
            self.config().font_scale_pct.clamp(50, 400)
        } else {
            pct
        }
    }

    /// Scales a pane's `inner` character rect inversely by `pane_effective_scale_pct(id)` relative
    /// to `viewport_scale_pct()` so zooming a pane's font changes that pane's terminal screen `(rows, cols)`.
    pub fn pane_scaled_inner_rect(&self, id: usize, inner: Rect) -> Rect {
        let vp_scale = u32::from(self.viewport_scale_pct());
        let pane_scale = u32::from(self.pane_effective_scale_pct(id));
        if pane_scale == vp_scale {
            return inner;
        }
        let w = ((u32::from(inner.width) * vp_scale + pane_scale / 2) / pane_scale).clamp(4, 500)
            as u16;
        let h = ((u32::from(inner.height) * vp_scale + pane_scale / 2) / pane_scale).clamp(2, 200)
            as u16;
        Rect::new(inner.x, inner.y, w, h)
    }

    /// Scales a pane's `outer` bounding rect so `inset_window_frame(scaled_outer)` matches `pane_scaled_inner_rect` (`FONT-01`).
    pub fn pane_scaled_outer_rect(&self, id: usize, outer: Rect) -> Rect {
        let vp_scale = u32::from(self.viewport_scale_pct());
        let pane_scale = u32::from(self.pane_effective_scale_pct(id));
        if pane_scale == vp_scale {
            return outer;
        }
        if self.is_framed() {
            let scaled_inner = self.pane_scaled_inner_rect(id, crate::wm::inset_window_frame(outer));
            Rect::new(
                outer.x,
                outer.y,
                scaled_inner.width.saturating_add(2),
                scaled_inner.height.saturating_add(2),
            )
        } else {
            self.pane_scaled_inner_rect(id, outer)
        }
    }

    /// Synchronizes `config.font_scale_pct` with the focused pane's independent scale percentage.
    pub fn sync_config_scale_from_active_pane(&mut self) {
        let pct = self.pane_effective_scale_pct(self.active_pane_id());
        self.config_mut().font_scale_pct = pct.clamp(50, 300);
    }

    fn pin_other_panes_default_scale(&mut self) {
        let active_id = self.active_pane_id();
        for (&id, pane) in self.panes_mut().iter_mut() {
            if id != active_id && pane.font_options().scale_pct.is_none() {
                pane.font_options_mut().scale_pct = Some(100);
            }
        }
    }

    /// Applies the current font scale to the active pane's OIA status bar, resizes its PTY/VT screen
    /// grid to match the new font scale, and queues an `OSC 50` host-terminal font passthrough sequence (`FONT-01`).
    fn notify_font_scale_changed(&mut self) -> String {
        let active_id = self.active_pane_id();
        let pct = self.pane_effective_scale_pct(active_id);
        self.config_mut().font_scale_pct = pct.clamp(50, 300);
        let pt = self.config().scaled_font_size_pt();
        let (cw, ch) = self.config().scaled_cell_dimensions_px();
        let passthrough = self.config().host_font_passthrough;
        let (rows, cols) = self.default_size;
        let _ = self.resize_all(Rect::new(0, 0, cols.max(1), rows.max(1)));
        let (vt_rows, vt_cols) = self.active_pane().vt().size();
        let msg = format!(
            "PANE #{active_id} FONT SIZE: {pt}pt ({pct}%) [cell {cw}x{ch}px, screen {vt_cols}x{vt_rows}]"
        );
        let pane = self.active_pane_mut();
        if passthrough {
            pane.set_last_osc52_sequence(Some(encode_host_font_passthrough(pt, pct)));
        }
        pane.set_status_message(Some(msg.clone()));
        msg
    }

    /// Increases the active pane's font scale by `+10%` (`Ctrl++` / `Ctrl+=` / `:fontsize +`) (`FONT-01`).
    pub fn font_zoom_in(&mut self) -> String {
        self.pin_other_panes_default_scale();
        let cur = self.pane_effective_scale_pct(self.active_pane_id());
        let next = (cur + 10).clamp(50, 300);
        self.active_pane_mut().font_options_mut().scale_pct = Some(next);
        self.config_mut().font_scale_pct = next;
        self.notify_font_scale_changed()
    }

    /// Decreases the active pane's font scale by `-10%` (`Ctrl+-` / `:fontsize -`) (`FONT-01`).
    pub fn font_zoom_out(&mut self) -> String {
        self.pin_other_panes_default_scale();
        let cur = self.pane_effective_scale_pct(self.active_pane_id());
        let next = cur.saturating_sub(10).clamp(50, 300);
        self.active_pane_mut().font_options_mut().scale_pct = Some(next);
        self.config_mut().font_scale_pct = next;
        self.notify_font_scale_changed()
    }

    /// Resets the active pane's font scale to `100%` (`Ctrl+0` / `:fontsize reset`) (`FONT-01`).
    pub fn font_zoom_reset(&mut self) -> String {
        self.pin_other_panes_default_scale();
        let opts = self.active_pane_mut().font_options_mut();
        opts.scale_pct = Some(100);
        opts.size_pt = None;
        self.config_mut().reset_font_scale();
        self.notify_font_scale_changed()
    }

    /// Sets the active pane's font size in points (`6..=49`) or scale percentage (`50..=300`) (`:fontsize <val>`) (`FONT-01`).
    pub fn font_set_size(&mut self, value: u16) -> String {
        self.pin_other_panes_default_scale();
        self.config_mut().set_font_size_or_scale(value);
        let next = self.config().font_scale_pct;
        let opts = self.active_pane_mut().font_options_mut();
        opts.scale_pct = Some(next);
        opts.size_pt = None;
        self.notify_font_scale_changed()
    }

    /// Synchronizes terminal font scaling with `:focalzoom` when in text mode so `:focalzoom`
    /// scales the font larger instead of double-spacing characters (`FONT-01`).
    pub fn sync_focal_zoom_font_scale(&mut self) {
        let has_canvas = self.active_pane().vt().tektronix().has_visible_content()
            || self.active_pane().canvas_scene().has_visible_content();
        if has_canvas {
            return;
        }
        let target_pct = if self.config().focal_zoom_mode == crate::ui::cursor::SpotlightMode::Off {
            100
        } else {
            self.config().focal_zoom_pct.clamp(50, 300)
        };
        self.config_mut().font_scale_pct = target_pct;
        let pt = self.config().scaled_font_size_pt();
        let passthrough = self.config().host_font_passthrough;
        if passthrough {
            self.active_pane_mut()
                .set_last_osc52_sequence(Some(encode_host_font_passthrough(pt, target_pct)));
        }
    }

    /// Recalculates the terminal character grid `(rows, cols)` that fits inside a pixel window
    /// of `(width_px, height_px)` at the current scaled cell dimensions and resizes all workspace panes (`FONT-01`).
    pub fn resize_for_pixel_window(&mut self, width_px: u32, height_px: u32) -> Result<Rect> {
        let (cw, ch) = self.config().scaled_cell_dimensions_px();
        let cols = (width_px / u32::from(cw.max(1))).clamp(20, 500) as u16;
        let rows = (height_px / u32::from(ch.max(1))).clamp(6, 200) as u16;
        let area = Rect::new(0, 0, cols, rows);
        self.set_viewport_scale_pct(self.config().font_scale_pct);
        self.resize_all(area)?;
        Ok(area)
    }

    /// Executes a font scaling [`Action`](crate::action::Action) (`FontZoomIn`, `FontZoomOut`, `FontZoomReset`, `FontSetSize`) (`FONT-01`).
    pub fn execute_font_action(&mut self, action: &crate::action::Action) -> String {
        match *action {
            crate::action::Action::FontZoomIn => self.font_zoom_in(),
            crate::action::Action::FontZoomOut => self.font_zoom_out(),
            crate::action::Action::FontZoomReset => self.font_zoom_reset(),
            crate::action::Action::FontSetSize(val) => self.font_set_size(val),
            _ => self.notify_font_scale_changed(),
        }
    }

    /// Executes a per-pane or targeted `:font <subcommand>` (`FONT-02`).
    pub fn execute_pane_font_command(&mut self, raw: &str) -> std::result::Result<String, String> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        let cfg = self.config().clone();
        if parts.first().copied() == Some("pane") {
            let Some(id_str) = parts.get(1) else {
                return Err("Usage: :font pane <id> <family|size|scale|bold|italic|double|reset>".to_string());
            };
            let id = id_str
                .parse::<usize>()
                .map_err(|_| format!("Invalid pane id '{id_str}'"))?;
            let sub = parts.get(2..).map(|s| s.join(" ")).unwrap_or_default();
            let Some(pane) = self.pane_mut(id) else {
                return Err(format!("No pane with id #{id}"));
            };
            return pane.execute_font_command(&cfg, &sub);
        }
        if parts.first().copied() == Some("all") {
            let sub = parts.get(1..).map(|s| s.join(" ")).unwrap_or_default();
            let ids: Vec<usize> = self.panes().keys().copied().collect();
            let mut last_msg = String::new();
            for id in ids {
                if let Some(p) = self.pane_mut(id) {
                    last_msg = p.execute_font_command(&cfg, &sub)?;
                }
            }
            if let Some(opts) = self.active_pane().font_options().clone().into() {
                self.config_mut().font_family = opts.family.display_name().to_string();
                if let Some(pt) = opts.size_pt {
                    self.config_mut().font_size_pt = pt;
                }
                if let Some(pct) = opts.scale_pct {
                    self.config_mut().font_scale_pct = pct;
                }
            }
            return Ok(format!("ALL PANES {last_msg}"));
        }
        let res = self.active_pane_mut().execute_font_command(&cfg, raw);
        if res.is_ok() {
            let opts = self.active_pane().font_options().clone();
            self.config_mut().font_family = opts.family.display_name().to_string();
            let (rows, cols) = self.default_size;
            let _ = self.resize_all(Rect::new(0, 0, cols.max(1), rows.max(1)));
        }
        res
    }
}

impl PaneFontOptions {
    pub fn effective_size_pt(&self, cfg: &crate::config::BiltermConfig) -> u16 {
        self.effective_size_pt_with_defaults(cfg.font_size_pt, cfg.font_scale_pct)
    }

    pub fn effective_scale_pct(&self, cfg: &crate::config::BiltermConfig) -> u16 {
        self.effective_scale_pct_with_default(cfg.font_scale_pct)
    }

    pub fn summary(&self, cfg: &crate::config::BiltermConfig) -> String {
        self.summary_with_defaults(cfg.font_size_pt, cfg.font_scale_pct)
    }

    pub fn execute_command(
        &mut self,
        cfg: &crate::config::BiltermConfig,
        raw: &str,
    ) -> std::result::Result<String, String> {
        self.execute_command_with_defaults(cfg.font_size_pt, cfg.font_scale_pct, raw)
    }
}

impl crate::pane::CompositePane {
    pub fn font_options(&self) -> &PaneFontOptions {
        &self.font_options
    }

    pub fn font_options_mut(&mut self) -> &mut PaneFontOptions {
        &mut self.font_options
    }

    /// Executes a per-pane `:font <subcommand>` (`FONT-02`).
    pub fn execute_font_command(
        &mut self,
        cfg: &crate::config::BiltermConfig,
        cmd: &str,
    ) -> std::result::Result<String, String> {
        match self.font_options.execute_command(cfg, cmd) {
            Ok(msg) => {
                if cfg.host_font_passthrough {
                    let pt = self.font_options.effective_size_pt(cfg);
                    let pct = self.font_options.effective_scale_pct(cfg);
                    let fam = self.font_options.family.display_name();
                    self.set_last_osc52_sequence(Some(format!(
                        "\x1b]50;family={fam};fontsize={pt};scale={pct}%\x07"
                    )));
                }
                self.set_status_message(Some(format!(
                    "[PANE #{}] {msg}",
                    self.id()
                )));
                Ok(msg)
            }
            Err(err) => {
                self.set_status_message(Some(err.clone()));
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use crate::input::{handle_wm_key_event, handle_wm_mouse_event};
    use crate::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
    use crate::wm::WindowManager;
    use crate::wm::split_tree::SplitDirection;

    #[test]
    fn per_pane_font_zoom_scales_screen_and_glyphs_independently_including_nmon_raw_pane() {
        let mut wm = WindowManager::new_headless(40, 120);
        let vp = Rect::new(0, 0, 120, 40);
        wm.resize_all(vp).unwrap();
        // 3-way split: left = p1, top-right = p2 (nmon), bottom-right = p3
        let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
        let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
        wm.resize_all(vp).unwrap();
        let p1 = 1;

        // Put top-right pane (p2) into DECSET 1049 ALT_SCREEN + !ECHO (simulating `nmon`)
        wm.focus_and_raise(p2);
        wm.active_pane_mut().process_pty_output(b"\x1b[?1049hNMON_TOP_RIGHT");
        wm.active_pane_mut().line_discipline_mut().echo_enabled = false;
        wm.active_pane_mut().line_discipline_mut().canonical_mode = false;

        let p1_initial = wm.pane(p1).unwrap().vt().size();
        let p2_initial = wm.pane(p2).unwrap().vt().size();
        let p3_initial = wm.pane(p3).unwrap().vt().size();

        // Press Ctrl++ twice on the active `nmon` pane (p2)
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('+'), KeyModifiers::CONTROL)).unwrap();
        handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('='), KeyModifiers::CONTROL)).unwrap();

        assert_eq!(wm.pane_effective_scale_pct(p1), 100);
        assert_eq!(wm.pane_effective_scale_pct(p2), 120);
        assert_eq!(wm.pane_effective_scale_pct(p3), 100);
        assert_eq!(wm.pane(p1).unwrap().vt().size(), p1_initial);
        assert_eq!(wm.pane(p3).unwrap().vt().size(), p3_initial);
        let p2_zoomed = wm.pane(p2).unwrap().vt().size();
        assert!(p2_zoomed.0 < p2_initial.0 && p2_zoomed.1 < p2_initial.1);

        // Press Ctrl+- three times on `nmon` pane (p2) -> 90% (more rows & cols than initial)
        for _ in 0..3 {
            handle_wm_key_event(&mut wm, KeyEvent::new(KeyCode::Char('-'), KeyModifiers::CONTROL)).unwrap();
        }
        assert_eq!(wm.pane_effective_scale_pct(p2), 90);
        let p2_shrunk = wm.pane(p2).unwrap().vt().size();
        assert!(p2_shrunk.0 > p2_initial.0 && p2_shrunk.1 > p2_initial.1);

        // Render GUI workspace and verify p2 emits scaled font_size_pt while p1 stays at base pt
        let mut renderer = GuiWorkspaceRenderer::new(10, 20);
        let mut surface = GuiFrameSurface::new(10, 20);
        renderer.render_workspace_with_cell_size(&wm, vp, 10, 20, &mut surface);
        let base_pt = wm.config().font_size_pt;
        let scaled_pt = ((u32::from(base_pt) * 90 + 50) / 100) as u16;
        assert!(surface.glyphs.iter().any(|g| g.x_px < 500.0 && g.font_size_pt == base_pt));
        assert!(surface.glyphs.iter().any(|g| g.x_px > 650.0 && g.y_px < 350.0 && g.font_size_pt == scaled_pt));
    }

    #[test]
    fn all_pane_frame_borders_and_corners_resize_in_three_pane_layout() {
        let mut wm = WindowManager::new_headless(40, 120);
        let vp = Rect::new(0, 0, 120, 40);
        wm.resize_all(vp).unwrap();
        let p2 = wm.split_focused(SplitDirection::Vertical).unwrap();
        let p3 = wm.split_focused(SplitDirection::Horizontal).unwrap();
        wm.resize_all(vp).unwrap();
        let p1 = 1;

        let rects0 = wm.compute_workspace_rects(vp);
        let r2 = rects0.iter().find(|(id, _)| *id == p2).unwrap().1;
        let r3 = rects0.iter().find(|(id, _)| *id == p3).unwrap().1;

        // 1. Drag bottom-right corner of top-right pane (p2) straight down: grows p2 height without collapsing width!
        let br_x = r2.x + r2.width - 1;
        let br_y = r2.y + r2.height - 1;
        handle_wm_mouse_event(&mut wm, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: br_x, row: br_y, modifiers: KeyModifiers::NONE }, vp).unwrap();
        handle_wm_mouse_event(&mut wm, MouseEvent { kind: MouseEventKind::Drag(MouseButton::Left), column: br_x, row: br_y + 8, modifiers: KeyModifiers::NONE }, vp).unwrap();
        handle_wm_mouse_event(&mut wm, MouseEvent { kind: MouseEventKind::Up(MouseButton::Left), column: br_x, row: br_y + 8, modifiers: KeyModifiers::NONE }, vp).unwrap();

        let rects1 = wm.compute_workspace_rects(vp);
        let r2_after = rects1.iter().find(|(id, _)| *id == p2).unwrap().1;
        assert_eq!(r2_after.width, r2.width, "vertical split must not collapse when dragging BottomRight straight down");
        assert!(r2_after.height > r2.height, "top-right pane height must grow");

        // 2. Drag top border of bottom-right pane (p3) upward: grows p3 without swapping panes
        let r3_cur = rects1.iter().find(|(id, _)| *id == p3).unwrap().1;
        let mid_col = r3_cur.x + r3_cur.width / 2;
        handle_wm_mouse_event(&mut wm, MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: mid_col, row: r3_cur.y, modifiers: KeyModifiers::NONE }, vp).unwrap();
        handle_wm_mouse_event(&mut wm, MouseEvent { kind: MouseEventKind::Drag(MouseButton::Left), column: mid_col, row: r3.y - 4, modifiers: KeyModifiers::NONE }, vp).unwrap();
        handle_wm_mouse_event(&mut wm, MouseEvent { kind: MouseEventKind::Up(MouseButton::Left), column: mid_col, row: r3.y - 4, modifiers: KeyModifiers::NONE }, vp).unwrap();

        let rects2 = wm.compute_workspace_rects(vp);
        let r2_final = rects2.iter().find(|(id, _)| *id == p2).unwrap().1;
        let r3_final = rects2.iter().find(|(id, _)| *id == p3).unwrap().1;
        let r1_final = rects2.iter().find(|(id, _)| *id == p1).unwrap().1;
        assert!(r2_final.y < r3_final.y && r1_final.x < r2_final.x);
        assert!(r3_final.height > r3.height);
    }
}
