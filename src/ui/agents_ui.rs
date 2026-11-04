//! AGENT TRAFFIC floating window renderer (Phase 50-07, D-07): the node graph on the left and the
//! newest bus events (one line each, newest at the bottom) on the right.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
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
use crate::wm::traffic::TrafficEvent;

/// Window title.
pub const AGENTS_WINDOW_TITLE: &str = "AGENT TRAFFIC";
/// Maximum characters of one rendered event line before display-width truncation.
const EVENT_LINE_CAP: usize = 160;

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

/// One-line rendering of a bus event: `#<seq> <from> → <to> <kind>: <preview>`.
pub fn event_line(e: &TrafficEvent) -> String {
    let line = format!(
        "#{} {} → {} {}: {}",
        e.seq,
        e.from.label(),
        e.to.label(),
        e.kind.as_str(),
        e.preview
    );
    sanitize_display(&line, EVENT_LINE_CAP)
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
    let (inner, graph, div_x, events) = (rects.inner, rects.graph, rects.splitter_col, rects.snoop);
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
    render_events(wm, events, palette, buf);
}

fn render_events(wm: &WindowManager, area: Rect, palette: &ThemePalette, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let tokens = DesignTokens::from_palette(palette);
    let rows = usize::from(area.height);
    let all: Vec<&TrafficEvent> = wm.traffic().iter().collect();
    let shown = &all[all.len().saturating_sub(rows)..];
    for (i, e) in shown.iter().enumerate() {
        let line = fit_cols(&event_line(e), usize::from(area.width));
        let fg: Color = if e.redacted { tokens.text_muted } else { tokens.text_primary };
        buf.set_stringn(
            area.x,
            area.y + i as u16,
            &line,
            usize::from(area.width),
            Style::default().fg(fg).bg(palette.window_frame_bg),
        );
    }
}
