//! Find window renderer and geometry (Phase 50.1): frame, tabs, query line, result rows, footer.
//!
//! 50.1-01 owns the frame and the geometry (`find_layout`); 50.1-07 draws rows, tabs and chips into
//! the rects `find_layout` returns, 50.1-08 hit-tests the same rects.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::design::{AppearanceProfile, ColorRole, DesignTokens};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
use crate::wm::find_cmd::FindKind;
use crate::wm::registry::types::{FindResults, FindRow, RowData, Transport};
use crate::wm::{WindowManager, inset_window_frame};

/// Window title (the frame draws it between the borders).
pub const FIND_TITLE: &str = " FIND ";

/// Minimum workspace area the modal needs to be drawn at all.
pub const FIND_MIN_AREA: (u16, u16) = (40, 8);

fn render_frame(
    rect: Rect,
    bounds: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    if cfg.liquid_glass {
        render_liquid_ambient_halo(buf, rect, bounds, palette);
    } else if cfg.drop_shadows {
        render_drop_shadow_styled(buf, rect, bounds, cfg.shadow_style, palette);
    }
    occlude_rect(buf, rect);
    if cfg.is_glass() {
        apply_frosted_glass_surface(buf, rect, palette.window_frame_bg, palette);
    }
    render_borland_frame_styled(rect, FIND_TITLE, true, cfg.window_outline, palette, glyphs, buf);
}

/// Tab chip labels, indexed by `FindKind::index` (`Agent`, `Session`, `History`).
pub const TAB_LABELS: [&str; 3] = ["[Agents]", "[Sessions]", "[History]"];
/// The `[clear]` chip at the right end of the query row.
pub const CLEAR_LABEL: &str = "[clear]";
/// Footer chips, left to right: jump, kind, prefill, close.
pub const FOOTER_LABELS: [&str; 4] = ["[Enter jump]", "[Tab kind]", "[^Y prefill]", "[Esc close]"];

/// Geometry of the Find window's inner area; the single source for render and hit-testing.
///
/// Row 0 holds the kind tabs, row 1 the query line with a right-aligned `[clear]` chip, the last
/// row the footer chips, the row above it the source line of the selected result, and the list
/// takes the rest. Rects that do not fit are empty (zero width or height) and never overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FindLayout {
    pub tabs: Rect,
    pub tab_chips: [Rect; 3],
    pub query: Rect,
    pub clear_chip: Rect,
    pub list: Rect,
    pub source_line: Rect,
    pub footer: Rect,
    pub footer_chips: [Rect; 4],
}

/// A one-row rect at `(x, y)` of up to `want` cells, clipped to `bounds` (empty when it does not fit).
fn clip_chip(x: u16, y: u16, want: u16, bounds: Rect) -> Rect {
    let right = bounds.x.saturating_add(bounds.width);
    let x = x.min(right);
    let w = want.min(right - x);
    if bounds.height == 0 {
        return Rect::new(bounds.x, bounds.y, 0, 0);
    }
    Rect::new(x, y, w, 1)
}

/// Lays chips with the given label widths left to right, one blank cell apart, inside `row`.
fn chips_in_row<const N: usize>(row: Rect, labels: [&str; N]) -> [Rect; N] {
    let mut x = row.x;
    labels.map(|label| {
        let want = label.chars().count() as u16;
        let chip = clip_chip(x, row.y, want, row);
        x = x.saturating_add(want).saturating_add(1);
        chip
    })
}

