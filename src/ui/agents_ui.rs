//! AGENT TRAFFIC floating window renderer (Phase 50-07, D-07): the node graph on the left and the
//! conversation (snoop) pane on the right: bus events, one line each, newest at the bottom.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthChar;

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::DesignTokens;
use crate::ui::glyphs::GlyphSet;
use crate::ui::graph_raster::render_graph_tui;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::vt::sanitize_display;
use crate::wm::WindowManager;
use crate::wm::agents_geometry::AgentsPaneRects;
use crate::wm::agents_window::{SnoopLine, WindowPane, snoop_view_range};

/// Window title.
pub const AGENTS_WINDOW_TITLE: &str = "AGENT TRAFFIC";
/// Maximum characters of one rendered conversation line before display-width truncation.
const EVENT_LINE_CAP: usize = 160;
/// The `[copy]` badge in the conversation header.
pub const COPY_BADGE: &str = "[copy]";

/// Column of the pin badge in the title row of a window with outer rect `outer`.
///
/// It sits two cells left of the leftmost title badge (`[_][□][X]`, 9 cells wide in every glyph
/// pack, right edge inset by 2). `None` when the title is too narrow to hold the badge next to the
/// `AGENT TRAFFIC` title; the renderer and the mouse hit-test both use this one function.
pub fn agents_pin_badge_x(outer: Rect) -> Option<u16> {
    const BADGES_W: u16 = 9;
    if outer.width < 24 {
        return None;
    }
    let x = (outer.x + outer.width).saturating_sub(BADGES_W + 2 + 2);
    let title_end = outer.x + 1 + AGENTS_WINDOW_TITLE.len() as u16 + 2;
    (x > title_end).then_some(x)
}

/// Pin badge glyph: `◆` pinned / `◇` unpinned, `*` / `o` in the ASCII pack.
pub fn pin_badge_glyph(glyphs: &GlyphSet, pinned: bool) -> &'static str {
    match (glyphs.pin_top_bottom.is_ascii(), pinned) {
        (true, true) => "*",
        (true, false) => "o",
        (false, true) => "◆",
        (false, false) => "◇",
    }
}

/// Splits the inner rect into `(graph, divider, events)` at `splitter_pct`.
pub fn split_panes(inner: Rect, splitter_pct: u16) -> (Rect, u16, Rect) {
    let pct = splitter_pct.clamp(10, 90);
    let graph_w = (u32::from(inner.width) * u32::from(pct) / 100) as u16;
    let graph_w = graph_w.clamp(1.min(inner.width), inner.width.saturating_sub(2));
    let graph = Rect::new(inner.x, inner.y, graph_w, inner.height);
    let div_x = inner.x + graph_w;
    let events_x = div_x + 1;
    let events_w = (inner.x + inner.width).saturating_sub(events_x);
    (graph, div_x, Rect::new(events_x, inner.y, events_w, inner.height))
}

/// Compact age: `--` (unknown), `7s`, `3m`, `2h`.
fn age_text(age: Option<std::time::Duration>) -> String {
    match age.map(|d| d.as_secs()) {
        None => "--".to_string(),
        Some(s) if s < 60 => format!("{s}s"),
        Some(s) if s < 3600 => format!("{}m", s / 60),
        Some(s) => format!("{}h", s / 3600),
    }
}

/// One-line rendering of a conversation line: `<age> <from>→<to> <kind>: <preview>`.
pub fn snoop_line_text(l: &SnoopLine, ascii: bool) -> String {
    let arrow = if ascii { "->" } else { "→" };
    let line = format!(
        "{:>3} {}{arrow}{} {}: {}",
        age_text(l.age),
        l.from_label,
        l.to_label,
        l.kind.as_str(),
        l.preview
    );
    sanitize_display(&line, EVENT_LINE_CAP)
}

/// Where the parts of the conversation pane sit. The renderer draws from it and the mouse handler
/// hit-tests against it, so the two can never disagree.
#[derive(Clone, Debug, Default)]
pub struct SnoopLayout {
    /// `[copy]` badge (zero-sized when the pane is too small for a header).
    pub copy: Rect,
    /// Area holding the conversation lines.
    pub lines_area: Rect,
    /// Visible lines: `(row, bus sequence number)`, oldest first.
    pub lines: Vec<(u16, u64)>,
}

/// Rows of the conversation pane: header row, then lines.
fn snoop_header_rows(area: Rect) -> u16 {
    u16::from(area.height >= 3)
}

/// Area of the conversation pane that holds lines.
pub fn snoop_lines_area(_wm: &WindowManager, area: Rect) -> Rect {
    let head = snoop_header_rows(area);
    Rect::new(area.x, area.y + head, area.width, area.height - head)
}

