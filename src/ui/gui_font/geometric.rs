use crate::pane::typography::FontFamilyPreset;
use crate::ui::glyphs::GlyphPack;

/// Returns `true` for box-drawing, block/shade, Braille, Powerline (`U+E0A0..=U+E0D7`), and
/// common CLI/TUI agent markers (`❯`, `⏺`, `⎿`, `✻`, `✢`, `✓`, `✗`, `●`, `○`, `◆`, `▸`, arrows)
/// that should be drawn geometrically so window frames, statuslines, and AI CLI prompts (`claude`,
/// `agy`, `gemini`) never render missing-glyph boxes (`▯`) when a monospace font lacks Dingbats.
pub fn is_geometric_glyph(ch: char) -> bool {
    matches!(
        ch,
        '\u{2500}'..='\u{257F}'
            | '\u{2580}'..='\u{259F}'
            | '\u{2800}'..='\u{28FF}'
            | '\u{E0A0}'..='\u{E0D7}'
            | '▪' | '▫' | '■' | '□'
            | '◢'
            | '►' | '▶' | '▸' | '▹' | '‣'
            | '◄' | '◀' | '◂' | '◃'
            | '▲' | '▴' | '▼' | '▾'
            | '❯' | '❮' | '›' | '‹' | '»' | '«'
            | '⏺' | '●' | '○' | '◉' | '◎' | '•' | '◦' | '·'
            | '◆' | '◇'
            | '⎿'
            | '✓' | '✔' | '✗' | '✘'
            | '✢' | '✣' | '✤' | '✥' | '✦' | '✧'
            | '✳' | '✶' | '✷' | '✸' | '✹' | '✺' | '✻' | '✼' | '✽'
            | '←' | '↑' | '→' | '↓' | '↔' | '↕' | '↳' | '↪' | '↵' | '➜' | '➔'
    )
}

