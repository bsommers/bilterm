//! AGENTS list floating window renderer (Phase 51-06, UIT-03, D-03): Borland frame titled `AGENTS`
//! with the pin badge and the min/dock/close trio, a column header, one row per tracked pane
//! (state dot, state word, name, `#id`, age), the selected row in reverse video, a marker on the
//! active pane's row, a scroll indicator, an empty-state line and the footer chips.
//!
//! Every region comes from [`WindowManager::agents_list_layout`], the same geometry the mouse
//! handler hit-tests, so what is drawn is what is clickable. Rendering has no input path of its own
//! and never reads pane screen text: names are identity tokens, sanitised again before drawing.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::BiltermConfig;
use crate::ui::agents_ui::pin_badge_glyph;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::drawer::navigator::{NavigatorGlyphs, dot_color};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::agents_list::AgentsListRow;
use crate::wm::agents_list_cmd::ListFilter;
use crate::wm::agents_list_geometry::{AgentsListRects, ListBadge, ListChip};

/// Window title.
pub const AGENTS_LIST_TITLE: &str = "AGENTS";
/// Title suffix shown while the window is docked into the tiling strip (when the title has room).
const DOCKED_HINT: &str = "AGENTS [docked]";
/// Leading cells of a row: marker, dot, blank, state word (7), blank.
const LEAD_W: usize = 11;
/// Cells of the `#id` column (blank + 5) and of the age column (blank + 5).
const PANE_COL_W: usize = 6;
const SINCE_COL_W: usize = 6;
/// Narrowest name column that is worth keeping alongside the optional columns.
const MIN_NAME_W: usize = 6;

/// Widths of the optional columns for a row area `w` cells wide: `(name_w, show_pane, show_since)`.
/// `SINCE` is dropped first, then `PANE`; a row never wraps.
fn columns(w: usize) -> (usize, bool, bool) {
    let name_w = |pane: bool, since: bool| {
        w.saturating_sub(LEAD_W + if pane { PANE_COL_W } else { 0 } + if since { SINCE_COL_W } else { 0 })
    };
    if name_w(true, true) >= MIN_NAME_W {
        (name_w(true, true), true, true)
    } else if name_w(true, false) >= MIN_NAME_W {
        (name_w(true, false), true, false)
    } else {
        (name_w(false, false), false, false)
    }
}

/// Compact age of a state: `7s`, `3m`, `2h`.
fn since_text(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m", s / 60),
        _ => format!("{}h", s / 3600),
    }
}

/// `s` cut to `w` terminal columns and padded with blanks to exactly `w` columns.
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
    out.extend(std::iter::repeat_n(' ', w - used));
    out
}

