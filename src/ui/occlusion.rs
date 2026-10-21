use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, Widget};
use unicode_width::UnicodeWidthStr;

/// Sanitizes 2-column wide Unicode/CJK/Emoji/Nerd Font graphemes straddling the left or right
/// boundary of `rect`, then clears `rect` in `buf` (`WM-04`).
///
/// Prevents wide characters on underlying PTY canvases from having only half of their 2-cell
/// footprint overwritten by a floating command prompt, stacked window, drop shadow, or Guake overlay.
pub fn occlude_rect(buf: &mut Buffer, rect: Rect) {
    let clamped = rect.intersection(buf.area);
    if clamped.width == 0 || clamped.height == 0 {
        return;
    }

    let buf_right = buf.area.x + buf.area.width;
    let rect_right = clamped.x + clamped.width;

    for y in clamped.y..clamped.y + clamped.height {
        // 1. Left boundary: if a 2-column wide character starts at `clamped.x - 1`, its second
        //    column is at `clamped.x` (inside the occluded region). Blank `clamped.x - 1` so
        //    the wide glyph does not bleed into the overlay's left border.
        if clamped.x > buf.area.x {
            let left_neighbor_x = clamped.x - 1;
            let is_wide = buf
                .cell((left_neighbor_x, y))
                .is_some_and(|c| UnicodeWidthStr::width(c.symbol()) >= 2);
            if is_wide {
                if let Some(cell) = buf.cell_mut((left_neighbor_x, y)) {
                    cell.set_symbol(" ");
                }
            }
        }

        // 2. Right boundary: if a 2-column wide character starts at `rect_right - 1` (the last
        //    column inside the occluded region), its continuation cell at `rect_right` must also
        //    be reset to `" "` so no orphaned right-half artifact remains outside the right border.
        if rect_right < buf_right {
            let last_inside_x = rect_right - 1;
            let is_wide = buf
                .cell((last_inside_x, y))
                .is_some_and(|c| UnicodeWidthStr::width(c.symbol()) >= 2);
            if is_wide {
                if let Some(cell) = buf.cell_mut((rect_right, y)) {
                    cell.set_symbol(" ");
                }
            }
        }
    }

    Clear.render(clamped, buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occlude_rect_blanks_split_wide_cjk_and_emoji_graphemes_at_edges() {
        let area = Rect::new(0, 0, 20, 3);
        let mut buf = Buffer::empty(area);

        // Place 2-column wide CJK character '漢' at x=4 (occupies cols 4 and 5)
        // and '語' at x=9 (occupies cols 9 and 10)
        buf.cell_mut((4, 1)).unwrap().set_symbol("漢");
        buf.cell_mut((5, 1)).unwrap().set_symbol(" ");
        buf.cell_mut((9, 1)).unwrap().set_symbol("語");
        buf.cell_mut((10, 1)).unwrap().set_symbol("X");

        // Occlude x=5..10 (width=5, covering cols 5, 6, 7, 8, 9)
        // Left boundary x=5 splits '漢' at x=4; right boundary x=9 splits '語' at x=9..10.
        occlude_rect(&mut buf, Rect::new(5, 1, 5, 1));

        assert_eq!(buf.cell((4, 1)).unwrap().symbol(), " ");
        assert_eq!(buf.cell((9, 1)).unwrap().symbol(), " ");
        assert_eq!(buf.cell((10, 1)).unwrap().symbol(), " ");
    }
}