/// Returns `true` if `ch` is in a Unicode Private Use Area (`U+E000..=U+F8FF` or `U+F0000..=U+10FFFF`).
pub fn is_private_use_area(ch: char) -> bool {
    matches!(ch, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{10FFFF}')
}

/// Maps Nerd Font / Powerline Private Use Area icons to either a geometric glyph (`\u{E0A0}..=\u{E0D7}`)
/// or a standard Unicode symbol present in system fonts (`Menlo`, `SF Mono`, `Apple Symbols`, `Consolas`,
/// `DejaVu Sans Mono`) when no Nerd Font is installed.
pub fn map_pua_icon_fallback(ch: char) -> char {
    match ch {
        '\u{E0A0}'..='\u{E0D7}' => ch,
        // Git branch / merge icons (Devicons, FontAwesome, Octicons, Material)
        '\u{E702}' | '\u{E725}' | '\u{E726}' | '\u{E727}' | '\u{E728}' | '\u{F126}'
        | '\u{F1D3}' | '\u{F1D2}' | '\u{F418}' | '\u{F062C}' => '\u{E0A0}',
        // Lock / read-only icons
        '\u{F023}' | '\u{F084}' | '\u{F13E}' | '\u{F456}' | '\u{F033E}' | '\u{F0341}' => '\u{E0A2}',
        // Apple / macOS icons
        '\u{F179}' | '\u{F302}' | '\u{E711}' | '\u{F0035}' => '⌘',
        // Linux / Tux / BSD / distro icons
        '\u{F17C}' | '\u{E712}' | '\u{F300}'..='\u{F32F}' => '◆',
        // Windows icons
        '\u{F17A}' | '\u{E70F}' | '\u{F05B3}' => '⊞',
        // Home directory icons
        '\u{F015}' | '\u{F46D}' | '\u{F02DC}' => '~',
        // Folder / directory icons
        '\u{E5FE}' | '\u{E5FF}' | '\u{F07B}' | '\u{F07C}' | '\u{F114}' | '\u{F115}'
        | '\u{F413}' | '\u{F4D3}' | '\u{F024B}' | '\u{F0256}' => '▸',
        // Check / success icons
        '\u{F00C}' | '\u{F058}' | '\u{F05D}' | '\u{F42E}' | '\u{F012C}' => '✓',
        // Cross / failure / error icons
        '\u{F00D}' | '\u{F057}' | '\u{F05C}' | '\u{F467}' | '\u{F0156}' => '✗',
        // Warning / lightning / alert icons
        '\u{F071}' | '\u{F06A}' | '\u{F0E7}' | '\u{F421}' => '!',
        // Clock / duration / history icons
        '\u{F017}' | '\u{F43A}' | '\u{F51F}' | '\u{F0150}' => '◷',
        // Gear / settings / jobs icons
        '\u{F013}' | '\u{F085}' | '\u{F423}' => '⚙',
        // Prompt right chevrons
        '\u{F054}' | '\u{F105}' | '\u{F101}' | '\u{F460}' => '❯',
        // Prompt left chevrons
        '\u{F053}' | '\u{F104}' | '\u{F100}' | '\u{F45F}' => '❮',
        // Terminal / shell icons
        '\u{F120}' | '\u{F489}' | '\u{E795}' => '$',
        // Star / dirty status icons
        '\u{F005}' | '\u{F006}' | '\u{F41E}' => '★',
        _ => '•',
    }
}

/// Maps Powerline and Nerd Font PUA codepoints to standard Unicode strings for TUI host terminals
/// when `GlyphPack::NerdFont` is not enabled.
pub fn map_tui_pua_fallback(ch: char) -> &'static str {
    match ch {
        '\u{E0A0}' => "⎇",
        '\u{E0A1}' => "ℓ",
        '\u{E0A2}' => "*",
        '\u{E0A3}' => "⋮",
        '\u{E0B0}' | '\u{E0C0}' | '\u{E0C4}' | '\u{E0C6}' | '\u{E0C8}' | '\u{E0CC}'
        | '\u{E0CE}' | '\u{E0D2}' => "▶",
        '\u{E0B1}' | '\u{E0C1}' | '\u{E0C5}' | '\u{E0CD}' => "❯",
        '\u{E0B2}' | '\u{E0C2}' | '\u{E0C7}' | '\u{E0CA}' | '\u{E0CF}' | '\u{E0D4}' => "◀",
        '\u{E0B3}' | '\u{E0C3}' | '\u{E0CB}' => "❮",
        '\u{E0B4}' => "◗",
        '\u{E0B5}' => ")",
        '\u{E0B6}' => "◖",
        '\u{E0B7}' => "(",
        '\u{E0B8}' | '\u{E0BC}' => "◣",
        '\u{E0B9}' | '\u{E0BF}' => "╲",
        '\u{E0BA}' | '\u{E0BE}' => "◢",
        '\u{E0BB}' | '\u{E0BD}' => "╱",
        '\u{E0C9}'..='\u{E0D7}' => "▶",
        other => match map_pua_icon_fallback(other) {
            '\u{E0A0}' => "⎇",
            '\u{E0A2}' => "*",
            '⌘' => "⌘",
            '◆' => "◆",
            '⊞' => "⊞",
            '~' => "~",
            '▸' => "▸",
            '✓' => "✓",
            '✗' => "✗",
            '!' => "!",
            '◷' => "◷",
            '⚙' => "⚙",
            '❯' => "❯",
            '❮' => "❮",
            '$' => "$",
            '★' => "★",
            _ => "•",
        },
    }
}

