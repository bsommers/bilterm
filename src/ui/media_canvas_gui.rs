//! GUI texture generation for Browser Canvas and Video Player Canvas (`WEB-01..02`, `VID-01..02`).

use ratatui::layout::Rect;
use crate::pane::CompositePane;
use crate::ui::renderer::GuiImageTexture;

/// Generates GUI image textures with `FloatingCommandPrompt` alpha cutouts for active media canvases.
pub fn render_media_canvas_textures(
    pane: &CompositePane,
    canvas_rect: Rect,
    prompt_rect: Option<Rect>,
    pane_id: usize,
    eff_cw: u16,
    eff_ch: u16,
    images: &mut Vec<GuiImageTexture>,
) {
    if canvas_rect.width == 0 || canvas_rect.height <= 1 {
        return;
    }

    // 1. Browser Canvas Texture (id: 910_000 + pane_id)
    if let Some(browser) = pane.browser_canvas()
        && let Some(frame) = browser.cached_frame()
    {
        let viewport_rect = Rect::new(
            canvas_rect.x,
            canvas_rect.y.saturating_add(1),
            canvas_rect.width,
            canvas_rect.height.saturating_sub(1),
        );
        if viewport_rect.width > 0 && viewport_rect.height > 0 {
            let dst_w = (u32::from(viewport_rect.width) * u32::from(eff_cw)) as usize;
            let dst_h = (u32::from(viewport_rect.height) * u32::from(eff_ch)) as usize;
            if dst_w > 0 && dst_h > 0 {
                let mut rgba = resize_or_crop_rgba(
                    &frame.rgba,
                    frame.width_px as usize,
                    frame.height_px as usize,
                    dst_w,
                    dst_h,
                );

                // Alpha cutout under FloatingCommandPrompt
                if let Some(pr) = prompt_rect {
                    mask_cutout(&mut rgba, dst_w, dst_h, viewport_rect, pr, eff_cw, eff_ch);
                }

                images.push(GuiImageTexture {
                    id: 910_000 + (pane_id as u32),
                    x_px: u32::from(viewport_rect.x) * u32::from(eff_cw),
                    y_px: u32::from(viewport_rect.y) * u32::from(eff_ch),
                    width_px: dst_w as u32,
                    height_px: dst_h as u32,
                    pixel_width: dst_w.min(u16::MAX as usize) as u16,
                    pixel_height: dst_h.min(u16::MAX as usize) as u16,
                    rgba_bytes: rgba,
                });
            }
        }
    }

    // 2. Video Player Canvas Texture (id: 920_000 + pane_id)
    if let Some(video) = pane.video_canvas()
        && let Some(frame) = video.cached_frame()
    {
        let viewport_rect = Rect::new(
            canvas_rect.x,
            canvas_rect.y,
            canvas_rect.width,
            canvas_rect.height.saturating_sub(1),
        );
            if viewport_rect.width > 0 && viewport_rect.height > 0 {
                let dst_w = (u32::from(viewport_rect.width) * u32::from(eff_cw)) as usize;
                let dst_h = (u32::from(viewport_rect.height) * u32::from(eff_ch)) as usize;
                if dst_w > 0 && dst_h > 0 {
                    let mut rgba = resize_or_crop_rgba(
                        &frame.rgba,
                        frame.width_px as usize,
                        frame.height_px as usize,
                        dst_w,
                        dst_h,
                    );

                    // Alpha cutout under FloatingCommandPrompt
                    if let Some(pr) = prompt_rect {
                        mask_cutout(&mut rgba, dst_w, dst_h, viewport_rect, pr, eff_cw, eff_ch);
                    }

                    images.push(GuiImageTexture {
                        id: 920_000 + (pane_id as u32),
                        x_px: u32::from(viewport_rect.x) * u32::from(eff_cw),
                        y_px: u32::from(viewport_rect.y) * u32::from(eff_ch),
                        width_px: dst_w as u32,
                        height_px: dst_h as u32,
                        pixel_width: dst_w.min(u16::MAX as usize) as u16,
                        pixel_height: dst_h.min(u16::MAX as usize) as u16,
                        rgba_bytes: rgba,
                    });
                }
            }
        }
    }

fn mask_cutout(
    rgba: &mut [u8],
    dst_w: usize,
    dst_h: usize,
    viewport_rect: Rect,
    prompt_rect: Rect,
    eff_cw: u16,
    eff_ch: u16,
) {
    let px_x = prompt_rect.x.saturating_sub(viewport_rect.x);
    let px_y = prompt_rect.y.saturating_sub(viewport_rect.y);
    let ox = (u32::from(px_x) * u32::from(eff_cw)) as usize;
    let oy = (u32::from(px_y) * u32::from(eff_ch)) as usize;
    let ow = (u32::from(prompt_rect.width) * u32::from(eff_cw)) as usize;
    let oh = (u32::from(prompt_rect.height) * u32::from(eff_ch)) as usize;

    if prompt_rect.y + prompt_rect.height <= viewport_rect.y
        || prompt_rect.y >= viewport_rect.bottom()
        || prompt_rect.x + prompt_rect.width <= viewport_rect.x
        || prompt_rect.x >= viewport_rect.right()
    {
        return;
    }

    for py in oy..(oy + oh).min(dst_h) {
        for px in ox..(ox + ow).min(dst_w) {
            let idx = (py * dst_w + px) * 4;
            rgba[idx..idx + 4].copy_from_slice(&[0, 0, 0, 0]);
        }
    }
}

fn resize_or_crop_rgba(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || src.len() < src_w * src_h * 4 {
        return vec![0u8; dst_w * dst_h * 4];
    }
    if src_w == dst_w && src_h == dst_h {
        return src[..dst_w * dst_h * 4].to_vec();
    }

    let mut out = vec![0u8; dst_w * dst_h * 4];
    for y in 0..dst_h {
        let sy = (y * src_h) / dst_h;
        for x in 0..dst_w {
            let sx = (x * src_w) / dst_w;
            let s_idx = (sy * src_w + sx) * 4;
            let d_idx = (y * dst_w + x) * 4;
            out[d_idx..d_idx + 4].copy_from_slice(&src[s_idx..s_idx + 4]);
        }
    }
    out
}
