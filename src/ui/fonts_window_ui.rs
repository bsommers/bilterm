//! FONTS utility window renderer (`WIN-01`, Phase 51.2).
//!
//! Borland double-line frame titled `FONTS (N)` with interactive scan/pin/dock/close badges,
//! active pane summary subheader, scrollable font family list with samples, and keyboard shortcut footer bar.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::wm::WindowManager;

/// Pads or truncates `s` to exact terminal column width `w`.
fn fit_pad(s: &str, w: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cw == 0 || used + cw > w {
            break;
        }
        used += cw;
        out.push(ch);
    }
    out.extend(std::iter::repeat_n(' ', w.saturating_sub(used)));
    out
}

/// Writes `text` at `(x, y)` clipped to `clip`.
fn put(buf: &mut Buffer, clip: Rect, x: u16, y: u16, text: &str, style: Style) {
    let mut x = x;
    let end = clip.x.saturating_add(clip.width);
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0
            || x < clip.x
            || x.saturating_add(w) > end
            || y < clip.y
            || y >= clip.y.saturating_add(clip.height)
        {
            if x >= end {
                break;
            }
            continue;
        }
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(ch.encode_utf8(&mut [0; 4]));
            cell.set_style(style);
        }
        x += w;
    }
}

/// Fills a region with spaces and `style`.
fn fill(buf: &mut Buffer, clip: Rect, target: Rect, style: Style) {
    let r = clip.intersection(target);
    for y in r.y..r.y.saturating_add(r.height) {
        for x in r.x..r.x.saturating_add(r.width) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

/// Renders the `FONTS` floating utility window when active.
pub fn render_fonts_window(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(rect) = wm.fonts_window_rect(workspace_area) else {
        return;
    };
    let rect = rect.intersection(buf.area);
    if rect.width < 12 || rect.height < 4 {
        return;
    }

    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);

    let tokens = DesignTokens::from_palette(palette);
    let bg = Style::default()
        .fg(tokens.text_primary)
        .bg(palette.window_frame_bg);
    fill(buf, rect, rect, bg);

    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }

    let rows = wm.fonts_window_rows();
    let state = wm.fonts_window();
    let title = format!(" FONTS ({}) ", rows.len());

    render_borland_frame_styled(
        rect,
        &title,
        state.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    // Title bar badges on right
    let r_edge = rect.x + rect.width;
    let badge_style = Style::default()
        .fg(palette.controls_fg.unwrap_or(palette.title_fg))
        .bg(palette.window_frame_bg)
        .add_modifier(Modifier::BOLD);

    if rect.width >= 24 {
        put(buf, rect, r_edge.saturating_sub(4), rect.y, "[×]", badge_style);
        put(buf, rect, r_edge.saturating_sub(8), rect.y, "[□]", badge_style);
        let pin_glyph = if state.pinned { "[•]" } else { "[o]" };
        put(buf, rect, r_edge.saturating_sub(12), rect.y, pin_glyph, badge_style);
        put(buf, rect, r_edge.saturating_sub(21), rect.y, "[s:Scan]", badge_style);
    }

    // Subheader row (current focused pane font summary)
    let active_id = wm.active_pane_id();
    let (fam_name, pt, pct) = if let Some(pane) = wm.pane(active_id) {
        (
            pane.font_options().family.display_name(),
            pane.font_options()
                .effective_size_pt_with_defaults(cfg.font_size_pt, cfg.font_scale_pct),
            pane.font_options()
                .effective_scale_pct_with_default(cfg.font_scale_pct),
        )
    } else {
        ("IBM 3270", cfg.font_size_pt, cfg.font_scale_pct)
    };
    let pin_str = if state.pinned { "ON" } else { "OFF" };
    let subhead = format!(" Active: {fam_name} {pt}pt ({pct}%) | Pin: {pin_str}");
    put(
        buf,
        rect,
        rect.x + 1,
        rect.y + 1,
        &fit_pad(&subhead, rect.width.saturating_sub(2) as usize),
        bg.fg(tokens.text_muted).add_modifier(Modifier::BOLD),
    );

    // Column header row
    if rect.height >= 6 {
        let col_header = "   FAMILY               BADGE     SAMPLE";
        put(
            buf,
            rect,
            rect.x + 1,
            rect.y + 2,
            &fit_pad(col_header, rect.width.saturating_sub(2) as usize),
            bg.fg(tokens.text_muted),
        );
    }

    // Scrollable rows
    let visible_rows = rect.height.saturating_sub(5).max(1) as usize;
    for (i, row) in rows.iter().skip(state.scroll).take(visible_rows).enumerate() {
        let y = rect.y + 3 + i as u16;
        let is_selected = row.index == state.selected;
        let cursor = if is_selected { "▶" } else { " " };
        let active_m = if row.active_on_focused { "*" } else { " " };
        let badge_fmt = format!("[{:^7}]", row.badge);
        let fam_fmt = fit_pad(&row.display_name, 19);
        let row_line = format!("{cursor}{active_m} {fam_fmt} {badge_fmt}  {}", row.sample);

        let row_style = if is_selected {
            Style::default()
                .fg(palette.window_frame_bg)
                .bg(tokens.accent)
                .add_modifier(Modifier::BOLD)
        } else if row.active_on_focused {
            bg.fg(tokens.text_primary).add_modifier(Modifier::BOLD)
        } else {
            bg.fg(tokens.text_primary)
        };

        put(
            buf,
            rect,
            rect.x + 1,
            y,
            &fit_pad(&row_line, rect.width.saturating_sub(2) as usize),
            row_style,
        );
    }

    // Footer bar
    if rect.height >= 5 {
        let footer_y = rect.y + rect.height.saturating_sub(2);
        let footer_text = " Enter:Apply  a:All  +/-:Size  [/]:Scale  s:Scan  *:Pin  Esc:Close";
        put(
            buf,
            rect,
            rect.x + 1,
            footer_y,
            &fit_pad(footer_text, rect.width.saturating_sub(2) as usize),
            bg.fg(tokens.accent).add_modifier(Modifier::BOLD),
        );
    }
}