/// Replaces Private Use Area characters in a TUI `ratatui::buffer::Buffer` when `glyphs` is not
/// `GlyphPack::NerdFont` so host terminals without Nerd Fonts never show `[?]` LastResort boxes.
pub fn sanitize_tui_pua_buffer(buf: &mut ratatui::buffer::Buffer, glyphs: GlyphPack) {
    if glyphs == GlyphPack::NerdFont {
        return;
    }
    let area = buf.area;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let sym = cell.symbol();
                if let Some(ch) = sym.chars().next()
                    && is_private_use_area(ch)
                {
                    cell.set_symbol(map_tui_pua_fallback(ch));
                }
            }
        }
    }
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
            "MesloLGS NF",
            "JetBrains Mono",
            "SF Mono",
            "Menlo",
            "Consolas",
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
    let blend_px = |buf: &mut [u32], px: u32, py: u32, cov: f32| {
        if px >= fb_w || py >= fb_h || cov <= 0.01 {
            return;
        }
        let idx = (py * fb_w + px) as usize;
        if cov >= 0.99 {
            buf[idx] = color;
            return;
        }
        let a = (cov * 256.0).clamp(0.0, 256.0) as u32;
        let inv = 256 - a;
        let dst = buf[idx];
        let dr = (dst >> 16) & 0xFF;
        let dg = (dst >> 8) & 0xFF;
        let db = dst & 0xFF;
        let r = (u32::from(fg_rgba[0]) * a + dr * inv) >> 8;
        let g = (u32::from(fg_rgba[1]) * a + dg * inv) >> 8;
        let b = (u32::from(fg_rgba[2]) * a + db * inv) >> 8;
        buf[idx] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
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
        '┌' | '╭' | '┏' => { h_seg(pixels, mx, cw, my); v_seg(pixels, mx, my, ch); }
        '┐' | '╮' | '┓' => { h_seg(pixels, 0, mx + stroke, my); v_seg(pixels, mx, my, ch); }
        '└' | '╰' | '┗' => { h_seg(pixels, mx, cw, my); v_seg(pixels, mx, 0, my + stroke); }
        '┘' | '╯' | '┛' => { h_seg(pixels, 0, mx + stroke, my); v_seg(pixels, mx, 0, my + stroke); }
        '├' | '┣' => { v_seg(pixels, mx, 0, ch); h_seg(pixels, mx, cw, my); }
        '┤' | '┫' => { v_seg(pixels, mx, 0, ch); h_seg(pixels, 0, mx + stroke, my); }
        '┬' | '┳' => { h_seg(pixels, 0, cw, my); v_seg(pixels, mx, my, ch); }
        '┴' | '┻' => { h_seg(pixels, 0, cw, my); v_seg(pixels, mx, 0, my + stroke); }
        '┼' | '╋' => { h_seg(pixels, 0, cw, my); v_seg(pixels, mx, 0, ch); }
        '═' => { h_seg(pixels, 0, cw, ty); h_seg(pixels, 0, cw, by); }
        '║' => { v_seg(pixels, lx, 0, ch); v_seg(pixels, rx, 0, ch); }
        '╔' => { h_seg(pixels, lx, cw, ty); h_seg(pixels, rx, cw, by); v_seg(pixels, lx, ty, ch); v_seg(pixels, rx, by, ch); }
        '╗' => { h_seg(pixels, 0, rx + stroke, ty); h_seg(pixels, 0, lx + stroke, by); v_seg(pixels, lx, by, ch); v_seg(pixels, rx, ty, ch); }
        '╚' => { h_seg(pixels, rx, cw, ty); h_seg(pixels, lx, cw, by); v_seg(pixels, lx, 0, by + stroke); v_seg(pixels, rx, 0, ty + stroke); }
        '╝' => { h_seg(pixels, 0, lx + stroke, ty); h_seg(pixels, 0, rx + stroke, by); v_seg(pixels, lx, 0, ty + stroke); v_seg(pixels, rx, 0, by + stroke); }
        '╠' | '╟' | '╞' => {
            v_seg(pixels, lx, 0, ch); v_seg(pixels, rx, 0, ty + stroke); v_seg(pixels, rx, by, ch);
            h_seg(pixels, rx, cw, ty); h_seg(pixels, rx, cw, by);
        }
        '╣' | '╢' | '╡' => {
            v_seg(pixels, rx, 0, ch); v_seg(pixels, lx, 0, ty + stroke); v_seg(pixels, lx, by, ch);
            h_seg(pixels, 0, lx + stroke, ty); h_seg(pixels, 0, lx + stroke, by);
        }
        '╦' | '╤' | '╥' => {
            h_seg(pixels, 0, cw, ty); h_seg(pixels, 0, lx + stroke, by); h_seg(pixels, rx, cw, by);
            v_seg(pixels, lx, by, ch); v_seg(pixels, rx, by, ch);
        }
        '╩' | '╧' | '╨' => {
            h_seg(pixels, 0, cw, by); h_seg(pixels, 0, lx + stroke, ty); h_seg(pixels, rx, cw, ty);
            v_seg(pixels, lx, 0, ty + stroke); v_seg(pixels, rx, 0, ty + stroke);
        }
        '╬' | '╪' | '╫' => {
            h_seg(pixels, 0, lx + stroke, ty); h_seg(pixels, rx, cw, ty);
            h_seg(pixels, 0, lx + stroke, by); h_seg(pixels, rx, cw, by);
            v_seg(pixels, lx, 0, ty + stroke); v_seg(pixels, lx, by, ch);
            v_seg(pixels, rx, 0, ty + stroke); v_seg(pixels, rx, by, ch);
        }
        '█' => fill_rect(pixels, 0, 0, cw, ch),
        '▀' => fill_rect(pixels, 0, 0, cw, my),
        '▄' => fill_rect(pixels, 0, my, cw, ch),
        '▌' => fill_rect(pixels, 0, 0, mx, ch),
        '▐' => fill_rect(pixels, mx, 0, cw, ch),
        '▁'..='▇' => {
            let level = (glyph as u32 - '▁' as u32 + 1).min(8);
            fill_rect(pixels, 0, ch.saturating_sub((ch * level) / 8), cw, ch);
        }
        '░' | '▒' | '▓' => {
            for dy in 0..ch {
                for dx in 0..cw {
                    let draw = match glyph {
                        '░' => (dx + dy) % 4 == 0,
                        '▒' => (dx + dy) % 2 == 0,
                        _ => (dx + dy) % 4 != 0,
                    };
                    if draw { put(pixels, x0 + dx, y0 + dy); }
                }
            }
        }
        '▪' | '■' => fill_rect(pixels, cw / 4, ch / 3, (cw * 3) / 4, (ch * 2) / 3),
        '▫' | '□' => {
            let (x1, y1, x2, y2) = (cw / 4, ch / 3, (cw * 3) / 4, (ch * 2) / 3);
            h_seg(pixels, x1, x2, y1);
            h_seg(pixels, x1, x2, y2.saturating_sub(stroke));
            v_seg(pixels, x1, y1, y2);
            v_seg(pixels, x2.saturating_sub(stroke), y1, y2);
        }
        '◢' => {
            for dy in 0..ch {
                let start_x = cw.saturating_sub(((dy + 1) * cw) / ch.max(1));
                fill_rect(pixels, start_x, dy, cw, dy + 1);
            }
        }
        '►' | '▶' | '▸' | '▹' | '‣' => {
            let h_span = (ch / 2).max(4);
            let y_top = (ch.saturating_sub(h_span)) / 2;
            for i in 0..h_span {
                let dist = if i < h_span / 2 { i } else { h_span - 1 - i };
                let w = ((dist + 1) * (cw / 2).max(2)) / (h_span / 2).max(1);
                fill_rect(pixels, cw / 4, y_top + i, (cw / 4 + w).min(cw), y_top + i + 1);
            }
        }
        '◄' | '◀' | '◂' | '◃' => {
            let h_span = (ch / 2).max(4);
            let y_top = (ch.saturating_sub(h_span)) / 2;
            for i in 0..h_span {
                let dist = if i < h_span / 2 { i } else { h_span - 1 - i };
                let w = ((dist + 1) * (cw / 2).max(2)) / (h_span / 2).max(1);
                let x_end = (cw * 3) / 4;
                fill_rect(pixels, x_end.saturating_sub(w), y_top + i, x_end, y_top + i + 1);
            }
        }
        '▲' | '▴' => {
            let rows = (ch / 3).max(3);
            let y_top = (ch.saturating_sub(rows)) / 2;
            for r in 0..rows {
                let half = ((r + 1) * cw) / (rows * 2 + 2);
                fill_rect(pixels, mx.saturating_sub(half), y_top + r, (mx + half + 1).min(cw), y_top + r + 1);
            }
        }
        '▼' | '▾' => {
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
        // Powerline solid right triangle `` (U+E0B0) & flame/pixelated/trapezoid right separators
        '\u{E0B0}' | '\u{E0C0}' | '\u{E0C4}' | '\u{E0C6}' | '\u{E0C8}' | '\u{E0CC}'
        | '\u{E0CE}' | '\u{E0D2}' => {
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            for dy in 0..ch {
                let dist = ((dy as f32) - cy).abs() / half;
                let x_limit = (1.0 - dist).max(0.0) * (cw as f32);
                for dx in 0..cw {
                    let cov = (x_limit - (dx as f32)).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline thin right chevron `` (U+E0B1) and prompt/list chevrons `❯` (U+276F), `›`, `»`
        '\u{E0B1}' | '\u{E0C1}' | '\u{E0C5}' | '\u{E0CD}' | '❯' | '›' | '»' => {
            let is_prompt_chev = matches!(glyph, '❯' | '›' | '»');
            let y_pad = if is_prompt_chev { (ch as f32) * 0.22 } else { 0.0 };
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = (cy - y_pad).max(2.0);
            let sw = if glyph == '❯' {
                (stroke as f32 * 1.35).max(1.8)
            } else {
                (stroke as f32).max(1.4)
            };
            let x_min = if is_prompt_chev { (cw as f32) * 0.20 } else { 0.0 };
            let span = if is_prompt_chev {
                ((cw as f32) * 0.55).max(2.0)
            } else {
                (cw.saturating_sub(1) as f32).max(1.0)
            };
            for dy in 0..ch {
                let vy = ((dy as f32) - cy).abs();
                if is_prompt_chev && vy > half + 1.0 {
                    continue;
                }
                let dist = (vy / half).clamp(0.0, 1.0);
                let xc = x_min + (1.0 - dist) * span;
                for dx in 0..cw {
                    let d = ((dx as f32) - xc).abs();
                    let cov = (sw - d).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                    if glyph == '»' {
                        let d2 = ((dx as f32) - (xc - span * 0.45)).abs();
                        let cov2 = (sw - d2).clamp(0.0, 1.0);
                        if cov2 > 0.0 {
                            blend_px(pixels, x0 + dx, y0 + dy, cov2);
                        }
                    }
                }
            }
        }
        // Powerline solid left triangle `` (U+E0B2) & left flame/pixelated/trapezoid separators
        '\u{E0B2}' | '\u{E0C2}' | '\u{E0C7}' | '\u{E0CA}' | '\u{E0CF}' | '\u{E0D4}' => {
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            for dy in 0..ch {
                let dist = ((dy as f32) - cy).abs() / half;
                let x_start = dist * (cw as f32);
                for dx in 0..cw {
                    let cov = ((dx as f32 + 1.0) - x_start).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline thin left chevron `` (U+E0B3) and left chevrons `❮` (U+276E), `‹`, `«`
        '\u{E0B3}' | '\u{E0C3}' | '\u{E0CB}' | '❮' | '‹' | '«' => {
            let is_prompt_chev = matches!(glyph, '❮' | '‹' | '«');
            let y_pad = if is_prompt_chev { (ch as f32) * 0.22 } else { 0.0 };
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = (cy - y_pad).max(2.0);
            let sw = if glyph == '❮' {
                (stroke as f32 * 1.35).max(1.8)
            } else {
                (stroke as f32).max(1.4)
            };
            let x_min = if is_prompt_chev { (cw as f32) * 0.20 } else { 0.0 };
            let span = if is_prompt_chev {
                ((cw as f32) * 0.55).max(2.0)
            } else {
                (cw.saturating_sub(1) as f32).max(1.0)
            };
            for dy in 0..ch {
                let vy = ((dy as f32) - cy).abs();
                if is_prompt_chev && vy > half + 1.0 {
                    continue;
                }
                let dist = (vy / half).clamp(0.0, 1.0);
                let xc = x_min + dist * span;
                for dx in 0..cw {
                    let d = ((dx as f32) - xc).abs();
                    let cov = (sw - d).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                    if glyph == '«' {
                        let d2 = ((dx as f32) - (xc + span * 0.45)).abs();
                        let cov2 = (sw - d2).clamp(0.0, 1.0);
                        if cov2 > 0.0 {
                            blend_px(pixels, x0 + dx, y0 + dy, cov2);
                        }
                    }
                }
            }
        }
        // Powerline solid right rounded pill cap `` (U+E0B4)
        '\u{E0B4}' => {
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            for dy in 0..ch {
                let ny = ((dy as f32) - cy) / half;
                let x_limit = (1.0 - ny * ny).max(0.0).sqrt() * (cw as f32);
                for dx in 0..cw {
                    let cov = (x_limit - (dx as f32)).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline thin right rounded arc `` (U+E0B5)
        '\u{E0B5}' => {
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            let sw = (stroke as f32).max(1.4);
            let span = (cw.saturating_sub(1) as f32).max(1.0);
            for dy in 0..ch {
                let ny = ((dy as f32) - cy) / half;
                let xc = (1.0 - ny * ny).max(0.0).sqrt() * span;
                for dx in 0..cw {
                    let cov = (sw - ((dx as f32) - xc).abs()).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline solid left rounded pill cap `` (U+E0B6)
        '\u{E0B6}' => {
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            for dy in 0..ch {
                let ny = ((dy as f32) - cy) / half;
                let x_start = (cw as f32) * (1.0 - (1.0 - ny * ny).max(0.0).sqrt());
                for dx in 0..cw {
                    let cov = ((dx as f32 + 1.0) - x_start).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline thin left rounded arc `` (U+E0B7)
        '\u{E0B7}' => {
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            let sw = (stroke as f32).max(1.4);
            let span = (cw.saturating_sub(1) as f32).max(1.0);
            for dy in 0..ch {
                let ny = ((dy as f32) - cy) / half;
                let xc = span * (1.0 - (1.0 - ny * ny).max(0.0).sqrt());
                for dx in 0..cw {
                    let cov = (sw - ((dx as f32) - xc).abs()).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline lower-left solid triangle `` (U+E0B8)
        '\u{E0B8}' => {
            for dy in 0..ch {
                let x_limit = ((dy as f32 + 1.0) * (cw as f32)) / (ch.max(1) as f32);
                for dx in 0..cw {
                    let cov = (x_limit - (dx as f32)).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline lower-right solid triangle `` (U+E0BA)
        '\u{E0BA}' => {
            for dy in 0..ch {
                let x_start = (cw as f32) - ((dy as f32 + 1.0) * (cw as f32)) / (ch.max(1) as f32);
                for dx in 0..cw {
                    let cov = ((dx as f32 + 1.0) - x_start).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline upper-left solid triangle `` (U+E0BC)
        '\u{E0BC}' => {
            for dy in 0..ch {
                let x_limit = ((ch - dy) as f32 * (cw as f32)) / (ch.max(1) as f32);
                for dx in 0..cw {
                    let cov = (x_limit - (dx as f32)).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline upper-right solid triangle `` (U+E0BE)
        '\u{E0BE}' => {
            for dy in 0..ch {
                let x_start = ((dy as f32) * (cw as f32)) / (ch.max(1) as f32);
                for dx in 0..cw {
                    let cov = ((dx as f32 + 1.0) - x_start).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline diagonal slashes `` (U+E0B9), `` (U+E0BB), `` (U+E0BD), `` (U+E0BF)
        '\u{E0B9}' | '\u{E0BF}' | '\u{E0BB}' | '\u{E0BD}' => {
            let forward = matches!(glyph, '\u{E0BB}' | '\u{E0BD}');
            let sw = (stroke as f32).max(1.4);
            let span = (cw.saturating_sub(1) as f32).max(1.0);
            for dy in 0..ch {
                let t = (dy as f32) / (ch.saturating_sub(1).max(1) as f32);
                let xc = if forward { (1.0 - t) * span } else { t * span };
                for dx in 0..cw {
                    let cov = (sw - ((dx as f32) - xc).abs()).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline Git branch icon `` (U+E0A0)
        '\u{E0A0}' => {
            let sx = (cw / 4).max(1);
            let bx = ((cw * 3) / 4).min(cw.saturating_sub(stroke + 1));
            let top_y = (ch / 5).max(1);
            let bot_y = ((ch * 4) / 5).min(ch.saturating_sub(stroke + 1));
            // Left vertical stem + top/bottom nodes
            v_seg(pixels, sx, top_y, bot_y + stroke);
            fill_rect(pixels, sx.saturating_sub(1), top_y.saturating_sub(1), sx + stroke + 1, top_y + stroke + 1);
            fill_rect(pixels, sx.saturating_sub(1), bot_y.saturating_sub(1), sx + stroke + 1, bot_y + stroke + 1);
            // Right branch node + connector
            fill_rect(pixels, bx.saturating_sub(1), top_y.saturating_sub(1), bx + stroke + 1, top_y + stroke + 1);
            v_seg(pixels, bx, top_y, my + stroke);
            h_seg(pixels, sx, bx + stroke, my);
        }
        // Powerline Padlock icon `` (U+E0A2)
        '\u{E0A2}' => {
            let bx0 = (cw / 5).max(1);
            let bx1 = ((cw * 4) / 5).min(cw.saturating_sub(1));
            let by0 = my.saturating_sub(1);
            let by1 = ((ch * 4) / 5).min(ch.saturating_sub(1));
            fill_rect(pixels, bx0, by0, bx1, by1);
            let sx0 = (cw / 3).max(bx0);
            let sx1 = ((cw * 2) / 3).min(bx1).max(sx0 + stroke);
            let sy0 = (ch / 5).max(1);
            v_seg(pixels, sx0, sy0, by0);
            v_seg(pixels, sx1.saturating_sub(stroke), sy0, by0);
            h_seg(pixels, sx0, sx1, sy0);
        }
        // Powerline Line/Column icons `` (U+E0A1), `` (U+E0A3), and remaining E0C0..E0D7
        '\u{E0A1}' | '\u{E0A3}' | '\u{E0C0}'..='\u{E0D7}' => {
            v_seg(pixels, lx, ch / 4, (ch * 3) / 4);
            h_seg(pixels, lx, rx + stroke, (ch * 3) / 4);
        }
        // Claude Code / CLI agent markers: `⏺`, `●`, `○`, `◉`, `◎`, `•`, `◦`, `·`, `◆`, `◇`, `⎿`, `✓`, `✗`, spinners, arrows
        '⏺' | '●' | '○' | '◉' | '◎' | '•' | '◦' | '·' | '◆' | '◇' => {
            let (cx, cy) = (cw as f32 * 0.5, ch as f32 * 0.5);
            let rad = match glyph {
                '·' => (cw.min(ch) as f32 * 0.14).max(1.1),
                '•' | '◦' => (cw.min(ch) as f32 * 0.22).max(1.6),
                _ => (cw.min(ch) as f32 * 0.34).max(2.4),
            };
            let hollow = matches!(glyph, '○' | '◎' | '◦' | '◇');
            let diamond = matches!(glyph, '◆' | '◇');
            for dy in 0..ch {
                for dx in 0..cw {
                    let (vx, vy) = ((dx as f32 + 0.5 - cx).abs(), (dy as f32 + 0.5 - cy).abs());
                    let d = if diamond { vx + vy * 0.85 } else { (vx * vx + vy * vy).sqrt() };
                    let cov = if hollow {
                        (1.3 - (d - rad).abs()).clamp(0.0, 1.0)
                    } else {
                        (rad + 0.5 - d).clamp(0.0, 1.0)
                    };
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        '⎿' => {
            v_seg(pixels, mx, ch / 5, my + stroke);
            h_seg(pixels, mx, (cw * 4) / 5, my);
        }
        '✓' | '✔' => {
            let sw = (stroke as f32 * 1.2).max(1.5);
            let (x1, y1) = (cw as f32 * 0.18, ch as f32 * 0.55);
            let (x2, y2) = (cw as f32 * 0.42, ch as f32 * 0.74);
            let (x3, y3) = (cw as f32 * 0.84, ch as f32 * 0.26);
            for dy in 0..ch {
                for dx in 0..cw {
                    let (px, py) = (dx as f32, dy as f32);
                    let t1 = (((px - x1) * (x2 - x1) + (py - y1) * (y2 - y1)) / ((x2 - x1).powi(2) + (y2 - y1).powi(2)).max(1.0)).clamp(0.0, 1.0);
                    let d1 = ((px - (x1 + t1 * (x2 - x1))).powi(2) + (py - (y1 + t1 * (y2 - y1))).powi(2)).sqrt();
                    let t2 = (((px - x2) * (x3 - x2) + (py - y2) * (y3 - y2)) / ((x3 - x2).powi(2) + (y3 - y2).powi(2)).max(1.0)).clamp(0.0, 1.0);
                    let d2 = ((px - (x2 + t2 * (x3 - x2))).powi(2) + (py - (y2 + t2 * (y3 - y2))).powi(2)).sqrt();
                    let cov = (sw - d1.min(d2)).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        '✗' | '✘' | '✢' | '✣' | '✤' | '✥' | '✦' | '✧' | '✳' | '✶' | '✷' | '✸' | '✹' | '✺' | '✻' | '✼' | '✽' => {
            let (cx, cy) = (cw as f32 * 0.5, ch as f32 * 0.5);
            let r = (cw.min(ch) as f32 * 0.34).max(2.5);
            let is_cross = matches!(glyph, '✗' | '✘');
            for dy in 0..ch {
                for dx in 0..cw {
                    let (vx, vy) = ((dx as f32 + 0.5 - cx).abs(), (dy as f32 + 0.5 - cy).abs());
                    if vx * vx + vy * vy <= r * r {
                        let d_diag = (vx - vy).abs() * 0.707;
                        let d_axis = vx.min(vy);
                        let d = if is_cross { d_diag } else { d_diag.min(d_axis) };
                        let cov = (1.35 - d).clamp(0.0, 1.0);
                        if cov > 0.0 {
                            blend_px(pixels, x0 + dx, y0 + dy, cov);
                        }
                    }
                }
            }
        }
        '←' | '↑' | '→' | '↓' | '↔' | '↕' | '↳' | '↪' | '↵' | '➜' | '➔' => {
            match glyph {
                '←' => {
                    h_seg(pixels, cw / 5, (cw * 4) / 5, my);
                    v_seg(pixels, cw / 4, my.saturating_sub(stroke * 2), my + stroke * 3);
                }
                '↑' => {
                    v_seg(pixels, mx, ch / 5, (ch * 4) / 5);
                    h_seg(pixels, mx.saturating_sub(stroke * 2), mx + stroke * 3, ch / 4);
                }
                '↓' => {
                    v_seg(pixels, mx, ch / 5, (ch * 4) / 5);
                    h_seg(pixels, mx.saturating_sub(stroke * 2), mx + stroke * 3, (ch * 3) / 4);
                }
                '↕' => v_seg(pixels, mx, ch / 5, (ch * 4) / 5),
                '↳' | '↪' => {
                    v_seg(pixels, cw / 3, ch / 4, my + stroke);
                    h_seg(pixels, cw / 3, (cw * 4) / 5, my);
                }
                '↵' => {
                    v_seg(pixels, (cw * 2) / 3, ch / 4, my + stroke);
                    h_seg(pixels, cw / 5, (cw * 2) / 3 + stroke, my);
                }
                _ => {
                    h_seg(pixels, cw / 5, (cw * 4) / 5, my);
                    v_seg(pixels, (cw * 3) / 5, my.saturating_sub(stroke * 2), my + stroke * 3);
                }
            }
        }
        _ => {}
    }
}
