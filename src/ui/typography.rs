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

use std::collections::BTreeMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::vt::VtParser;
use crate::wm::WindowManager;

/// Per-row DEC VT100/VT220 line-size attribute (`DECSWL`, `DECDWL`, `DECDHL`) (`FONT-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineSizeAttr {
    /// Standard 1x1 character cells (`\x1b#5` `DECSWL`).
    #[default]
    Single,
    /// Double-width 2x1 character cells (`\x1b#6` `DECDWL`).
    DoubleWidth,
    /// Double-height top-half 2x2 character cells (`\x1b#3` `DECDHL`).
    DoubleHeightTop,
    /// Double-height bottom-half 2x2 character cells (`\x1b#4` `DECDHL`).
    DoubleHeightBottom,
}

impl LineSizeAttr {
    /// Returns `true` when each logical character occupies 2 horizontal cells (`DECDWL` or `DECDHL`).
    #[inline]
    pub const fn is_double_width(self) -> bool {
        !matches!(self, Self::Single)
    }

    /// Returns `true` when the row is part of a `DECDHL` double-height pair (`\x1b#3` or `\x1b#4`).
    #[inline]
    pub const fn is_double_height(self) -> bool {
        matches!(self, Self::DoubleHeightTop | Self::DoubleHeightBottom)
    }

    /// Human-readable badge for the 3270 Inspector and diagnostics.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Single => "1x1 (DECSWL)",
            Self::DoubleWidth => "2x1 (DECDWL)",
            Self::DoubleHeightTop => "2x2-TOP (DECDHL)",
            Self::DoubleHeightBottom => "2x2-BOT (DECDHL)",
        }
    }
}

/// Scans a raw PTY byte stream for DEC line-size escape sequences (`\x1b#3`, `\x1b#4`, `\x1b#5`, `\x1b#6`),
/// tracking intermediate newlines and CSI cursor-positioning sequences so multi-row bursts assign
/// line-size attributes to the exact target row (`FONT-01`).
pub fn scan_dec_line_size_escapes(
    bytes: &[u8],
    start_row: u16,
    max_rows: u16,
    attrs: &mut BTreeMap<u16, LineSizeAttr>,
) {
    let limit = max_rows.max(1);
    let mut cur_row = start_row.min(limit.saturating_sub(1));
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                cur_row = (cur_row + 1).min(limit.saturating_sub(1));
                i += 1;
            }
            0x1b if i + 1 < bytes.len() => match bytes[i + 1] {
                b'#' if i + 2 < bytes.len() => {
                    match bytes[i + 2] {
                        b'3' => {
                            attrs.insert(cur_row, LineSizeAttr::DoubleHeightTop);
                        }
                        b'4' => {
                            attrs.insert(cur_row, LineSizeAttr::DoubleHeightBottom);
                        }
                        b'5' => {
                            attrs.remove(&cur_row);
                        }
                        b'6' => {
                            attrs.insert(cur_row, LineSizeAttr::DoubleWidth);
                        }
                        _ => {}
                    }
                    i += 3;
                }
                b'c' => {
                    attrs.clear();
                    cur_row = 0;
                    i += 2;
                }
                b'[' => {
                    let mut j = i + 2;
                    while j < bytes.len() && !(0x40..=0x7e).contains(&bytes[j]) {
                        j += 1;
                    }
                    if j < bytes.len() {
                        let final_byte = bytes[j];
                        let params = std::str::from_utf8(&bytes[i + 2..j]).unwrap_or("");
                        match final_byte {
                            b'H' | b'f' => {
                                let row_1 = params
                                    .split(';')
                                    .next()
                                    .and_then(|s| s.parse::<u16>().ok())
                                    .unwrap_or(1);
                                cur_row = row_1.saturating_sub(1).min(limit.saturating_sub(1));
                            }
                            b'J' if params == "2" || params == "3" => {
                                attrs.clear();
                            }
                            _ => {}
                        }
                        i = j + 1;
                    } else {
                        break;
                    }
                }
                _ => {
                    i += 2;
                }
            },
            _ => {
                i += 1;
            }
        }
    }
}

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

    /// Handles the JSON-RPC `workspace.fontsize` method (`FONT-01`).
    pub fn handle_rpc_fontsize(
        &mut self,
        params: &crate::api::JsonValue,
    ) -> std::result::Result<crate::api::JsonValue, String> {
        use crate::api::JsonValue;
        if let Some(action) = params.get("action").and_then(JsonValue::as_str) {
            match action.trim().to_ascii_lowercase().as_str() {
                "in" | "up" | "+" | "zoom_in" => {
                    self.font_zoom_in();
                }
                "out" | "down" | "-" | "zoom_out" => {
                    self.font_zoom_out();
                }
                "reset" | "0" | "default" => {
                    self.font_zoom_reset();
                }
                other => {
                    if let Ok(val) = other
                        .trim_end_matches("pt")
                        .trim_end_matches('%')
                        .parse::<u16>()
                    {
                        self.font_set_size(val);
                    } else {
                        return Err(format!("invalid workspace.fontsize action: {other}"));
                    }
                }
            }
        }
        if let Some(size) = params.get("size").and_then(JsonValue::as_usize) {
            self.font_set_size(size as u16);
        }
        if let Some(scale) = params.get("scale_pct").and_then(JsonValue::as_usize) {
            self.font_set_size((scale as u16).clamp(50, 300));
        }
        let (cw, ch) = self.config().scaled_cell_dimensions_px();
        Ok(JsonValue::obj([
            (
                "font_size_pt",
                JsonValue::Number(i64::from(self.config().scaled_font_size_pt())),
            ),
            (
                "base_font_size_pt",
                JsonValue::Number(i64::from(self.config().font_size_pt)),
            ),
            (
                "font_scale_pct",
                JsonValue::Number(i64::from(self.config().font_scale_pct)),
            ),
            ("cell_width_px", JsonValue::Number(i64::from(cw))),
            ("cell_height_px", JsonValue::Number(i64::from(ch))),
            (
                "host_font_passthrough",
                JsonValue::Bool(self.config().host_font_passthrough),
            ),
        ]))
    }
}