/// Layout of the conversation pane `rects.snoop`.
pub fn snoop_layout(wm: &WindowManager, rects: &AgentsPaneRects) -> SnoopLayout {
    let area = rects.snoop;
    let lines_area = snoop_lines_area(wm, area);
    let st = &wm.agents_window().snoop;
    let all = wm.snoop_lines();
    let (start, end) = snoop_view_range(all.len(), usize::from(lines_area.height), st.follow, st.scroll);
    let lines = (start..end)
        .enumerate()
        .map(|(i, idx)| (lines_area.y + i as u16, all[idx].seq))
        .collect();
    let badge_w = COPY_BADGE.len() as u16;
    let copy = if snoop_header_rows(area) == 1 && area.width >= badge_w + 14 {
        Rect::new(area.x + area.width - badge_w, area.y, badge_w, 1)
    } else {
        Rect::default()
    };
    SnoopLayout { copy, lines_area, lines }
}

/// Truncates `s` to at most `max_cols` terminal columns.
fn fit_cols(s: &str, max_cols: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > max_cols {
            break;
        }
        used += w;
        out.push(ch);
    }
    out
}

/// Renders the AGENT TRAFFIC window when open (above the drawer, below the `:mcp` console).
pub fn render_agents_window(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some(rect) = wm.agents_window_rect(workspace_area) else {
        return;
    };
    let rect = rect.intersection(buf.area);
    if rect.width < 8 || rect.height < 4 {
        return;
    }
    let state = wm.agents_window();
    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, workspace_area, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, workspace_area, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);
    let bg = Style::default()
        .fg(DesignTokens::from_palette(palette).text_primary)
        .bg(palette.window_frame_bg);
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ");
                cell.set_style(bg);
            }
        }
    }
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }
    render_borland_frame_styled(
        rect,
        AGENTS_WINDOW_TITLE,
        state.captures_input(),
        cfg.window_outline,
        palette,
        glyphs,
        buf,
    );

    // Window geometry comes from the same helper the mouse hit-tests use.
    let Some(rects) = wm.agents_pane_rects(workspace_area) else {
        return;
    };
    if let Some(x) = agents_pin_badge_x(rects.outer)
        && let Some(cell) = buf.cell_mut((x, rects.outer.y))
    {
        let fg = if state.pinned { palette.title_fg } else { palette.controls_fg.unwrap_or(palette.title_fg) };
        cell.set_symbol(pin_badge_glyph(glyphs, state.pinned));
        cell.set_style(Style::default().fg(fg).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD));
    }
    let (inner, graph, div_x) = (rects.inner, rects.graph, rects.splitter_col);
    let divider = Style::default()
        .fg(palette.inactive_border_fg)
        .bg(palette.window_frame_bg);
    for y in inner.y..inner.y + inner.height {
        if let Some(cell) = buf.cell_mut((div_x, y)) {
            cell.set_symbol("│");
            cell.set_style(divider);
        }
    }

    render_graph_tui(&wm.agents_graph_scene(), graph, buf, palette);
    render_snoop(wm, &rects, palette, glyphs, buf);
}

fn render_snoop(wm: &WindowManager, rects: &AgentsPaneRects, palette: &ThemePalette, glyphs: &GlyphSet, buf: &mut Buffer) {
    let area = rects.snoop;
    if area.width == 0 || area.height == 0 {
        return;
    }
    let tokens = DesignTokens::from_palette(palette);
    let ascii = glyphs.pin_top_bottom.is_ascii();
    let bg = palette.window_frame_bg;
    let width = usize::from(area.width);
    let state = wm.agents_window();
    let lay = snoop_layout(wm, rects);

    // Focus markers: the conversation header and the top of the divider column.
    let marker = |on: bool, glyph: &'static str, plain: &'static str| if on { if ascii { plain } else { glyph } } else { " " };
    let focus = state.pane_focus;
    if focus == WindowPane::Graph
        && let Some(cell) = buf.cell_mut((rects.splitter_col, rects.inner.y))
    {
        cell.set_symbol(marker(true, "◂", "<"));
    }
    if snoop_header_rows(area) == 1 {
        let head = format!("{} conversation", marker(focus == WindowPane::Snoop, "▸", ">"));
        let style = Style::default().fg(tokens.accent).bg(bg).add_modifier(Modifier::BOLD);
        buf.set_stringn(area.x, area.y, &head, width, style);
        if lay.copy.width > 0 {
            buf.set_stringn(lay.copy.x, lay.copy.y, COPY_BADGE, usize::from(lay.copy.width), Style::default().fg(tokens.accent).bg(bg));
        }
    }

    let all = wm.snoop_lines();
    let selected = wm.snoop_selected_seq();
    for (y, seq) in &lay.lines {
        let Some(l) = all.iter().find(|l| l.seq == *seq) else { continue };
        let text = fit_cols(&snoop_line_text(l, ascii), usize::from(lay.lines_area.width));
        let is_sel = selected == Some(*seq) && focus == WindowPane::Snoop;
        let style = if is_sel {
            Style::default().fg(tokens.accent_on).bg(tokens.accent)
        } else {
            let fg: Color = if l.redacted { tokens.text_muted } else { tokens.text_primary };
            Style::default().fg(fg).bg(bg)
        };
        if is_sel {
            for x in lay.lines_area.x..lay.lines_area.x + lay.lines_area.width {
                if let Some(cell) = buf.cell_mut((x, *y)) {
                    cell.set_style(style);
                }
            }
        }
        buf.set_stringn(lay.lines_area.x, *y, &text, usize::from(lay.lines_area.width), style);
    }
}
