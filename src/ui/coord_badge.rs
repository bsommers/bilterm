//! Title-bar hierarchical coordinate badge (`COORD-03`, Phase 71): `(w1,main,y2)` drawn right-aligned,
//! one cell left of the `[_][□][X]` controls. Drawn only when the style is not `Off` and the text fits
//! without touching the title or the controls, so `Off` leaves every cell untouched.

use crate::ui::theme::ThemePalette;
use crate::ui::glyphs::GlyphSet;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// Draws `text` into the title row of `outer`; returns `true` when drawn.
pub fn render_coord_badge(
    outer: Rect,
    title: &str,
    text: &str,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) -> bool {
    if text.is_empty() || outer.width < 24 || outer.height < 3 {
        return false;
    }
    let ctrl_w: u16 = [glyphs.minimize_badge, glyphs.zoom_badge, glyphs.close_badge]
        .iter()
        .map(|b| Span::raw(*b).width() as u16)
        .sum();
    let right_edge = outer.x + outer.width;
    let controls_x = right_edge.saturating_sub(ctrl_w + 2);
    let w = Span::raw(text).width() as u16;
    // Title occupies `outer.x + 1 ..` as " {title} "; keep one blank cell after it.
    let title_end = if title.is_empty() { outer.x + 1 } else { outer.x + 1 + Span::raw(title).width() as u16 + 2 };
    let Some(x) = controls_x.checked_sub(w + 1) else { return false };
    if x <= title_end {
        return false;
    }
    let style = Style::default().fg(palette.title_fg).bg(palette.window_frame_bg).add_modifier(Modifier::BOLD);
    buf.set_string(x, outer.y, text, style);
    true
}