/// Pure geometry of the Find window for the inner (frame-less) rect `inner`.
pub fn find_layout(inner: Rect, _cfg: &BiltermConfig) -> FindLayout {
    let bottom = inner.y.saturating_add(inner.height);
    let mut top_y = inner.y;
    let mut bottom_y = bottom;
    let take_top = |top_y: &mut u16| -> Rect {
        let h = u16::from(*top_y < bottom_y);
        let r = Rect::new(inner.x, *top_y, inner.width, h);
        *top_y += h;
        r
    };
    let tabs = take_top(&mut top_y);
    let query_row = take_top(&mut top_y);
    let take_bottom = |bottom_y: &mut u16| -> Rect {
        let h = u16::from(*bottom_y > top_y);
        *bottom_y -= h;
        Rect::new(inner.x, *bottom_y, inner.width, h)
    };
    let footer = take_bottom(&mut bottom_y);
    let source_line = take_bottom(&mut bottom_y);
    let list = Rect::new(inner.x, top_y, inner.width, bottom_y - top_y);

    let clear_w = CLEAR_LABEL.chars().count() as u16;
    let (query, clear_chip) = if query_row.height == 1 && query_row.width > clear_w + 1 {
        let q_w = query_row.width - clear_w - 1;
        (
            Rect::new(query_row.x, query_row.y, q_w, 1),
            Rect::new(query_row.x + query_row.width - clear_w, query_row.y, clear_w, 1),
        )
    } else {
        (query_row, Rect::new(query_row.x + query_row.width, query_row.y, 0, 0))
    };
    FindLayout {
        tabs,
        tab_chips: chips_in_row(tabs, TAB_LABELS),
        query,
        clear_chip,
        list,
        source_line,
        footer,
        footer_chips: chips_in_row(footer, FOOTER_LABELS),
    }
}

/// Draws the Find window when `Modal::Find` is open.
pub(super) fn render_find_modal(
    wm: &WindowManager,
    workspace_area: Rect,
    cfg: &BiltermConfig,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    let Some((kind, query, selected)) = wm.find_modal_state() else {
        return;
    };
    let Some(outer) = wm.active_modal_rect(workspace_area) else {
        return;
    };
    render_frame(outer, workspace_area, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(outer);
    if is_empty(inner) {
        return;
    }
    let st = FindStyles::new(cfg, palette, glyphs);
    let layout = find_layout(inner, cfg);
    let results = wm.find_results();
    buf.set_style(inner, st.base);
    render_tabs(buf, &layout, kind, results, &st);
    render_query(buf, &layout, kind, query, &st);
    render_rows(buf, layout.list, kind, results, selected, &st);
    render_footer(buf, &layout, results, selected, &st);
}

/// Colours and glyph choices shared by the Find window parts.
struct FindStyles {
    base: Style,
    dim: Style,
    accent: Style,
    chip: Style,
    chip_active: Style,
    selected: Style,
    ascii: bool,
    modern: bool,
}

impl FindStyles {
    fn new(cfg: &BiltermConfig, palette: &ThemePalette, glyphs: &GlyphSet) -> Self {
        let bg = palette.window_frame_bg;
        let fg = palette.text_fg.unwrap_or(Color::White);
        let modern = AppearanceProfile::resolve(cfg) != AppearanceProfile::Legacy;
        let selected = if modern {
            let overlay = DesignTokens::from_palette(palette).get(ColorRole::Overlay);
            Style::default().fg(palette.title_fg).bg(overlay).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Black).bg(palette.active_border_fg)
        };
        Self {
            base: Style::default().fg(fg).bg(bg),
            dim: Style::default().fg(palette.inactive_border_fg).bg(bg),
            accent: Style::default().fg(palette.prompt_prefix_fg).bg(bg).add_modifier(Modifier::BOLD),
            chip: Style::default().fg(palette.prompt_prefix_fg).bg(bg),
            chip_active: Style::default()
                .fg(palette.prompt_prefix_fg)
                .bg(bg)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
            selected,
            ascii: glyphs.oia_separator.is_ascii(),
            modern,
        }
    }

    fn ellipsis(&self) -> char {
        if self.ascii { '~' } else { '…' }
    }

    fn marker(&self, selected: bool) -> &'static str {
        match (selected, self.ascii) {
            (false, _) => " ",
            (true, true) => ">",
            (true, false) => "▎",
        }
    }
}

/// Replaces control and bidi-override characters with `?` (T-50.1-14); one char in, one char out.
fn clean(text: &str) -> String {
    text.chars()
        .map(|c| {
            let bidi = matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}');
            if c.is_control() || bidi { '?' } else { c }
        })
        .collect()
}

/// `text` cut to at most `max` display cells, ending in `ellipsis` when something was dropped.
fn truncate(text: &str, max: usize, ellipsis: char) -> String {
    if UnicodeWidthStr::width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let budget = max - 1;
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w > budget {
            break;
        }
        used += w;
        out.push(c);
    }
    out.push(ellipsis);
    out
}

