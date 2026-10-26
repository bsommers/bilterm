use crate::pane::typography::FontFamilyPreset;

/// Returns `true` for box-drawing, block/shade, and Braille characters that should be drawn
/// geometrically from cell edge to cell edge so window frames and plots never have gaps.
pub fn is_geometric_glyph(ch: char) -> bool {
    matches!(
        ch,
        '\u{2500}'..='\u{257F}'
            | '\u{2580}'..='\u{259F}'
            | '\u{2800}'..='\u{28FF}'
            | '▪'
            | '◢'
            | '►'
            | '◄'
            | '▲'
            | '▼'
    )
}

/// Resolves the ordered list of candidate OS TrueType font family names for `family_str`.
pub fn resolve_candidate_font_faces(family_str: &str) -> Vec<String> {
    if let Some(preset) = FontFamilyPreset::from_name(family_str) {
        preset
            .candidate_os_font_faces()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
    } else {
        let trimmed = family_str.trim();
        let mut out = Vec::new();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
        for fb in [
            "Cascadia Mono",
            "Cascadia Code",
            "Consolas",
            "JetBrains Mono",
            "Lucida Console",
            "Courier New",
        ] {
            if !out.iter().any(|s| s.eq_ignore_ascii_case(fb)) {
                out.push(fb.to_string());
            }
        }
        out
    }
}

