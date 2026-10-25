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
        if let Some(cmd) = params.get("command").and_then(JsonValue::as_str) {
            self.execute_pane_font_command(cmd)?;
        }
        let (cw, ch) = self.config().scaled_cell_dimensions_px();
        let pane_font = self.active_pane().font_options();
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
            (
                "pane_font_family",
                JsonValue::String(pane_font.family.display_name().to_string()),
            ),
            (
                "pane_font_size_pt",
                JsonValue::Number(i64::from(pane_font.effective_size_pt(self.config()))),
            ),
            (
                "pane_font_scale_pct",
                JsonValue::Number(i64::from(pane_font.effective_scale_pct(self.config()))),
            ),
            ("pane_font_bold", JsonValue::Bool(pane_font.bold)),
            ("pane_font_italic", JsonValue::Bool(pane_font.italic)),
            (
                "pane_font_double_width",
                JsonValue::Bool(pane_font.double_width),
            ),
        ]))
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
            return Ok(format!("ALL PANES {last_msg}"));
        }
        self.active_pane_mut().execute_font_command(&cfg, raw)
    }
}

/// Selectable font family preset per pane or window (`FONT-02`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontFamilyPreset {
    /// Classic IBM 3270 Mainframe phosphor terminal font (`"IBM 3270"`).
    #[default]
    Ibm3270,
    /// Classic DOS Borland Turbo Vision 8×16 bitmap font (`"PxPlus IBM VGA8"`).
    BorlandDosVga,
    /// Classic 1974 Tektronix 4014 storage-tube vector stroke font (`"Tektronix 4014"`).
    Tektronix4014,
    /// Modern developer ligature font (`"JetBrains Mono"`).
    JetBrainsMono,
    /// Modern functional programming font (`"Fira Code"`).
    FiraCode,
    /// High-density slender terminal font (`"Iosevka Term"`).
    IosevkaTerm,
    /// Microsoft terminal font (`"Cascadia Code"`).
    CascadiaCode,
    /// Clean open-source monospace font (`"Hack"`).
    Hack,
    /// User-specified custom font family name.
    Custom(String),
}

impl FontFamilyPreset {
    pub fn from_name(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ibm3270" | "3270" | "ibm" | "mainframe" => Some(Self::Ibm3270),
            "vga" | "vga8" | "dos" | "borland" | "cp437" | "turbo" => Some(Self::BorlandDosVga),
            "tek" | "tek4014" | "tektronix" | "vector" => Some(Self::Tektronix4014),
            "jetbrains" | "jetbrains-mono" | "jbmono" | "jb" => Some(Self::JetBrainsMono),
            "fira" | "firacode" | "fira-code" => Some(Self::FiraCode),
            "iosevka" | "iosevka-term" => Some(Self::IosevkaTerm),
            "cascadia" | "cascadia-code" => Some(Self::CascadiaCode),
            "hack" => Some(Self::Hack),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Ibm3270 => "IBM 3270",
            Self::BorlandDosVga => "PxPlus IBM VGA8",
            Self::Tektronix4014 => "Tektronix 4014",
            Self::JetBrainsMono => "JetBrains Mono",
            Self::FiraCode => "Fira Code",
            Self::IosevkaTerm => "Iosevka Term",
            Self::CascadiaCode => "Cascadia Code",
            Self::Hack => "Hack",
            Self::Custom(name) => name.as_str(),
        }
    }

    pub fn short_badge(&self) -> &str {
        match self {
            Self::Ibm3270 => "3270",
            Self::BorlandDosVga => "VGA8",
            Self::Tektronix4014 => "TEK",
            Self::JetBrainsMono => "JBMono",
            Self::FiraCode => "Fira",
            Self::IosevkaTerm => "Iosevka",
            Self::CascadiaCode => "Cascadia",
            Self::Hack => "Hack",
            Self::Custom(name) => name.as_str(),
        }
    }
}

/// Per-pane / per-window selectable typography options (`FONT-02`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneFontOptions {
    pub family: FontFamilyPreset,
    pub size_pt: Option<u16>,
    pub scale_pct: Option<u16>,
    pub bold: bool,
    pub italic: bool,
    pub double_width: bool,
}

impl PaneFontOptions {
    pub fn effective_size_pt(&self, cfg: & crate::config::BiltermConfig) -> u16 {
        let base = self.size_pt.unwrap_or(cfg.font_size_pt).clamp(6, 72);
        let pct = self.effective_scale_pct(cfg);
        ((u32::from(base) * u32::from(pct) + 50) / 100).clamp(6, 96) as u16
    }

    pub fn effective_scale_pct(&self, cfg: &crate::config::BiltermConfig) -> u16 {
        self.scale_pct.unwrap_or(cfg.font_scale_pct).clamp(50, 400)
    }

    pub fn has_custom_overrides(&self) -> bool {
        self.family != FontFamilyPreset::Ibm3270
            || self.size_pt.is_some()
            || self.scale_pct.is_some()
            || self.bold
            || self.italic
            || self.double_width
    }

