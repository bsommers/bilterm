//! High-resolution GUI vector and texture renderer for the `CLIPBOARD` window (`CLIP-03`, Phase 57).

use ratatui::layout::Rect;
use crate::ui::pixel_canvas::PixelCanvasBuffer;
use crate::ui::renderer::GuiImageTexture;
use crate::wm::WindowManager;

/// Dedicated texture ID for the clipboard viewer GUI vector surface.
pub const CLIPBOARD_WINDOW_TEXTURE_ID: u32 = 980_000;

/// Renders the clipboard viewer backdrop into a GUI image texture for hardware-accelerated presentation.
pub fn draw_clipboard_window_vector(
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
    let mut canvas = PixelCanvasBuffer::new(px_w, px_h, [0x14, 0x18, 0x24, 0xf2]);
    canvas.draw_rect_aa(
        0.0,
        0.0,
        px_w as f32,
        px_h as f32,
        false,
        (0x4a, 0x55, 0x68),
        1.0,
    );

    Some(GuiImageTexture {
        id: CLIPBOARD_WINDOW_TEXTURE_ID,
        x_px: 0,
        y_px: 0,
        width_px: px_w,
        height_px: px_h,
        pixel_width: cell_w,
        pixel_height: cell_h,
        rgba_bytes: canvas.rgba,
    })
}
