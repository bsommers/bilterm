//! High-resolution GUI vector and texture renderer for the session map (Phase 54, VIEW-02, D-03).

use ratatui::layout::Rect;
use crate::ui::pixel_canvas::PixelCanvasBuffer;
use crate::ui::renderer::GuiImageTexture;
use crate::wm::WindowManager;

/// Dedicated texture ID for the session packing viewer GUI vector surface.
pub const SESSION_MAP_TEXTURE_ID: u32 = 970_000;

/// Renders the session map into a GUI image texture for hardware-accelerated presentation.
pub fn draw_session_map_vector(
    _wm: &WindowManager,
    rect: Rect,
    cell_w: u16,
    cell_h: u16,
) -> Option<GuiImageTexture> {
    if rect.width == 0 || rect.height == 0 || cell_w == 0 || cell_h == 0 {
        return None;
    }
    let px_w = (rect.width as u32).saturating_mul(cell_w as u32);
    let px_h = (rect.height as u32).saturating_mul(cell_h as u32);
    if px_w == 0 || px_h == 0 {
        return None;
    }
    let mut canvas = PixelCanvasBuffer::new(px_w, px_h, [0x16, 0x18, 0x22, 0xf0]);
    // Draw subtle border
    canvas.draw_rect_aa(0.0, 0.0, px_w as f32, px_h as f32, false, (0x4a, 0x55, 0x68), 1.0);

    Some(GuiImageTexture {
        id: SESSION_MAP_TEXTURE_ID,
        x_px: 0,
        y_px: 0,
        width_px: px_w,
        height_px: px_h,
        pixel_width: cell_w,
        pixel_height: cell_h,
        rgba_bytes: canvas.rgba,
    })
}
