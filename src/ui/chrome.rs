use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::ui::glyphs::GlyphSet;
use crate::ui::theme::ThemePalette;

/// Renders a classic Borland Turbo Vision translucent character-cell drop shadow (`UI-01`):
/// - **2 columns wide** along the right edge (`rect.x + rect.width .. rect.x + rect.width + 2`, `y = rect.y + 1 .. rect.y + rect.height + 1`)
/// - **1 row tall** along the bottom edge (`x = rect.x + 2 .. rect.x + rect.width + 2`, `y = rect.y + rect.height`)
///
/// Unlike opaque block shadows, this preserves the underlying PTY characters in `buf` while
/// mutating their `Style` to `DarkGray` on `Black` (classic VGA attribute `0x08` shadow).
pub fn render_drop_shadow(buf: &mut Buffer, rect: Rect, bounds: Rect) {
    let clamped_bounds = bounds.intersection(buf.area);
    if rect.width == 0 || rect.height == 0 {
        return;
    }

    let shadow_style = Style::default()
        .fg(Color::DarkGray)
        .bg(Color::Black)
        .remove_modifier(Modifier::BOLD);

    let right_x_start = rect.x.saturating_add(rect.width);
    let right_x_end = right_x_start.saturating_add(2);
    let bottom_y = rect.y.saturating_add(rect.height);

    // 2-column right shadow
    for y in rect.y.saturating_add(1)..=bottom_y {
        for x in right_x_start..right_x_end {
            if x >= clamped_bounds.x
                && x < clamped_bounds.x + clamped_bounds.width
                && y >= clamped_bounds.y
                && y < clamped_bounds.y + clamped_bounds.height
            {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(shadow_style);
                }
            }
        }
    }

    // 1-row bottom shadow
    if bottom_y >= clamped_bounds.y && bottom_y < clamped_bounds.y + clamped_bounds.height {
        for x in rect.x.saturating_add(2)..right_x_end {
            if x >= clamped_bounds.x && x < clamped_bounds.x + clamped_bounds.width {
                if let Some(cell) = buf.cell_mut((x, bottom_y)) {
                    cell.set_style(shadow_style);
                }
            }
        }
    }
}

/// Renders a Borland Turbo Vision window frame (`╔═╗` active / `┌─┐` inactive) with:
/// - Top-left `[▪]` close widget
/// - Centered/left title badge (`[1] PANE #1 (Z:1/2)`)
/// - Top-right `[↕]` zoom widget
/// - Bottom-right `◢` resize corner glyph (`UI-01`).
pub fn render_borland_frame(
    outer: Rect,
    title: &str,
    active: bool,
    palette: &ThemePalette,
    glyphs: &GlyphSet,
    buf: &mut Buffer,
) {
    if outer.width < 4 || outer.height < 3 {
        return;
    }
    let border_type = if active {
        BorderType::Double
    } else {
        BorderType::Plain
    };
    let border_fg = if active {
        palette.active_border_fg
    } else {
        palette.inactive_border_fg
    };
    let frame_bg = palette.window_frame_bg;

    let block = Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(palette.title_fg)
                .bg(frame_bg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(Style::default().fg(border_fg).bg(frame_bg));
    block.render(outer, buf);

    // Top-right [↕] zoom badge and [▪] close badge when window is wide enough
    if outer.width >= 24 {
        let right_edge = outer.x + outer.width;
        let badges = format!("{}{}", glyphs.zoom_badge, glyphs.close_badge);
        let start_x = right_edge.saturating_sub(badges.chars().count() as u16 + 2);
        for (i, ch) in badges.chars().enumerate() {
            let x = start_x + i as u16;
            if x < right_edge - 1 {
                if let Some(cell) = buf.cell_mut((x, outer.y)) {
                    let mut sym = [0u8; 4];
                    cell.set_symbol(ch.encode_utf8(&mut sym));
                    cell.set_style(
                        Style::default()
                            .fg(palette.title_fg)
                            .bg(frame_bg)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
    }

    // Bottom-right ◢ resize corner
    let br_x = outer.x + outer.width - 1;
    let br_y = outer.y + outer.height - 1;
    if let Some(cell) = buf.cell_mut((br_x, br_y)) {
        cell.set_symbol(glyphs.resize_corner);
        cell.set_style(
            Style::default()
                .fg(palette.title_fg)
                .bg(frame_bg)
                .add_modifier(Modifier::BOLD),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_shadow_dims_2col_right_and_1row_bottom_without_erasing_text() {
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        buf.cell_mut((10, 2)).unwrap().set_symbol("S");
        buf.cell_mut((4, 5)).unwrap().set_symbol("B");

        // Window at x=2, y=1, width=8 (right edge = 10), height=4 (bottom edge = 5)
        render_drop_shadow(&mut buf, Rect::new(2, 1, 8, 4), area);

        // Right shadow at (10, 2) preserves "S" and sets DarkGray on Black
        assert_eq!(buf.cell((10, 2)).unwrap().symbol(), "S");
        assert_eq!(buf.cell((10, 2)).unwrap().style().fg, Some(Color::DarkGray));
        assert_eq!(buf.cell((10, 2)).unwrap().style().bg, Some(Color::Black));

        // Bottom shadow at (4, 5) preserves "B" and sets DarkGray on Black
        assert_eq!(buf.cell((4, 5)).unwrap().symbol(), "B");
        assert_eq!(buf.cell((4, 5)).unwrap().style().fg, Some(Color::DarkGray));
    }
}
