//! Crisp IBM PC VGA / CP437 Bitmap Font & Unicode Box-Drawing / Block-Element Rasterizer (`GPU-01`).
//!
//! Renders ASCII (`0x20..=0x7E`), Unicode single/double box-drawing (`─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬`),
//! shade/block characters (`░▒▓█▀▄▌▐▪▫◢►◄▲▼`), and `2×4` Braille patterns (`U+2800..=U+28FF`)
//! directly into [`GuiFrameSurface`](crate::ui::renderer::GuiFrameSurface)'s RGBA pixel buffer.

/// Rasterizes a single glyph cell at `(x0, y0)` with cell size `(cw, ch)` into `fb` (`w_px * h_px * 4`).
#[allow(clippy::too_many_arguments)]
pub fn rasterize_glyph_cell(
    fb: &mut [u8],
    w_px: u32,
    h_px: u32,
    x0: u32,
    y0: u32,
    cw: u32,
    ch: u32,
    glyph: char,
    fg_rgba: [u8; 4],
    bold: bool,
) {
    if glyph == ' ' || cw == 0 || ch == 0 || x0 >= w_px || y0 >= h_px {
        return;
    }

    if rasterize_box_or_block(fb, w_px, h_px, x0, y0, cw, ch, glyph, fg_rgba) {
        return;
    }

    if ('\u{2800}'..='\u{28FF}').contains(&glyph) {
        let mask = (glyph as u32 - 0x2800) as u8;
        let dot_positions: [(u8, u32, u32); 8] = [
            (0x01, 0, 0),
            (0x02, 0, 1),
            (0x04, 0, 2),
            (0x08, 1, 0),
            (0x10, 1, 1),
            (0x20, 1, 2),
            (0x40, 0, 3),
            (0x80, 1, 3),
        ];
        for (bit, dx, dy) in dot_positions {
            if (mask & bit) != 0 {
                let px_start = x0 + (dx * cw) / 2;
                let px_end = x0 + ((dx + 1) * cw) / 2;
                let py_start = y0 + (dy * ch) / 4;
                let py_end = y0 + ((dy + 1) * ch) / 4;
                fill_rect(
                    fb,
                    w_px,
                    h_px,
                    px_start,
                    py_start,
                    (px_end - px_start).max(1),
                    (py_end - py_start).max(1),
                    fg_rgba,
                );
            }
        }
        return;
    }

    let bitmap = glyph_5x7_rows(glyph);
    let pad_x = (cw.saturating_sub(5)) / 2;
    let pad_y = (ch.saturating_sub(7)) / 2;
    let draw_w = cw.saturating_sub(pad_x * 2).max(5);
    let draw_h = ch.saturating_sub(pad_y * 2).max(7);

    for gy in 0..draw_h {
        let src_r = ((gy * 7) / draw_h).min(6) as usize;
        let row_bits = bitmap[src_r];
        if row_bits == 0 {
            continue;
        }
        for gx in 0..draw_w {
            let src_c = ((gx * 5) / draw_w).min(4);
            let on = ((row_bits >> (4 - src_c)) & 1) != 0;
            if on {
                let px = x0 + pad_x + gx;
                let py = y0 + pad_y + gy;
                put_px(fb, w_px, h_px, px, py, fg_rgba);
                if bold {
                    put_px(fb, w_px, h_px, px + 1, py, fg_rgba);
                }
            }
        }
    }
}