    pub fn summary(&self, cfg: &crate::config::BiltermConfig) -> String {
        format!(
            "family='{}' size={}pt ({}%) bold={} italic={} double_width={}",
            self.family.display_name(),
            self.effective_size_pt(cfg),
            self.effective_scale_pct(cfg),
            self.bold,
            self.italic,
            self.double_width
        )
    }

    pub fn execute_command(
        &mut self,
        cfg: &crate::config::BiltermConfig,
        raw: &str,
    ) -> std::result::Result<String, String> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        match parts.first().copied().unwrap_or("status") {
            "status" => Ok(format!("PANE FONT: {}", self.summary(cfg))),
            "list" | "presets" => Ok(
                "FONT PRESETS: ibm3270 | vga (Borland DOS) | tek4014 | jetbrains | firacode | iosevka | cascadia | hack | custom <name>"
                    .to_string(),
            ),
            "reset" | "default" => {
                *self = Self::default();
                Ok(format!("PANE FONT RESET: {}", self.summary(cfg)))
            }
            "+" | "in" | "up" | "bigger" => {
                let cur = self.effective_scale_pct(cfg);
                self.scale_pct = Some((cur + 10).clamp(50, 400));
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            "-" | "out" | "down" | "smaller" => {
                let cur = self.effective_scale_pct(cfg);
                self.scale_pct = Some(cur.saturating_sub(10).clamp(50, 400));
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            "bold" => {
                self.bold = match parts.get(1).copied() {
                    Some("on" | "true" | "1") => true,
                    Some("off" | "false" | "0") => false,
                    _ => !self.bold,
                };
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            "italic" => {
                self.italic = match parts.get(1).copied() {
                    Some("on" | "true" | "1") => true,
                    Some("off" | "false" | "0") => false,
                    _ => !self.italic,
                };
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            "double" | "2x" | "wide" => {
                self.double_width = match parts.get(1).copied() {
                    Some("on" | "true" | "1") => true,
                    Some("off" | "false" | "0") => false,
                    _ => !self.double_width,
                };
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            "size" => {
                let Some(v) = parts.get(1).and_then(|s| s.trim_end_matches("pt").parse::<u16>().ok()) else {
                    return Err("Usage: :font size <6..72>".to_string());
                };
                self.size_pt = Some(v.clamp(6, 72));
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            "scale" => {
                let Some(v) = parts.get(1).and_then(|s| s.trim_end_matches('%').parse::<u16>().ok()) else {
                    return Err("Usage: :font scale <50..400>%".to_string());
                };
                self.scale_pct = Some(v.clamp(50, 400));
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            "custom" | "family" => {
                let name = parts.get(1..).map(|s| s.join(" ")).unwrap_or_default();
                if name.trim().is_empty() {
                    return Err("Usage: :font custom <Font Family Name>".to_string());
                }
                if let Some(preset) = FontFamilyPreset::from_name(&name) {
                    self.family = preset;
                } else {
                    self.family = FontFamilyPreset::Custom(name);
                }
                Ok(format!("PANE FONT: {}", self.summary(cfg)))
            }
            other => {
                if let Some(preset) = FontFamilyPreset::from_name(other) {
                    self.family = preset;
                    if let Some(arg2) = parts.get(1).copied() {
                        if let Some(pt) = arg2.strip_suffix("pt").and_then(|s| s.parse::<u16>().ok()) {
                            self.size_pt = Some(pt.clamp(6, 72));
                        } else if let Some(pct) = arg2.strip_suffix('%').and_then(|s| s.parse::<u16>().ok()) {
                            self.scale_pct = Some(pct.clamp(50, 400));
                        } else if let Ok(v) = arg2.parse::<u16>() {
                            if v < 50 {
                                self.size_pt = Some(v.clamp(6, 72));
                            } else {
                                self.scale_pct = Some(v.clamp(50, 400));
                            }
                        }
                    }
                    return Ok(format!("PANE FONT: {}", self.summary(cfg)));
                }
                if let Some(pt) = other.strip_suffix("pt").and_then(|s| s.parse::<u16>().ok()) {
                    self.size_pt = Some(pt.clamp(6, 72));
                    return Ok(format!("PANE FONT: {}", self.summary(cfg)));
                }
                if let Some(pct) = other.strip_suffix('%').and_then(|s| s.parse::<u16>().ok()) {
                    self.scale_pct = Some(pct.clamp(50, 400));
                    return Ok(format!("PANE FONT: {}", self.summary(cfg)));
                }
                if let Ok(v) = other.parse::<u16>() {
                    if v < 50 {
                        self.size_pt = Some(v.clamp(6, 72));
                    } else {
                        self.scale_pct = Some(v.clamp(50, 400));
                    }
                    return Ok(format!("PANE FONT: {}", self.summary(cfg)));
                }
                Err(format!(
                    "Unknown :font option '{other}'. Usage: :font <ibm3270|vga|tek4014|jetbrains|firacode|iosevka|cascadia|hack|custom <name>|size <pt>|scale <pct>%|+|-|bold|italic|double|pane <id> ...|all ...|reset|list>"
                ))
            }
        }
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