/// Draws `text` from `(x, y)` up to column `limit` (exclusive); returns the next free column.
fn put(buf: &mut Buffer, x: u16, y: u16, limit: u16, text: &str, style: Style) -> u16 {
    let mut x = x;
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0) as u16;
        if w == 0 {
            continue;
        }
        if x.saturating_add(w) > limit {
            break;
        }
        let mut tmp = [0u8; 4];
        buf[(x, y)].set_symbol(c.encode_utf8(&mut tmp)).set_style(style);
        for dx in 1..w {
            buf[(x + dx, y)].set_symbol("").set_style(style);
        }
        x += w;
    }
    x
}

fn rect_end(r: Rect) -> u16 {
    r.x.saturating_add(r.width)
}

fn is_empty(r: Rect) -> bool {
    r.width == 0 || r.height == 0
}

/// Rightmost edge of the chips that were laid out (zero-width chips are ignored).
fn chips_end(chips: &[Rect], fallback: u16) -> u16 {
    chips.iter().filter(|c| !is_empty(**c)).map(|c| rect_end(*c)).max().unwrap_or(fallback)
}

/// Draws `text` right-aligned in `row` when it fits right of `left_end`; reports whether it did.
fn put_right(buf: &mut Buffer, row: Rect, left_end: u16, text: &str, style: Style) -> bool {
    if is_empty(row) || text.is_empty() {
        return false;
    }
    let w = UnicodeWidthStr::width(text) as u16;
    let x = rect_end(row).saturating_sub(w);
    let clear_of_chips = left_end <= row.x || x > left_end;
    if w > row.width || !clear_of_chips {
        return false;
    }
    put(buf, x, row.y, rect_end(row), text, style);
    true
}

fn fmt_age(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m", s / 60),
        3600..=86_399 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86_400),
    }
}

fn host_word(row: &FindRow) -> String {
    row.host.as_deref().map_or_else(|| "local".to_string(), clean)
}

/// Secondary text of a row, per kind (D-U3 row shapes).
fn row_detail(row: &FindRow) -> String {
    let mut parts: Vec<String> = Vec::new();
    let transport_words = |parts: &mut Vec<String>| {
        if row.transport != Transport::Local {
            parts.push(row.transport.as_str().to_string());
        }
        if row.metadata_only {
            parts.push("label-only".to_string());
        }
    };
    match &row.data {
        RowData::Agent(a) => {
            parts.push(format!("#{}", a.pane));
            parts.push(a.state.as_str().to_string());
            parts.push(host_word(row));
            parts.push(fmt_age(a.since_ms));
        }
        RowData::Session(s) => {
            parts.push(format!("{} panes", s.pane_count));
            transport_words(&mut parts);
        }
        RowData::Pane(p) => {
            parts.push(format!("#{}", p.pane));
            transport_words(&mut parts);
            if let Some(host) = row.host.as_deref() {
                parts.push(clean(host));
            }
        }
        RowData::History(h) => {
            if let Some(pane) = h.pane {
                parts.push(format!("#{pane}"));
            }
            parts.push(host_word(row));
            if let Some(code) = h.exit_code {
                parts.push(format!("exit {code}"));
            }
        }
    }
    if let Some(also) = row.also_in.first() {
        parts.push(format!("(also {})", also.label()));
    }
    clean(&parts.join(" "))
}

/// Draws one result row into the one-line `rect`: marker, label with matches emphasised, dimmer
/// detail, and the right-aligned source tag (never truncated before the label, D-U3).
fn draw_row(buf: &mut Buffer, rect: Rect, row: &FindRow, selected: bool, st: &FindStyles) {
    let base = if selected { st.selected } else { st.base };
    buf.set_style(rect, base);
    let end = rect_end(rect);
    put(buf, rect.x, rect.y, end, st.marker(selected), if selected { st.selected } else { st.accent });
    let width = usize::from(rect.width);
    let tag = truncate(&format!("[{}]", clean(&row.source.label())), width.saturating_sub(4).max(1), st.ellipsis());
    let tag_w = UnicodeWidthStr::width(tag.as_str());
    let body_w = width.saturating_sub(2 + tag_w + 1);
    let label_t = truncate(&clean(&row.label), body_w, st.ellipsis());
    let label_w = UnicodeWidthStr::width(label_t.as_str());
    let detail_room = body_w.saturating_sub(label_w + 2);
    let detail = if detail_room >= 2 { truncate(&row_detail(row), detail_room, st.ellipsis()) } else { String::new() };

    let hit_style = base.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let mut x = rect.x.saturating_add(2);
    for (i, c) in label_t.chars().enumerate() {
        let style = if row.matched.contains(&(i as u32)) { hit_style } else { base };
        let mut tmp = [0u8; 4];
        x = put(buf, x, rect.y, end, c.encode_utf8(&mut tmp), style);
    }
    if !detail.is_empty() {
        put(buf, x.saturating_add(2), rect.y, end, &detail, if selected { base } else { st.dim });
    }
    let tag_x = end.saturating_sub(tag_w as u16);
    put(buf, tag_x, rect.y, end, &tag, if selected { base } else { st.accent });
}