#[inline]
fn put_px(fb: &mut [u8], w_px: u32, h_px: u32, px: u32, py: u32, rgba: [u8; 4]) {
    if px < w_px && py < h_px {
        let idx = ((py * w_px + px) * 4) as usize;
        fb[idx..idx + 4].copy_from_slice(&rgba);
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    fb: &mut [u8],
    w_px: u32,
    h_px: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rgba: [u8; 4],
) {
    let x1 = (x + w).min(w_px);
    let y1 = (y + h).min(h_px);
    for py in y.min(h_px)..y1 {
        for px in x.min(w_px)..x1 {
            let idx = ((py * w_px + px) * 4) as usize;
            fb[idx..idx + 4].copy_from_slice(&rgba);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rasterize_box_or_block(
    fb: &mut [u8],
    w_px: u32,
    h_px: u32,
    x0: u32,
    y0: u32,
    cw: u32,
    ch: u32,
    glyph: char,
    fg: [u8; 4],
) -> bool {
    let mx = x0 + cw / 2;
    let my = y0 + ch / 2;
    let d1_x = mx.saturating_sub(1);
    let d2_x = (mx + 1).min(x0 + cw.saturating_sub(1));
    let d1_y = my.saturating_sub(1);
    let d2_y = (my + 1).min(y0 + ch.saturating_sub(1));

    match glyph {
        '█' => fill_rect(fb, w_px, h_px, x0, y0, cw, ch, fg),
        '▀' => fill_rect(fb, w_px, h_px, x0, y0, cw, (ch / 2).max(1), fg),
        '▄' => {
            let top = ch / 2;
            fill_rect(fb, w_px, h_px, x0, y0 + top, cw, ch.saturating_sub(top), fg);
        }
        '▌' => fill_rect(fb, w_px, h_px, x0, y0, (cw / 2).max(1), ch, fg),
        '▐' => {
            let left = cw / 2;
            fill_rect(fb, w_px, h_px, x0 + left, y0, cw.saturating_sub(left), ch, fg);
        }
        '░' | '▒' | '▓' => {
            let mod_step = match glyph {
                '░' => 4,
                '▒' => 2,
                _ => 1,
            };
            for gy in 0..ch {
                for gx in 0..cw {
                    let draw = if mod_step == 1 {
                        ((gx + gy) & 3) != 0
                    } else {
                        ((gx + gy * 2) % mod_step) == 0
                    };
                    if draw {
                        put_px(fb, w_px, h_px, x0 + gx, y0 + gy, fg);
                    }
                }
            }
        }
        '▪' | '■' => {
            let bx = cw / 4;
            let by = ch / 3;
            fill_rect(
                fb,
                w_px,
                h_px,
                x0 + bx,
                y0 + by,
                cw.saturating_sub(bx * 2).max(2),
                ch.saturating_sub(by * 2).max(2),
                fg,
            );
        }
        '◢' => {
            for gy in 0..ch {
                let start_gx = cw.saturating_sub(((gy + 1) * cw) / ch.max(1));
                for gx in start_gx..cw {
                    put_px(fb, w_px, h_px, x0 + gx, y0 + gy, fg);
                }
            }
        }
        '─' | '━' | '╌' | '┄' => fill_rect(fb, w_px, h_px, x0, my, cw, 1, fg),
        '│' | '┃' | '╎' | '┆' => fill_rect(fb, w_px, h_px, mx, y0, 1, ch, fg),
        '═' => {
            fill_rect(fb, w_px, h_px, x0, d1_y, cw, 1, fg);
            fill_rect(fb, w_px, h_px, x0, d2_y, cw, 1, fg);
        }
        '║' => {
            fill_rect(fb, w_px, h_px, d1_x, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, d2_x, y0, 1, ch, fg);
        }
        '┌' | '╭' | '┏' => {
            fill_rect(fb, w_px, h_px, mx, my, cw - (mx - x0), 1, fg);
            fill_rect(fb, w_px, h_px, mx, my, 1, ch - (my - y0), fg);
        }
        '┐' | '╮' | '┓' => {
            fill_rect(fb, w_px, h_px, x0, my, (mx - x0) + 1, 1, fg);
            fill_rect(fb, w_px, h_px, mx, my, 1, ch - (my - y0), fg);
        }
        '└' | '╰' | '┗' => {
            fill_rect(fb, w_px, h_px, mx, my, cw - (mx - x0), 1, fg);
            fill_rect(fb, w_px, h_px, mx, y0, 1, (my - y0) + 1, fg);
        }
        '┘' | '╯' | '┛' => {
            fill_rect(fb, w_px, h_px, x0, my, (mx - x0) + 1, 1, fg);
            fill_rect(fb, w_px, h_px, mx, y0, 1, (my - y0) + 1, fg);
        }
        '├' | '┝' | '┣' => {
            fill_rect(fb, w_px, h_px, mx, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, mx, my, cw - (mx - x0), 1, fg);
        }
        '┤' | '┥' | '┫' => {
            fill_rect(fb, w_px, h_px, mx, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, x0, my, (mx - x0) + 1, 1, fg);
        }
        '┬' | '┳' => {
            fill_rect(fb, w_px, h_px, x0, my, cw, 1, fg);
            fill_rect(fb, w_px, h_px, mx, my, 1, ch - (my - y0), fg);
        }
        '┴' | '┻' => {
            fill_rect(fb, w_px, h_px, x0, my, cw, 1, fg);
            fill_rect(fb, w_px, h_px, mx, y0, 1, (my - y0) + 1, fg);
        }
        '┼' | '╋' => {
            fill_rect(fb, w_px, h_px, x0, my, cw, 1, fg);
            fill_rect(fb, w_px, h_px, mx, y0, 1, ch, fg);
        }
        '╔' => {
            fill_rect(fb, w_px, h_px, d1_x, d1_y, x0 + cw - d1_x, 1, fg);
            fill_rect(fb, w_px, h_px, d2_x, d2_y, x0 + cw - d2_x, 1, fg);
            fill_rect(fb, w_px, h_px, d1_x, d1_y, 1, y0 + ch - d1_y, fg);
            fill_rect(fb, w_px, h_px, d2_x, d2_y, 1, y0 + ch - d2_y, fg);
        }
        '╗' => {
            fill_rect(fb, w_px, h_px, x0, d1_y, d2_x - x0 + 1, 1, fg);
            fill_rect(fb, w_px, h_px, x0, d2_y, d1_x - x0 + 1, 1, fg);
            fill_rect(fb, w_px, h_px, d1_x, d2_y, 1, y0 + ch - d2_y, fg);
            fill_rect(fb, w_px, h_px, d2_x, d1_y, 1, y0 + ch - d1_y, fg);
        }
        '╚' => {
            fill_rect(fb, w_px, h_px, d1_x, y0, 1, d2_y - y0 + 1, fg);
            fill_rect(fb, w_px, h_px, d2_x, y0, 1, d1_y - y0 + 1, fg);
            fill_rect(fb, w_px, h_px, d2_x, d1_y, x0 + cw - d2_x, 1, fg);
            fill_rect(fb, w_px, h_px, d1_x, d2_y, x0 + cw - d1_x, 1, fg);
        }
        '╝' => {
            fill_rect(fb, w_px, h_px, d1_x, y0, 1, d1_y - y0 + 1, fg);
            fill_rect(fb, w_px, h_px, d2_x, y0, 1, d2_y - y0 + 1, fg);
            fill_rect(fb, w_px, h_px, x0, d1_y, d1_x - x0 + 1, 1, fg);
            fill_rect(fb, w_px, h_px, x0, d2_y, d2_x - x0 + 1, 1, fg);
        }
        '╠' | '╟' => {
            fill_rect(fb, w_px, h_px, d1_x, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, d2_x, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, d2_x, d1_y, x0 + cw - d2_x, 1, fg);
            fill_rect(fb, w_px, h_px, d2_x, d2_y, x0 + cw - d2_x, 1, fg);
        }
        '╣' | '╢' => {
            fill_rect(fb, w_px, h_px, d1_x, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, d2_x, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, x0, d1_y, d1_x - x0 + 1, 1, fg);
            fill_rect(fb, w_px, h_px, x0, d2_y, d1_x - x0 + 1, 1, fg);
        }
        '╦' | '╤' => {
            fill_rect(fb, w_px, h_px, x0, d1_y, cw, 1, fg);
            fill_rect(fb, w_px, h_px, x0, d2_y, cw, 1, fg);
            fill_rect(fb, w_px, h_px, d1_x, d2_y, 1, y0 + ch - d2_y, fg);
            fill_rect(fb, w_px, h_px, d2_x, d2_y, 1, y0 + ch - d2_y, fg);
        }
        '╩' | '╧' => {
            fill_rect(fb, w_px, h_px, x0, d1_y, cw, 1, fg);
            fill_rect(fb, w_px, h_px, x0, d2_y, cw, 1, fg);
            fill_rect(fb, w_px, h_px, d1_x, y0, 1, d1_y - y0 + 1, fg);
            fill_rect(fb, w_px, h_px, d2_x, y0, 1, d1_y - y0 + 1, fg);
        }
        '╬' | '╪' | '╫' => {
            fill_rect(fb, w_px, h_px, x0, d1_y, cw, 1, fg);
            fill_rect(fb, w_px, h_px, x0, d2_y, cw, 1, fg);
            fill_rect(fb, w_px, h_px, d1_x, y0, 1, ch, fg);
            fill_rect(fb, w_px, h_px, d2_x, y0, 1, ch, fg);
        }
        _ => return false,
    }
    true
}

/// Returns 7 rows of 5-bit patterns (`0bxxxxx`) for printable ASCII and common terminal symbols.
fn glyph_5x7_rows(ch: char) -> [u8; 7] {
    match ch {
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '"' => [0x0A, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00],
        '#' => [0x0A, 0x1F, 0x0A, 0x0A, 0x1F, 0x0A, 0x00],
        '$' => [0x04, 0x0F, 0x14, 0x0E, 0x05, 0x1E, 0x04],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '&' => [0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D],
        '\'' => [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '*' => [0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x04, 0x04, 0x08],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x06, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        ';' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x04, 0x08],
        '<' | '◄' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '>' | '►' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        '@' => [0x0E, 0x11, 0x17, 0x15, 0x17, 0x10, 0x0E],
        'A' | 'a' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' | 'b' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' | 'c' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' | 'd' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' | 'e' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' | 'f' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' | 'g' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' | 'h' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' | 'i' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' | 'j' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' | 'k' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' | 'l' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' | 'm' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' | 'n' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' | 'o' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' | 'p' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' | 'q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' | 'r' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' | 's' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' | 't' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' | 'u' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' | 'v' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' | 'w' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' | 'x' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' | 'y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' | 'z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        '\\' => [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '^' | '▲' => [0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '`' => [0x08, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
        '{' => [0x02, 0x04, 0x04, 0x08, 0x04, 0x04, 0x02],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '}' => [0x08, 0x04, 0x04, 0x02, 0x04, 0x04, 0x08],
        '~' => [0x00, 0x08, 0x15, 0x02, 0x00, 0x00, 0x00],
        '↕' => [0x04, 0x0E, 0x15, 0x04, 0x15, 0x0E, 0x04],
        '▼' => [0x00, 0x00, 0x1F, 0x0E, 0x04, 0x00, 0x00],
        _ => [0x0E, 0x11, 0x15, 0x11, 0x15, 0x11, 0x0E],
    }
}
