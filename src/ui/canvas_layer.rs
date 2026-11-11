//! The cacheable base layer of a pane's `OutputCanvas`: background fill, VT cells with WCAG
//! contrast, URL and trigger highlights, CRT scanlines.
//!
//! [`paint_canvas_layer`] is a pure function of the [`VtParser`] grid and [`CanvasLayerInputs`].
//! It deliberately takes neither the pane nor the config, so it cannot read an input that the
//! cache key in [`crate::ui::canvas_cache`] does not cover. A new input goes into
//! `CanvasLayerInputs`; the key destructures that struct exhaustively and stops compiling until
//! the field is handled.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use crate::config::BiltermConfig;
use crate::pane::CompositePane;
use crate::ui::theme::{ThemePalette, ThemePreset};
use crate::ui::typography::{self, LineSizeAttr};
use crate::ui::{contrast, gui_font};
use crate::vt::VtParser;

/// Every input of the canvas base layer other than the VT grid itself (which is identified by
/// `VtParser::generation`, the scrollback offset and the grid size).
#[derive(Clone, Copy, Debug)]
pub struct CanvasLayerInputs<'a> {
    /// Absolute canvas rectangle in the frame buffer.
    pub canvas: Rect,
    /// Theme preset: selects the ANSI 0..15 table and the fallback foreground.
    pub theme: ThemePreset,
    /// Resolved palette (global, per-window theme or per-window colour overrides).
    pub palette: &'a ThemePalette,
    /// Per-pane font option: force bold.
    pub font_bold: bool,
    /// Per-pane font option: force italic.
    pub font_italic: bool,
    /// Per-pane font option: render every row double-width.
    pub font_double_width: bool,
    /// Output triggers highlighted on the canvas.
    pub triggers: &'a [String],
    /// CRT scanline dimming on odd rows.
    pub crt_scanlines: bool,
}

impl<'a> CanvasLayerInputs<'a> {
    /// Collects the layer inputs for `pane` drawn into `canvas` with `cfg` and `palette`.
    pub fn gather(
        pane: &CompositePane,
        canvas: Rect,
        cfg: &'a BiltermConfig,
        palette: &'a ThemePalette,
    ) -> Self {
        let font = pane.font_options();
        Self {
            canvas,
            theme: cfg.theme,
            palette,
            font_bold: font.bold,
            font_italic: font.italic,
            font_double_width: font.double_width,
            triggers: &cfg.triggers,
            crt_scanlines: cfg.crt_scanlines,
        }
    }
}

fn is_safe_terminal_cluster(s: &str, expected_width: usize, grapheme_mode_2027: bool) -> bool {
    if s.is_empty() {
        return false;
    }
    let w = unicode_width::UnicodeWidthStr::width(s);
    if w != expected_width {
        let first_ok = expected_width == 1
            && s.chars().next().is_some_and(|ch| {
                gui_font::geometric::is_private_use_area(ch)
                    || gui_font::geometric::is_geometric_glyph(ch)
                    || unicode_width::UnicodeWidthChar::width(ch) == Some(1)
            });
        if !first_ok {
            return false;
        }
    }
    for ch in s.chars() {
        let code = ch as u32;
        if matches!(code, 0xFE00..=0xFE0F) || (grapheme_mode_2027 && code == 0x200D) {
            continue;
        }
        if ch.is_control()
            || matches!(
                code,
                0x200B..=0x200F | 0x2028..=0x202F | 0x2060..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB
            )
        {
            return false;
        }
    }
    true
}