fn kind_noun(kind: FindKind) -> &'static str {
    match kind {
        FindKind::Agent => "agents",
        FindKind::Session => "sessions",
        FindKind::History => "history",
    }
}

fn render_rows(buf: &mut Buffer, list: Rect, kind: FindKind, results: &FindResults, selected: usize, st: &FindStyles) {
    if is_empty(list) {
        return;
    }
    if results.rows.is_empty() {
        let mut msg = format!("No {} match", kind_noun(kind));
        if results.masked > 0 {
            msg.push_str(&format!(", {} masked", results.masked));
        }
        put(buf, list.x, list.y, rect_end(list), &msg, st.dim);
        return;
    }
    let rows = usize::from(list.height);
    let sel = selected.min(results.rows.len() - 1);
    let start = sel.saturating_sub(rows - 1);
    for (i, row) in results.rows.iter().enumerate().skip(start).take(rows) {
        let rect = Rect::new(list.x, list.y + (i - start) as u16, list.width, 1);
        draw_row(buf, rect, row, i == sel, st);
    }
}

fn render_tabs(buf: &mut Buffer, layout: &FindLayout, kind: FindKind, results: &FindResults, st: &FindStyles) {
    for (i, chip) in layout.tab_chips.iter().enumerate() {
        if !is_empty(*chip) {
            let style = if i == kind.index() { st.chip_active } else { st.chip };
            put(buf, chip.x, chip.y, rect_end(*chip), TAB_LABELS[i], style);
        }
    }
    let left_end = chips_end(&layout.tab_chips, layout.tabs.x);
    put_right(buf, layout.tabs, left_end, &format!("{} results", results.total), st.dim);
}

fn render_query(buf: &mut Buffer, layout: &FindLayout, kind: FindKind, query: &str, st: &FindStyles) {
    if is_empty(layout.query) {
        return;
    }
    let shown = clean(query);
    let text = if st.modern && !st.ascii {
        format!("❯ {shown}▍")
    } else {
        format!("Find {} > {shown}_", kind.as_str())
    };
    // Keep the tail (the cursor end) visible when the query is longer than the row.
    let room = usize::from(layout.query.width);
    let mut tail: Vec<char> = Vec::new();
    let mut used = 0;
    for c in text.chars().rev() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w > room {
            break;
        }
        used += w;
        tail.push(c);
    }
    let text: String = tail.into_iter().rev().collect();
    put(buf, layout.query.x, layout.query.y, rect_end(layout.query), &text, st.accent);
    if !is_empty(layout.clear_chip) {
        let c = layout.clear_chip;
        put(buf, c.x, c.y, rect_end(c), CLEAR_LABEL, st.chip);
    }
}