/// Draws a geometric box-drawing, block/shade, or Braille glyph directly into a 32-bit `0xFFRRGGBB`
/// pixel buffer at `(x0, y0)` with cell dimensions `(cw, ch)` and proportional stroke width.
#[allow(clippy::too_many_arguments)]
pub fn draw_geometric_cell_bgra(
    pixels: &mut [u32],
    fb_w: u32,
    fb_h: u32,
    x0: u32,
    y0: u32,
    cw: u32,
    ch: u32,
    glyph: char,
    fg_rgba: [u8; 4],
) {
    if cw == 0 || ch == 0 || x0 >= fb_w || y0 >= fb_h {
        return;
    }
    let color = 0xFF00_0000
        | (u32::from(fg_rgba[0]) << 16)
        | (u32::from(fg_rgba[1]) << 8)
        | u32::from(fg_rgba[2]);

    let put = |buf: &mut [u32], px: u32, py: u32| {
        if px < fb_w && py < fb_h {
            buf[(py * fb_w + px) as usize] = color;
        }
    };
    let fill_rect = |buf: &mut [u32], rx0: u32, ry0: u32, rx1: u32, ry1: u32| {
        let x_end = rx1.min(cw);
        let y_end = ry1.min(ch);
        for dy in ry0.min(y_end)..y_end {
            let py = y0 + dy;
            if py >= fb_h {
                break;
            }
            for dx in rx0.min(x_end)..x_end {
                let px = x0 + dx;
                if px < fb_w {
                    buf[(py * fb_w + px) as usize] = color;
                }
            }
        }
    };

    let stroke = (cw / 9).clamp(1, 3);
    let mx = cw / 2;
    let my = ch / 2;
    let gap = stroke.max(2);
    let lx = mx.saturating_sub(gap);
    let rx = (mx + gap).min(cw.saturating_sub(stroke));
    let ty = my.saturating_sub(gap);
    let by = (my + gap).min(ch.saturating_sub(stroke));

    let h_seg = |buf: &mut [u32], sx0: u32, sx1: u32, sy: u32| {
        fill_rect(buf, sx0, sy, sx1, sy + stroke);
    };
    let v_seg = |buf: &mut [u32], sx: u32, sy0: u32, sy1: u32| {
        fill_rect(buf, sx, sy0, sx + stroke, sy1);
    };

    match glyph {
        '─' | '━' | '╌' | '╍' | '┄' | '┅' => h_seg(pixels, 0, cw, my),
        '│' | '┃' | '╎' | '╏' | '┆' | '┇' => v_seg(pixels, mx, 0, ch),
        '┌' | '╭' | '┏' => {
            h_seg(pixels, mx, cw, my);
            v_seg(pixels, mx, my, ch);
        }
        '┐' | '╮' | '┓' => {
            h_seg(pixels, 0, mx + stroke, my);
            v_seg(pixels, mx, my, ch);
        }
        '└' | '╰' | '┗' => {
            h_seg(pixels, mx, cw, my);
            v_seg(pixels, mx, 0, my + stroke);
        }
        '┘' | '╯' | '┛' => {
            h_seg(pixels, 0, mx + stroke, my);
            v_seg(pixels, mx, 0, my + stroke);
        }
        '├' | '┣' => {
            v_seg(pixels, mx, 0, ch);
            h_seg(pixels, mx, cw, my);
        }
        '┤' | '┫' => {
            v_seg(pixels, mx, 0, ch);
            h_seg(pixels, 0, mx + stroke, my);
        }
        '┬' | '┳' => {
            h_seg(pixels, 0, cw, my);
            v_seg(pixels, mx, my, ch);
        }
        '┴' | '┻' => {
            h_seg(pixels, 0, cw, my);
            v_seg(pixels, mx, 0, my + stroke);
        }
        '┼' | '╋' => {
            h_seg(pixels, 0, cw, my);
            v_seg(pixels, mx, 0, ch);
        }
        '═' => {
            h_seg(pixels, 0, cw, ty);
            h_seg(pixels, 0, cw, by);
        }
        '║' => {
            v_seg(pixels, lx, 0, ch);
            v_seg(pixels, rx, 0, ch);
        }
        '╔' => {
            h_seg(pixels, lx, cw, ty);
            h_seg(pixels, rx, cw, by);
            v_seg(pixels, lx, ty, ch);
            v_seg(pixels, rx, by, ch);
        }
        '╗' => {
            h_seg(pixels, 0, rx + stroke, ty);
            h_seg(pixels, 0, lx + stroke, by);
            v_seg(pixels, lx, by, ch);
            v_seg(pixels, rx, ty, ch);
        }
        '╚' => {
            h_seg(pixels, rx, cw, ty);
            h_seg(pixels, lx, cw, by);
            v_seg(pixels, lx, 0, by + stroke);
            v_seg(pixels, rx, 0, ty + stroke);
        }
        '╝' => {
            h_seg(pixels, 0, lx + stroke, ty);
            h_seg(pixels, 0, rx + stroke, by);
            v_seg(pixels, lx, 0, ty + stroke);
            v_seg(pixels, rx, 0, by + stroke);
        }
        '╠' | '╟' | '╞' => {
            v_seg(pixels, lx, 0, ch);
            v_seg(pixels, rx, 0, ty + stroke);
            v_seg(pixels, rx, by, ch);
            h_seg(pixels, rx, cw, ty);
            h_seg(pixels, rx, cw, by);
        }
        '╣' | '╢' | '╡' => {
            v_seg(pixels, rx, 0, ch);
            v_seg(pixels, lx, 0, ty + stroke);
            v_seg(pixels, lx, by, ch);
            h_seg(pixels, 0, lx + stroke, ty);
            h_seg(pixels, 0, lx + stroke, by);
        }
        '╦' | '╤' | '╥' => {
            h_seg(pixels, 0, cw, ty);
            h_seg(pixels, 0, lx + stroke, by);
            h_seg(pixels, rx, cw, by);
            v_seg(pixels, lx, by, ch);
            v_seg(pixels, rx, by, ch);
        }
        '╩' | '╧' | '╨' => {
            h_seg(pixels, 0, cw, by);
            h_seg(pixels, 0, lx + stroke, ty);
            h_seg(pixels, rx, cw, ty);
            v_seg(pixels, lx, 0, ty + stroke);
            v_seg(pixels, rx, 0, ty + stroke);
        }
        '╬' | '╪' | '╫' => {
            h_seg(pixels, 0, lx + stroke, ty);
            h_seg(pixels, rx, cw, ty);
            h_seg(pixels, 0, lx + stroke, by);
            h_seg(pixels, rx, cw, by);
            v_seg(pixels, lx, 0, ty + stroke);
            v_seg(pixels, lx, by, ch);
            v_seg(pixels, rx, 0, ty + stroke);
            v_seg(pixels, rx, by, ch);
        }
        '█' => fill_rect(pixels, 0, 0, cw, ch),
        '▀' => fill_rect(pixels, 0, 0, cw, my),
        '▄' => fill_rect(pixels, 0, my, cw, ch),
        '▌' => fill_rect(pixels, 0, 0, mx, ch),
        '▐' => fill_rect(pixels, mx, 0, cw, ch),
        '▁'..='▇' => {
            let level = (glyph as u32 - '▁' as u32 + 1).min(8);
            let top = ch.saturating_sub((ch * level) / 8);
            fill_rect(pixels, 0, top, cw, ch);
        }
        '░' => {
            for dy in 0..ch {
                for dx in 0..cw {
                    if (dx + dy) % 4 == 0 {
                        put(pixels, x0 + dx, y0 + dy);
                    }
                }
            }
        }
        '▒' => {
            for dy in 0..ch {
                for dx in 0..cw {
                    if (dx + dy) % 2 == 0 {
                        put(pixels, x0 + dx, y0 + dy);
                    }
                }
            }
        }
        '▓' => {
            for dy in 0..ch {
                for dx in 0..cw {
                    if (dx + dy) % 4 != 0 {
                        put(pixels, x0 + dx, y0 + dy);
                    }
                }
            }
        }
        '▪' | '■' => fill_rect(pixels, cw / 4, ch / 3, (cw * 3) / 4, (ch * 2) / 3),
        '◢' => {
            for dy in 0..ch {
                let start_x = cw.saturating_sub(((dy + 1) * cw) / ch.max(1));
                fill_rect(pixels, start_x, dy, cw, dy + 1);
            }
        }
        '►' | '▶' => {
            let h_span = (ch / 2).max(4);
            let y_top = (ch.saturating_sub(h_span)) / 2;
            for i in 0..h_span {
                let dist = if i < h_span / 2 { i } else { h_span - 1 - i };
                let w = ((dist + 1) * (cw / 2).max(2)) / (h_span / 2).max(1);
                fill_rect(pixels, cw / 4, y_top + i, (cw / 4 + w).min(cw), y_top + i + 1);
            }
        }
        '◄' | '◀' => {
            let h_span = (ch / 2).max(4);
            let y_top = (ch.saturating_sub(h_span)) / 2;
            for i in 0..h_span {
                let dist = if i < h_span / 2 { i } else { h_span - 1 - i };
                let w = ((dist + 1) * (cw / 2).max(2)) / (h_span / 2).max(1);
                let x_end = (cw * 3) / 4;
                fill_rect(pixels, x_end.saturating_sub(w), y_top + i, x_end, y_top + i + 1);
            }
        }
        '▲' => {
            let rows = (ch / 3).max(3);
            let y_top = (ch.saturating_sub(rows)) / 2;
            for r in 0..rows {
                let half = ((r + 1) * cw) / (rows * 2 + 2);
                fill_rect(pixels, mx.saturating_sub(half), y_top + r, (mx + half + 1).min(cw), y_top + r + 1);
            }
        }
        '▼' => {
            let rows = (ch / 3).max(3);
            let y_top = (ch.saturating_sub(rows)) / 2;
            for r in 0..rows {
                let inv = rows - 1 - r;
                let half = ((inv + 1) * cw) / (rows * 2 + 2);
                fill_rect(pixels, mx.saturating_sub(half), y_top + r, (mx + half + 1).min(cw), y_top + r + 1);
            }
        }
        '\u{2800}'..='\u{28FF}' => {
            let mask = (glyph as u32 - 0x2800) as u8;
            let dot_bits: [(u8, u32, u32); 8] = [
                (0x01, 0, 0),
                (0x02, 0, 1),
                (0x04, 0, 2),
                (0x08, 1, 0),
                (0x10, 1, 1),
                (0x20, 1, 2),
                (0x40, 0, 3),
                (0x80, 1, 3),
            ];
            let dw = (cw / 4).clamp(1, 3);
            let dh = (ch / 8).clamp(1, 3);
            for &(bit, dc, dr) in &dot_bits {
                if mask & bit != 0 {
                    let dx = (dc * cw) / 2 + cw / 6;
                    let dy = (dr * ch) / 4 + ch / 10;
                    fill_rect(pixels, dx, dy, dx + dw, dy + dh);
                }
            }
        }
        _ => {}
    }
}
