//! Find window renderer and geometry (Phase 50.1): frame, tabs, query line, result rows, footer.
//!
//! 50.1-01 owns the frame and the geometry (`find_layout`); 50.1-07 draws rows, tabs and chips into
//! the rects `find_layout` returns, 50.1-08 hit-tests the same rects.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::config::BiltermConfig;
use crate::ui::chrome::{render_borland_frame_styled, render_drop_shadow_styled};
use crate::ui::glyphs::GlyphSet;
use crate::ui::occlusion::occlude_rect;
use crate::ui::theme::ThemePalette;
use crate::ui::visual_style::{apply_frosted_glass_surface, render_liquid_ambient_halo};
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
    let Some((kind, query, _selected)) = wm.find_modal_state() else {
        return;
    };
    let Some(outer) = wm.active_modal_rect(workspace_area) else {
        return;
    };
    render_frame(outer, workspace_area, cfg, palette, glyphs, buf);
    let inner = inset_window_frame(outer);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let header = Line::from(Span::styled(
        format!("Find {} > {query}_", kind.as_str()),
        Style::default()
            .fg(palette.prompt_prefix_fg)
            .bg(palette.window_frame_bg)
            .add_modifier(Modifier::BOLD),
    ));
    Paragraph::new(vec![header]).render(inner, buf);
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