/// Source line of the selected row, footer chips, `<i>/<total>` and the masked count (a number only).
fn render_footer(buf: &mut Buffer, layout: &FindLayout, results: &FindResults, selected: usize, st: &FindStyles) {
    let masked = (results.masked > 0).then(|| format!("{} masked", results.masked));
    let sel = selected.min(results.rows.len().saturating_sub(1));
    let pos = (!results.rows.is_empty()).then(|| format!("{}/{}", sel + 1, results.total.max(results.rows.len())));
    let mut masked_placed = false;
    if !is_empty(layout.footer) {
        for (i, chip) in layout.footer_chips.iter().enumerate() {
            if !is_empty(*chip) {
                put(buf, chip.x, chip.y, rect_end(*chip), FOOTER_LABELS[i], st.chip);
            }
        }
        let left_end = chips_end(&layout.footer_chips, layout.footer.x);
        let both = pos.iter().chain(masked.iter()).cloned().collect::<Vec<_>>().join("  ");
        if masked.is_some() && put_right(buf, layout.footer, left_end, &both, st.dim) {
            masked_placed = true;
        } else if let Some(m) = &masked {
            masked_placed = put_right(buf, layout.footer, left_end, m, st.dim);
        }
        if masked.is_none()
            && let Some(p) = &pos
        {
            put_right(buf, layout.footer, left_end, p, st.dim);
        }
    }
    let r = layout.source_line;
    if is_empty(r) {
        return;
    }
    // The footer chips can fill a narrow row; the masked count then rides on the source line.
    let mut reserve = 0usize;
    if let Some(m) = masked.as_deref().filter(|_| !masked_placed)
        && put_right(buf, r, r.x, m, st.dim)
    {
        reserve = UnicodeWidthStr::width(m) + 1;
    }
    if let Some(row) = results.rows.get(sel) {
        let mut text = format!("source: {} {}", row.source.origin.as_str(), clean(&row.source.name));
        if let Some(path) = row.source.path.as_deref() {
            text.push(' ');
            text.push_str(&clean(path));
        }
        let room = usize::from(r.width).saturating_sub(reserve);
        put(buf, r.x, r.y, rect_end(r), &truncate(&text, room, st.ellipsis()), st.dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn non_empty(r: Rect) -> bool {
        r.width > 0 && r.height > 0
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        non_empty(a) && non_empty(b) && a.intersects(b)
    }

    fn check(inner: Rect) {
        let l = find_layout(inner, &BiltermConfig::default());
        let rows = [l.tabs, l.query, l.clear_chip, l.list, l.source_line, l.footer];
        for (i, a) in rows.iter().enumerate() {
            if non_empty(*a) {
                assert_eq!(a.intersection(inner), *a, "{a:?} escapes {inner:?}");
            }
            for b in &rows[i + 1..] {
                assert!(!overlaps(*a, *b), "{a:?} overlaps {b:?} in {inner:?}");
            }
        }
        for (chips, parent) in [(&l.tab_chips[..], l.tabs), (&l.footer_chips[..], l.footer)] {
            for (i, c) in chips.iter().enumerate() {
                if non_empty(*c) {
                    assert_eq!(c.intersection(parent), *c, "chip {c:?} escapes {parent:?}");
                }
                for d in &chips[i + 1..] {
                    assert!(!overlaps(*c, *d), "chips {c:?} {d:?} overlap");
                }
            }
        }
    }

    #[test]
    fn layout_has_no_overlap_at_minimum_comfortable_and_degenerate_sizes() {
        check(Rect::new(1, 1, 38, 6)); // 40x8 modal
        check(Rect::new(1, 1, 98, 16)); // 100x18 modal
        check(Rect::new(1, 1, 22, 6)); // 24 columns
        for h in 0..8 {
            for w in [0u16, 1, 5, 8, 20, 23, 24, 39, 40] {
                check(Rect::new(3, 2, w, h));
            }
        }
    }

    #[test]
    fn layout_rows_follow_the_documented_order() {
        let inner = Rect::new(1, 1, 98, 16);
        let l = find_layout(inner, &BiltermConfig::default());
        assert_eq!(l.tabs, Rect::new(1, 1, 98, 1));
        assert_eq!(l.query.y, 2);
        assert_eq!(l.footer, Rect::new(1, 16, 98, 1));
        assert_eq!(l.source_line, Rect::new(1, 15, 98, 1));
        assert_eq!(l.list, Rect::new(1, 3, 98, 12));
        assert_eq!(l.clear_chip.x + l.clear_chip.width, inner.x + inner.width);
        assert_eq!(l.tab_chips[0], Rect::new(1, 1, 8, 1));
        assert_eq!(l.tab_chips[1].x, 1 + 8 + 1);
        assert!(l.footer_chips.iter().all(|c| c.width > 0));
        assert!(l.footer_chips[0].x < l.footer_chips[3].x);
    }
}
