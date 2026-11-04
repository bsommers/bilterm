//! AGENT TRAFFIC floating window renderer (Phase 50-07, D-07): the node graph on the left and the
//! conversation (snoop) pane on the right: bus events, one line each, newest at the bottom.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
use crate::wm::agents_cmd::SnoopFilterKind;
use crate::wm::agents_window::{ComposeGate, SnoopLine, WindowPane, key_label, snoop_view_range};

/// Window title.
pub const AGENTS_WINDOW_TITLE: &str = "AGENT TRAFFIC";
/// Maximum characters of one rendered conversation line before display-width truncation.
const EVENT_LINE_CAP: usize = 160;
/// The `[copy]` badge in the conversation header.
pub const COPY_BADGE: &str = "[copy]";
/// The `[send]` badge of the compose line.
pub const SEND_BADGE: &str = "[send]";

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

/// Header buttons of the conversation pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnoopButton {
    Agent,
    Pair,
    Keyword,
    Clear,
}

impl SnoopButton {
    /// Button text.
    pub fn label(self) -> &'static str {
        match self {
            Self::Agent => "[agent]",
            Self::Pair => "[pair]",
            Self::Keyword => "[/]",
            Self::Clear => "[clear]",
        }
    }
}

/// Where the parts of the conversation pane sit. The renderer draws from it and the mouse handler
/// hit-tests against it, so the two can never disagree. Absent parts have a zero-sized rect.
#[derive(Clone, Debug, Default)]
pub struct SnoopLayout {
    /// Filter and clear buttons.
    pub header_buttons: Vec<(Rect, SnoopButton)>,
    /// Active filter chips; the last cell of each rect is its `×`.
    pub chips: Vec<(Rect, SnoopFilterKind)>,
    /// Follow marker (click toggles follow).
    pub follow: Rect,
    /// `[copy]` badge.
    pub copy: Rect,
    /// Keyword entry line (while open).
    pub entry: Option<Rect>,
    /// Area holding the conversation lines.
    pub lines_area: Rect,
    /// Visible lines: `(row, bus sequence number)`, oldest first.
    pub lines: Vec<(u16, u64)>,
    /// The `send as me` compose line (whole row).
    pub compose: Rect,
    /// `[send]` badge inside the compose line.
    pub send: Rect,
}

/// Header rows of the conversation pane above the lines.
struct SnoopRows {
    title: bool,
    buttons: bool,
    /// Chip row, or the keyword entry line while it is open.
    extra: bool,
    /// Compose line at the bottom.
    compose: bool,
}

impl SnoopRows {
    fn header(&self) -> u16 {
        u16::from(self.title) + u16::from(self.buttons) + u16::from(self.extra)
    }
}

fn snoop_rows(wm: &WindowManager, area: Rect) -> SnoopRows {
    let st = &wm.agents_window().snoop;
    let has_extra = st.keyword_entry.is_some()
        || st.filter_agent.is_some()
        || st.filter_pair.is_some()
        || st.filter_text.is_some();
    SnoopRows {
        title: area.height >= 3,
        buttons: area.height >= 5,
        extra: has_extra && area.height >= 7,
        compose: area.height >= 4,
    }
}

/// Area of the conversation pane that holds lines.
pub fn snoop_lines_area(wm: &WindowManager, area: Rect) -> Rect {
    let rows = snoop_rows(wm, area);
    let (head, foot) = (rows.header(), u16::from(rows.compose));
    Rect::new(area.x, area.y + head, area.width, area.height - head - foot)
}

/// Text of the chip for one active filter (without the `×`).
fn chip_text(kind: SnoopFilterKind, wm: &WindowManager, ascii: bool) -> Option<String> {
    let st = &wm.agents_window().snoop;
    let cap = |s: &str| sanitize_display(s, 14);
    Some(match kind {
        SnoopFilterKind::Agent => format!("agent:{}", cap(&key_label(st.filter_agent.as_ref()?))),
        SnoopFilterKind::Pair => {
            let (a, b) = st.filter_pair.as_ref()?;
            format!("pair:{}{}{}", cap(&key_label(a)), if ascii { "-" } else { "↔" }, cap(&key_label(b)))
        }
        SnoopFilterKind::Text => format!("text:{}", cap(st.filter_text.as_ref()?)),
    })
}

