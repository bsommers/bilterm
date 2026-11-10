//! Cell-to-pixel geometry for the browser viewport (`WEB-02`).
//!
//! The frame is always stretched over the whole viewport cell rect (half-blocks in the TUI, a
//! scaled texture in the GUI), so a cell maps to the frame by proportion. No fixed cell size.

use ratatui::layout::Rect;

/// Nominal pixels rendered per terminal cell when sizing the page (cells are about 1:2).
pub const CELL_PX_W: u16 = 10;
pub const CELL_PX_H: u16 = 20;
pub const MIN_VIEWPORT_PX: (u16, u16) = (320, 200);
pub const MAX_VIEWPORT_PX: (u16, u16) = (3840, 2160);

/// Viewport cell rect below the 1-row nav bar.
pub fn viewport_body(canvas_rect: Rect) -> Rect {
    Rect::new(canvas_rect.x, canvas_rect.y.saturating_add(1), canvas_rect.width, canvas_rect.height.saturating_sub(1))
}

/// Page size in pixels for a canvas of `canvas_cols x canvas_rows` cells (nav bar row excluded).
pub fn viewport_px_for_cells(canvas_cols: u16, canvas_rows: u16) -> (u16, u16) {
    let w = canvas_cols.saturating_mul(CELL_PX_W).clamp(MIN_VIEWPORT_PX.0, MAX_VIEWPORT_PX.0);
    let h = canvas_rows.saturating_sub(1).saturating_mul(CELL_PX_H).clamp(MIN_VIEWPORT_PX.1, MAX_VIEWPORT_PX.1);
    (w, h)
}

/// Maps the centre of cell `(col, row)` inside `body` to a pixel of a `frame_w x frame_h` frame.
pub fn cell_to_frame_px(body: Rect, col: u16, row: u16, frame_w: u16, frame_h: u16) -> Option<(u16, u16)> {
    if body.width == 0 || body.height == 0 || frame_w == 0 || frame_h == 0 {
        return None;
    }
    if col < body.x || col >= body.right() || row < body.y || row >= body.bottom() {
        return None;
    }
    let centre = |cell: u16, cells: u16, px: u16| {
        let v = (u32::from(cell) * 2 + 1) * u32::from(px) / (u32::from(cells) * 2);
        v.min(u32::from(px) - 1) as u16
    };
    Some((centre(col - body.x, body.width, frame_w), centre(row - body.y, body.height, frame_h)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_map_to_their_centre_in_the_frame() {
        let body = Rect::new(2, 5, 80, 23);
        // 800x460 frame: each cell covers 10x20 px.
        assert_eq!(cell_to_frame_px(body, 2, 5, 800, 460), Some((5, 10)));
        assert_eq!(cell_to_frame_px(body, 81, 27, 800, 460), Some((795, 450)));
        // A GUI frame of another size maps by proportion, not by a fixed cell size.
        assert_eq!(cell_to_frame_px(body, 42, 5, 1600, 920), Some((810, 20)));
        assert_eq!(cell_to_frame_px(body, 42, 16, 400, 230), Some((202, 115)));
    }

    #[test]
    fn points_outside_the_body_or_degenerate_frames_map_to_nothing() {
        let body = Rect::new(2, 5, 80, 23);
        assert_eq!(cell_to_frame_px(body, 1, 5, 800, 460), None);
        assert_eq!(cell_to_frame_px(body, 82, 5, 800, 460), None);
        assert_eq!(cell_to_frame_px(body, 2, 4, 800, 460), None);
        assert_eq!(cell_to_frame_px(body, 2, 28, 800, 460), None);
        assert_eq!(cell_to_frame_px(body, 2, 5, 0, 460), None);
        assert_eq!(cell_to_frame_px(Rect::new(0, 0, 0, 0), 0, 0, 800, 460), None);
        // A frame narrower than the cell grid still yields in-range pixels.
        assert_eq!(cell_to_frame_px(body, 81, 27, 40, 10), Some((39, 9)));
    }

    #[test]
    fn viewport_size_follows_the_cell_grid_within_limits() {
        assert_eq!(viewport_px_for_cells(80, 24), (800, 460));
        assert_eq!(viewport_px_for_cells(10, 3), MIN_VIEWPORT_PX);
        assert_eq!(viewport_px_for_cells(1000, 400), MAX_VIEWPORT_PX);
        assert_eq!(viewport_body(Rect::new(1, 2, 80, 24)), Rect::new(1, 3, 80, 23));
    }
}