/// Paints the canvas base layer into `buf`. Every cell of `inputs.canvas` that lies inside `buf`
/// gets its symbol, foreground and background replaced and modifiers added (never removed), which
/// is what lets [`crate::ui::canvas_cache`] replay a stored layer over any prior buffer content.
pub fn paint_canvas_layer(vt: &VtParser, inputs: &CanvasLayerInputs<'_>, buf: &mut Buffer) {
    let canvas = inputs.canvas;
    let palette = inputs.palette;
    let (vt_rows, vt_cols) = vt.size();
    let render_rows = canvas.height.min(vt_rows);
    let render_cols = canvas.width.min(vt_cols);
    let grapheme_2027 = vt.is_grapheme_cluster_2027();
    let fill_style = Style::default()
        .fg(palette.text_fg.unwrap_or(palette.title_fg))
        .bg(palette.window_frame_bg);
    for y in canvas.y..canvas.y + canvas.height {
        for x in canvas.x..canvas.x + canvas.width {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_symbol(" ");
                c.set_style(fill_style);
            }
        }
    }

    // Runs of cells with the same colours and attributes are the norm; reuse the last style
    // instead of redoing the contrast maths (the style depends on nothing else in the cell).
    let mut last_style: Option<(contrast::CellStyleKey, Style)> = None;
    for r in 0..render_rows {
        let mut line_attr = vt.line_size_attr(r);
        if !line_attr.is_double_width() && inputs.font_double_width {
            line_attr = LineSizeAttr::DoubleWidth;
        }
        if line_attr.is_double_width() {
            typography::render_double_width_row(vt, r, canvas, render_cols, line_attr, buf);
            continue;
        }
        let mut skip_next_wide_continuation = false;
        for c in 0..render_cols {
            let Some(vt_cell) = vt.cell_view(r, c) else {
                skip_next_wide_continuation = false;
                continue;
            };
            let dst_x = canvas.x + c;
            let dst_y = canvas.y + r;
            let style_key = contrast::CellStyleKey::of(&vt_cell);
            let mut style = match last_style {
                Some((key, style)) if key == style_key => style,
                _ => {
                    let style = contrast::readable_canvas_cell_style(&vt_cell, inputs.theme, palette);
                    last_style = Some((style_key, style));
                    style
                }
            };
            if inputs.font_bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if inputs.font_italic {
                style = style.add_modifier(Modifier::ITALIC);
            }

            if skip_next_wide_continuation {
                skip_next_wide_continuation = false;
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_style(style);
                }
                continue;
            }

            if vt_cell.is_wide_continuation() {
                if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_symbol(" ");
                    dst_cell.set_style(style);
                }
                continue;
            }

            let contents = vt_cell.grapheme;
            if vt_cell.is_wide() {
                // Prevent a 2-column wide glyph at the rightmost column from overflowing/wrapping the host terminal
                if c + 1 < render_cols && is_safe_terminal_cluster(contents, 2, grapheme_2027) {
                    if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                        dst_cell.set_symbol(contents);
                        dst_cell.set_style(style);
                    }
                    skip_next_wide_continuation = true;
                } else if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                    dst_cell.set_symbol(" ");
                    dst_cell.set_style(style);
                }
            } else if let Some(dst_cell) = buf.cell_mut((dst_x, dst_y)) {
                if contents.is_empty() {
                    dst_cell.set_symbol(" ");
                } else if is_safe_terminal_cluster(contents, 1, grapheme_2027) {
                    if !grapheme_2027 && contents.contains(['\u{FE0E}', '\u{FE0F}'])
                        && let Some(base) = contents.chars().next()
                    {
                        let mut sym = [0u8; 4];
                        dst_cell.set_symbol(base.encode_utf8(&mut sym));
                    } else {
                        dst_cell.set_symbol(contents);
                    }
                } else {
                    dst_cell.set_symbol("·");
                }
                dst_cell.set_style(style);
            }
        }
    }

    // Highlight detected URLs with underline + bright cyan tint (VAL-03)
    for url in vt.detect_visible_urls() {
        if url.row < render_rows {
            for c in url.start_col..=url.end_col.min(render_cols.saturating_sub(1)) {
                if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + url.row)) {
                    cell.set_style(
                        cell.style()
                            .fg(Color::Rgb(85, 255, 255))
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
            }
        }
    }

    // Highlight custom output triggers (v0.7.0)
    for trigger in inputs.triggers {
        for m in vt.search_canvas(trigger) {
            if m.row < render_rows {
                for c in m.start_col..=m.end_col.min(render_cols.saturating_sub(1)) {
                    if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + m.row)) {
                        cell.set_style(
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Rgb(200, 0, 80))
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                }
            }
        }
    }

    // Apply CRT phosphor scanline effect on odd rows when enabled (v0.6.0)
    if inputs.crt_scanlines {
        for r in (1..render_rows).step_by(2) {
            for c in 0..render_cols {
                if let Some(cell) = buf.cell_mut((canvas.x + c, canvas.y + r)) {
                    cell.set_style(cell.style().add_modifier(Modifier::DIM));
                }
            }
        }
    }
}
