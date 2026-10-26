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
    /// Applies the current font scale to the active pane's OIA status bar and queues an `OSC 50`
    /// host-terminal font passthrough sequence when `config.host_font_passthrough` is enabled (`FONT-01`).
    fn notify_font_scale_changed(&mut self) -> String {
        let pt = self.config().scaled_font_size_pt();
        let pct = self.config().font_scale_pct;
        let (cw, ch) = self.config().scaled_cell_dimensions_px();
        let passthrough = self.config().host_font_passthrough;
        let msg = format!("FONT SIZE: {pt}pt ({pct}%) [cell {cw}x{ch}px]");
        let pane = self.active_pane_mut();
        if passthrough {
            pane.set_last_osc52_sequence(Some(encode_host_font_passthrough(pt, pct)));
        }
        pane.set_status_message(Some(msg.clone()));
        msg
    }

    /// Increases workspace font scale by `+10%` (`Ctrl++` / `Ctrl+=` / `:fontsize +`) (`FONT-01`).
    pub fn font_zoom_in(&mut self) -> String {
        self.config_mut().step_font_scale(10);
        self.notify_font_scale_changed()
    }

    /// Decreases workspace font scale by `-10%` (`Ctrl+-` / `:fontsize -`) (`FONT-01`).
    pub fn font_zoom_out(&mut self) -> String {
        self.config_mut().step_font_scale(-10);
        self.notify_font_scale_changed()
    }

    /// Resets workspace font scale to `100%` (`Ctrl+0` / `:fontsize reset`) (`FONT-01`).
    pub fn font_zoom_reset(&mut self) -> String {
        self.config_mut().reset_font_scale();
        self.notify_font_scale_changed()
    }

    /// Sets workspace font size in points (`6..=49`) or scale percentage (`50..=300`) (`:fontsize <val>`) (`FONT-01`).
    pub fn font_set_size(&mut self, value: u16) -> String {
        self.config_mut().set_font_size_or_scale(value);
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
            if let Some(pt) = opts.size_pt {
                self.config_mut().font_size_pt = pt;
            }
            if let Some(pct) = opts.scale_pct {
                self.config_mut().font_scale_pct = pct;
            }
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