/// Layout of the conversation pane `rects.snoop`. Chip and follow glyphs differ between glyph
/// packs but are one cell wide in each, so the layout does not depend on the pack.
pub fn snoop_layout(wm: &WindowManager, rects: &AgentsPaneRects) -> SnoopLayout {
    let area = rects.snoop;
    let rows = snoop_rows(wm, area);
    let lines_area = snoop_lines_area(wm, area);
    let st = &wm.agents_window().snoop;
    let all = wm.snoop_lines();
    let (start, end) = snoop_view_range(all.len(), usize::from(lines_area.height), st.follow, st.scroll);
    let lines = (start..end)
        .enumerate()
        .map(|(i, idx)| (lines_area.y + i as u16, all[idx].seq))
        .collect();
    let mut lay = SnoopLayout { lines_area, lines, ..SnoopLayout::default() };
    let right = area.x + area.width;

    if rows.title {
        let badge_w = COPY_BADGE.len() as u16;
        if area.width >= badge_w + 14 {
            lay.copy = Rect::new(right - badge_w, area.y, badge_w, 1);
            if area.width >= badge_w + 14 + 1 + FOLLOW_W {
                lay.follow = Rect::new(lay.copy.x - 1 - FOLLOW_W, area.y, FOLLOW_W, 1);
            }
        }
    }
    if rows.compose {
        lay.compose = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
        let w = SEND_BADGE.len() as u16;
        if area.width >= w + 12 {
            lay.send = Rect::new(right - w, lay.compose.y, w, 1);
        }
    }
    let mut next_y = area.y + u16::from(rows.title);
    if rows.buttons {
        let mut x = area.x;
        for b in [SnoopButton::Agent, SnoopButton::Pair, SnoopButton::Keyword, SnoopButton::Clear] {
            let w = b.label().len() as u16;
            if x + w > right {
                break;
            }
            lay.header_buttons.push((Rect::new(x, next_y, w, 1), b));
            x += w + 1;
        }
        next_y += 1;
    }
    if rows.extra {
        if st.keyword_entry.is_some() {
            lay.entry = Some(Rect::new(area.x, next_y, area.width, 1));
        } else {
            let mut x = area.x;
            for kind in [SnoopFilterKind::Agent, SnoopFilterKind::Pair, SnoopFilterKind::Text] {
                let Some(text) = chip_text(kind, wm, false) else { continue };
                let w = (UnicodeWidthStr::width(text.as_str()) + 2) as u16;
                if x + w > right {
                    break;
                }
                lay.chips.push((Rect::new(x, next_y, w, 1), kind));
                x += w + 1;
            }
        }
    }
    lay
}

/// Cells of the follow marker (`▼ follow`).
const FOLLOW_W: u16 = 8;

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
    let accent = Style::default().fg(tokens.accent).bg(bg);
    if snoop_rows(wm, area).title {
        let head = format!("{} conversation", marker(focus == WindowPane::Snoop, "▸", ">"));
        buf.set_stringn(area.x, area.y, &head, width, accent.add_modifier(Modifier::BOLD));
        if lay.copy.width > 0 {
            buf.set_stringn(lay.copy.x, lay.copy.y, COPY_BADGE, usize::from(lay.copy.width), accent);
        }
        if lay.follow.width > 0 {
            let (on, off) = if ascii { ("v follow", "- follow") } else { ("▼ follow", "▽ follow") };
            let (text, style) = if state.snoop.follow {
                (on, accent.add_modifier(Modifier::BOLD))
            } else {
                (off, Style::default().fg(tokens.text_muted).bg(bg))
            };
            buf.set_stringn(lay.follow.x, lay.follow.y, text, usize::from(lay.follow.width), style);
        }
    }
    for (r, b) in &lay.header_buttons {
        buf.set_stringn(r.x, r.y, b.label(), usize::from(r.width), accent);
    }
    for (r, kind) in &lay.chips {
        let close = if ascii { "x" } else { "×" };
        let text = format!("{} {close}", chip_text(*kind, wm, ascii).unwrap_or_default());
        buf.set_stringn(r.x, r.y, &text, usize::from(r.width), Style::default().fg(tokens.text_primary).bg(bg));
    }
    if lay.compose.width > 0 {
        let c = lay.compose;
        let focused = focus == WindowPane::Compose;
        let (gate, note) = match wm.compose_gate() {
            ComposeGate::Unavailable(why) => ("disabled", why),
        };
        let sep = if ascii { ">" } else { "▸" };
        let lead = match (focused, ascii) {
            (false, _) => "  ",
            (true, true) => "> ",
            (true, false) => "▸ ",
        };
        let text = format!("{lead}send as me {sep} {gate}: {note}");
        let room = usize::from(c.width.saturating_sub(lay.send.width + u16::from(lay.send.width > 0)));
        let mut style = Style::default().fg(tokens.text_muted).bg(bg);
        if focused {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        buf.set_stringn(c.x, c.y, fit_cols(&text, room), room, style);
        if lay.send.width > 0 {
            buf.set_stringn(lay.send.x, lay.send.y, SEND_BADGE, usize::from(lay.send.width), Style::default().fg(tokens.text_muted).bg(bg));
        }
    }
    if let (Some(r), Some(entry)) = (lay.entry, state.snoop.keyword_entry.as_ref()) {
        let cursor = if ascii { "_" } else { "▏" };
        let text = format!("/ {entry}{cursor}");
        buf.set_stringn(r.x, r.y, fit_cols(&text, usize::from(r.width)), usize::from(r.width), Style::default().fg(tokens.text_primary).bg(bg));
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
