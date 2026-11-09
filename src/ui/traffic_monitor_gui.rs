//! GUI vector card for the TRAFFIC MONITOR window (`TMON-03`, Phase 64-03). Bars come from the
//! same per-second series (`Vec<u64>`) the TUI sparkline uses.

use ratatui::layout::Rect;

use crate::ui::pixel_canvas::PixelCanvasBuffer;
use crate::ui::renderer::GuiImageTexture;
use crate::wm::WindowManager;

/// Dedicated texture id for the traffic monitor GUI card.
pub const TRAFFIC_MONITOR_TEXTURE_ID: u32 = 990_000;

/// Bar heights (0..=`max_px`) for `series`, scaled to its own maximum.
pub fn bar_heights(series: &[u64], max_px: u32) -> Vec<u32> {
    let max = series.iter().copied().max().unwrap_or(0).max(1);
    series.iter().map(|v| (v * u64::from(max_px) / max) as u32).collect()
}

/// Renders the window backdrop plus a combined rx+tx bar strip of the first row group.
pub fn draw_traffic_monitor_vector(wm: &WindowManager, rect: Rect, cell_w: u16, cell_h: u16) -> Option<GuiImageTexture> {
    if rect.width == 0 || rect.height == 0 || cell_w == 0 || cell_h == 0 {
        return None;
    }
    let px_w = u32::from(rect.width) * u32::from(cell_w);
    let px_h = u32::from(rect.height) * u32::from(cell_h);
    let mut canvas = PixelCanvasBuffer::new(px_w, px_h, [0x14, 0x18, 0x24, 0xf2]);
    canvas.draw_rect_aa(0.0, 0.0, px_w as f32, px_h as f32, false, (0x4a, 0x55, 0x68), 1.0);
    let rows = wm.traffic_rows().rows;
    if let Some(r) = rows.first() {
        let combined: Vec<u64> = r.series_rx.iter().zip(&r.series_tx).map(|(a, b)| a + b).collect();
        let strip_h = u32::from(cell_h) * 2;
        let bw = (px_w.saturating_sub(8) / combined.len().max(1) as u32).max(1);
        for (i, h) in bar_heights(&combined, strip_h).into_iter().enumerate() {
            let x = 4 + i as u32 * bw;
            if h > 0 && x + bw <= px_w {
                canvas.draw_rect_aa(x as f32, (px_h - 4 - h) as f32, bw as f32 - 1.0, h as f32, true, (0x5f, 0xb3, 0x8f), 1.0);
            }
        }
    }
    Some(GuiImageTexture {
        id: TRAFFIC_MONITOR_TEXTURE_ID,
        x_px: 0,
        y_px: 0,
        width_px: px_w,
        height_px: px_h,
        pixel_width: cell_w,
        pixel_height: cell_h,
        rgba_bytes: canvas.rgba,
    })
}