/// Writes `text` from `(x, y)` clipped to `clip` (and so to the buffer) by display width.
fn put(buf: &mut Buffer, clip: Rect, x: u16, y: u16, text: &str, style: Style) {
    let mut x = x;
    let end = clip.x.saturating_add(clip.width);
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if w == 0 || x < clip.x || x.saturating_add(w) > end || y < clip.y || y >= clip.y.saturating_add(clip.height) {
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

/// Fills the cells of `r` (clipped to `clip`) with blanks in `style`.
fn fill(buf: &mut Buffer, clip: Rect, r: Rect, style: Style) {
    let r = r.intersection(clip);
    for y in r.y..r.y + r.height {
        for x in r.x..r.x + r.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

/// Renders the AGENTS list window when open (above the AGENT TRAFFIC window, below modals).
pub fn render_agents_list(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(layout) = wm.agents_list_layout(workspace_area) else {
        return;
    };
    let rect = layout.outer.intersection(buf.area);
    if rect.width < 8 || rect.height < 4 {
        return;
    }
    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);
    let tokens = DesignTokens::from_palette(palette);
    let bg = Style::default().fg(tokens.text_primary).bg(palette.window_frame_bg);
    fill(buf, rect, rect, bg);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }
    let state = wm.agents_list();
    render_borland_frame_styled(
        rect,
        &title_text(wm, &layout),
        state.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );
    if let Some((_, r)) = layout.badges.iter().find(|(b, _)| *b == ListBadge::Pin) {
        let fg = if state.pinned { palette.title_fg } else { palette.controls_fg.unwrap_or(palette.title_fg) };
        let style = Style::default().fg(fg).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD);
        put(buf, rect, r.x, r.y, pin_badge_glyph(glyphs, state.pinned), style);
    }
    let rows = wm.agents_list_rows();
    render_header(&layout, &tokens, bg, rect, buf);
    render_rows(wm, &layout, &rows, (&tokens, palette), cfg, rect, buf);
    render_footer(wm, &layout, rows.len(), &tokens, bg, rect, buf);
}

/// Frame title: `AGENTS`, or `AGENTS [docked]` while docked and the title row has room before the
/// leftmost badge.
fn title_text(wm: &WindowManager, layout: &AgentsListRects) -> String {
    let outer = layout.outer;
    let room_end = layout.badges.iter().map(|(_, r)| r.x).min().unwrap_or(outer.x + outer.width);
    let needed = u16::try_from(DOCKED_HINT.len() + 2).unwrap_or(u16::MAX);
    if wm.agents_list().docked && outer.x.saturating_add(1).saturating_add(needed) <= room_end {
        DOCKED_HINT.to_string()
    } else {
        AGENTS_LIST_TITLE.to_string()
    }
}

/// Column titles (`STATE NAME PANE SINCE`); empty windows too short for a header draw none.
fn render_header(layout: &AgentsListRects, t: &DesignTokens, bg: Style, clip: Rect, buf: &mut Buffer) {
    let h = layout.header;
    if h.width == 0 || h.height == 0 {
        return;
    }
    let (name_w, pane, since) = columns(usize::from(layout.rows_area.width).saturating_sub(scroll_col(layout)));
    let mut line = format!("  {} {}", fit_pad("STATE", 7), fit_pad("NAME", name_w));
    if pane {
        line.push(' ');
        line.push_str(&fit_pad("PANE", PANE_COL_W - 1));
    }
    if since {
        line.push(' ');
        line.push_str(&fit_pad("SINCE", SINCE_COL_W - 1));
    }
    put(buf, clip, h.x, h.y, &line, bg.fg(t.text_muted).add_modifier(Modifier::BOLD));
}

/// Cells reserved on the right of the rows for the scroll indicator (fixed per window, so header
/// and rows share the same columns).
fn scroll_col(layout: &AgentsListRects) -> usize {
    usize::from(layout.visible_rows > 0 && layout.rows_area.width > LEAD_W as u16)
}

fn render_rows(
    wm: &WindowManager,
    layout: &AgentsListRects,
    rows: &[AgentsListRow],
    (t, palette): (&DesignTokens, &ThemePalette),
    cfg: &BiltermConfig,
    clip: Rect,
    buf: &mut Buffer,
) {
    let area = layout.rows_area;
    if area.width == 0 || area.height == 0 {
        return;
    }
    let state = wm.agents_list();
    let bg = Style::default().fg(t.text_primary).bg(palette.window_frame_bg);
    if rows.is_empty() {
        let msg = if state.filter == ListFilter::Agents { "No agents detected" } else { "No panes" };
        put(buf, clip, area.x + 1, area.y, msg, bg.fg(t.text_muted));
        return;
    }
    let g = NavigatorGlyphs::for_pack(cfg.glyphs);
    let tick = wm.drawer().spinner_tick();
    let reserve = scroll_col(layout);
    let text_w = usize::from(area.width).saturating_sub(reserve);
    let (name_w, pane, since) = columns(text_w);
    for (i, row) in rows.iter().skip(state.scroll).take(layout.visible_rows).enumerate() {
        let y = area.y + i as u16;
        let selected = state.selected == Some(row.pane_id);
        let style = if selected { bg.add_modifier(Modifier::REVERSED) } else { bg };
        fill(buf, clip, Rect::new(area.x, y, u16::try_from(text_w).unwrap_or(u16::MAX), 1), style);
        let marker = if row.is_active_pane { g.current } else { " " };
        put(buf, clip, area.x, y, marker, style);
        // The dot keeps its state colour; on the reverse-video bar it sits on the bar colour.
        let mut dot_style = Style::default().fg(dot_color(row.state, t));
        dot_style = dot_style.bg(if selected { t.text_primary } else { palette.window_frame_bg });
        put(buf, clip, area.x + 1, y, &g.dot(row.state, tick), dot_style);
        let name = sanitize_display(&row.name, name_w.max(1));
        let mut line = format!(" {} {}", fit_pad(row.state.as_str(), 7), fit_pad(&name, name_w));
        if row.unread_mail > 0 {
            line.push_str(&format!(" ✉{}", row.unread_mail));
        }
        if pane {
            line.push(' ');
            line.push_str(&fit_pad(&format!("#{}", row.pane_id), PANE_COL_W - 1));
        }
        if since {
            line.push(' ');
            line.push_str(&format!("{:>w$}", since_text(row.since_ms), w = SINCE_COL_W - 1));
        }
        put(buf, clip, area.x + 2, y, &line, style);
    }
    if rows.len() > layout.visible_rows && reserve > 0 {
        render_scrollbar(layout, rows.len(), state.scroll, t, palette, cfg, clip, buf);
    }
}

/// Scroll indicator in the rightmost column of the rows area: arrows at the ends, a thumb whose
/// position follows the first visible row.
#[allow(clippy::too_many_arguments)]
fn render_scrollbar(
    layout: &AgentsListRects,
    total: usize,
    scroll: usize,
    t: &DesignTokens,
    palette: &ThemePalette,
    cfg: &BiltermConfig,
    clip: Rect,
    buf: &mut Buffer,
) {
    let area = layout.rows_area;
    let vis = layout.visible_rows;
    let x = area.x + area.width - 1;
    let ascii = !NavigatorGlyphs::for_pack(cfg.glyphs).spinner;
    let (up, down, track, thumb) = if ascii { ("^", "v", "|", "#") } else { ("▲", "▼", "│", "█") };
    let style = Style::default().fg(t.text_muted).bg(palette.window_frame_bg);
    let max_scroll = total.saturating_sub(vis).max(1);
    let travel = vis.saturating_sub(1);
    let thumb_at = (scroll.min(max_scroll) * travel + max_scroll / 2) / max_scroll;
    for i in 0..vis {
        let sym = if i == 0 && scroll > 0 {
            up
        } else if i + 1 == vis && scroll + vis < total {
            down
        } else if i == thumb_at {
            thumb
        } else {
            track
        };
        put(buf, clip, x, area.y + i as u16, sym, style);
    }
}

fn render_footer(
    wm: &WindowManager,
    layout: &AgentsListRects,
    shown: usize,
    t: &DesignTokens,
    bg: Style,
    clip: Rect,
    buf: &mut Buffer,
) {
    let f = layout.footer;
    if f.width == 0 || f.height == 0 {
        return;
    }
    let s = wm.agents_list();
    let mut end = f.x;
    for (chip, r) in &layout.chips {
        let label = crate::wm::agents_list_geometry::LIST_CHIP_LABELS
            .iter()
            .find(|(c, _)| c == chip)
            .map_or("", |(_, l)| *l);
        let on = match chip {
            ListChip::Filter => s.filter == ListFilter::Agents,
            ListChip::Dock => s.docked,
            ListChip::Zoom => false,
        };
        let style = bg.fg(t.accent).add_modifier(if on { Modifier::BOLD | Modifier::UNDERLINED } else { Modifier::BOLD });
        put(buf, clip, r.x, r.y, label, style);
        end = r.x + r.width;
    }
    let total = wm.agent_statuses().len();
    let count = format!("{shown}/{total}");
    let w = u16::try_from(count.width()).unwrap_or(u16::MAX);
    let x = (f.x + f.width).saturating_sub(w);
    if f.width >= w && (layout.chips.is_empty() || x > end) {
        put(buf, clip, x, f.y, &count, bg.fg(t.text_muted));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_degrade_with_width() {
        assert_eq!(columns(34), (11, true, true));
        // 22 cells: SINCE dropped first, then PANE.
        assert_eq!(columns(24), (7, true, false));
        assert_eq!(columns(18), (7, false, false));
    }

    #[test]
    fn since_text_units() {
        assert_eq!(since_text(0), "0s");
        assert_eq!(since_text(61_000), "1m");
        assert_eq!(since_text(7_300_000), "2h");
    }
}
