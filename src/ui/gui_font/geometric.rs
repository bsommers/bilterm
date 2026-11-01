use crate::pane::typography::FontFamilyPreset;
use crate::ui::glyphs::GlyphPack;

/// Returns `true` for box-drawing, block/shade, Braille, Powerline (`U+E0A0..=U+E0D7`),
/// CLI/TUI agent markers (`❯`, `⏺`, `⎿`, `✻`, `✢`, `✓`, `✗`, `●`, `○`, `◆`, `▸`, arrows),
/// and modern/Liquid Glass UI icons (`◈`, `◌`, `×`, `⌟`, `⊹`, `◧`, `⧉`, `💧`, `💎`, `✨`, etc.)
/// that are drawn geometrically so window frames, statuslines, and overlays never render
/// missing-glyph tofu boxes (`▯`) when an OS monospace font lacks Dingbats or Emojis.
pub fn is_geometric_glyph(ch: char) -> bool {
    matches!(
        ch,
        '\u{2500}'..='\u{257F}'
            | '\u{2580}'..='\u{259F}'
            | '\u{2800}'..='\u{28FF}'
            | '\u{E0A0}'..='\u{E0D7}'
            | '▪' | '▫' | '■' | '□' | '▣' | '▯' | '⊞' | '❐'
            | '◢' | '◣' | '◤' | '◥' | '◧' | '⧉' | '◷' | '◖' | '◗'
            | '►' | '▶' | '▸' | '▹' | '‣'
            | '◄' | '◀' | '◂' | '◃'
            | '▲' | '▴' | '▼' | '▾'
            | '❯' | '❮' | '›' | '‹' | '»' | '«'
            | '⏺' | '●' | '○' | '◉' | '◎' | '◌' | '•' | '◦' | '·'
            | '◆' | '◇' | '◈' | '♦' | '❖' | '⬢'
            | '⎿' | '⌟' | '⊹' | '⌖' | '⌘' | '⌕' | '⎇' | '⌨' | '⏸' | '⋮'
            | '≡' | '≣' | '☰' | '＋'
            | '✓' | '✔' | '✗' | '✘' | '×' | '✕' | '✖'
            | '✢' | '✣' | '✤' | '✥' | '✦' | '✧' | '✨' | '★' | '☆'
            | '✳' | '✶' | '✷' | '✸' | '✹' | '✺' | '✻' | '✼' | '✽' | '❄' | '⚙' | '☸'
            | '⚠' | '⚡'
            | '←' | '↑' | '→' | '↓' | '↔' | '↕' | '↳' | '↪' | '↵' | '⏎' | '➜' | '➔' | '⬅'
            | '↻' | '⟳' | '⇄' | '⇳'
            | '💧' | '💎' | '🔒' | '🔑' | '📌' | '🔍' | '🌐' | '📡' | '🔔' | '🖥' | '🔗' | '📈'
            | '🎨' | '📁' | '📂' | '📄' | '📝' | '🎛' | '🌿' | '🐍' | '🐚' | '🐳' | '🤖' | '🦀'
    )
}

/// Returns `true` if `ch` is in a Unicode Private Use Area (`U+E000..=U+F8FF` or `U+F0000..=U+10FFFF`).
pub fn is_private_use_area(ch: char) -> bool {
    matches!(ch, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{10FFFF}')
}

/// Maps Nerd Font / Powerline Private Use Area icons and Supplementary Emoji icons to a
/// geometric glyph so missing font glyphs never produce `.notdef` boxes.
pub fn map_pua_icon_fallback(ch: char) -> char {
    if is_geometric_glyph(ch) {
        return ch;
    }
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
        '\u{F071}' | '\u{F06A}' | '\u{F0E7}' | '\u{F421}' => '⚠',
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
        _ => '◈',
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
            '⚠' | '!' => "!",
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

/// Draws a geometric box-drawing, block/shade, Braille, Powerline, or UI icon glyph directly into
/// a 32-bit `0xFFRRGGBB` pixel buffer at `(x0, y0)` with cell dimensions `(cw, ch)`.
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
        let r = (u32::from(fg_rgba[0]) * a + ((dst >> 16) & 0xFF) * inv) >> 8;
        let g = (u32::from(fg_rgba[1]) * a + ((dst >> 8) & 0xFF) * inv) >> 8;
        let b = (u32::from(fg_rgba[2]) * a + (dst & 0xFF) * inv) >> 8;
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
        // Horizontal solid & half-lines
        '─' | '━' | '╼' | '╾' => h_seg(pixels, 0, cw, my),
        '╴' | '╸' => h_seg(pixels, 0, mx + stroke, my),
        '╶' | '╺' => h_seg(pixels, mx, cw, my),
        // Horizontal dashed/dotted lines
        '╌' | '╍' | '┄' | '┅' | '┈' | '┉' => {
            for dx in 0..cw {
                if (dx / 2) % 2 == 0 {
                    fill_rect(pixels, dx, my, dx + 1, my + stroke);
                }
            }
        }
        // Vertical solid & half-lines
        '│' | '┃' | '╽' | '╿' => v_seg(pixels, mx, 0, ch),
        '╵' | '╹' => v_seg(pixels, mx, 0, my + stroke),
        '╷' | '╻' => v_seg(pixels, mx, my, ch),
        // Vertical dashed/dotted lines & vertical ellipsis
        '╎' | '╏' | '┆' | '┇' | '┊' | '┋' | '⋮' => {
            for dy in 0..ch {
                if (dy / 2) % 2 == 0 {
                    fill_rect(pixels, mx, dy, mx + stroke, dy + 1);
                }
            }
        }
        // Smooth anti-aliased quarter-circle rounded corners (`╭`, `╮`, `╰`, `╯`)
        '╭' | '╮' | '╰' | '╯' => {
            let (sx, sy) = match glyph {
                '╭' => (1.0f32, 1.0f32),
                '╮' => (-1.0, 1.0),
                '╰' => (1.0, -1.0),
                _ => (-1.0, -1.0),
            };
            let vx = mx as f32 + stroke as f32 * 0.5;
            let hy = my as f32 + stroke as f32 * 0.5;
            let max_r = (mx.min(cw.saturating_sub(mx + 1)).min(my).min(ch.saturating_sub(my + 1))) as f32;
            let r = (cw.min(ch) as f32 * 0.42).clamp(1.5, max_r.max(1.5));
            let hw = (stroke as f32 * 0.55).max(0.85);
            for dy in 0..ch {
                for dx in 0..cw {
                    let u = (dx as f32 + 0.5 - vx) * sx;
                    let v = (dy as f32 + 0.5 - hy) * sy;
                    let dist = if u <= r && v <= r {
                        ((u - r).hypot(v - r) - r).abs()
                    } else if u > r && v <= r {
                        v.abs()
                    } else if v > r && u <= r {
                        u.abs()
                    } else {
                        u.min(v)
                    };
                    let cov = (hw + 0.5 - dist).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Sharp single/heavy box corners & tees
        '┌' | '┍' | '┎' | '┏' => { h_seg(pixels, mx, cw, my); v_seg(pixels, mx, my, ch); }
        '┐' | '┑' | '┒' | '┓' => { h_seg(pixels, 0, mx + stroke, my); v_seg(pixels, mx, my, ch); }
        '└' | '┕' | '┖' | '┗' => { h_seg(pixels, mx, cw, my); v_seg(pixels, mx, 0, my + stroke); }
        '┘' | '┙' | '┚' | '┛' | '⌟' => { h_seg(pixels, 0, mx + stroke, my); v_seg(pixels, mx, 0, my + stroke); }
        '├' | '┝'..='┣' => { v_seg(pixels, mx, 0, ch); h_seg(pixels, mx, cw, my); }
        '┤' | '┥'..='┫' => { v_seg(pixels, mx, 0, ch); h_seg(pixels, 0, mx + stroke, my); }
        '┬' | '┭'..='┳' => { h_seg(pixels, 0, cw, my); v_seg(pixels, mx, my, ch); }
        '┴' | '┵'..='┻' => { h_seg(pixels, 0, cw, my); v_seg(pixels, mx, 0, my + stroke); }
        '┼' | '┽'..='╋' | '⊹' | '＋' => { h_seg(pixels, 0, cw, my); v_seg(pixels, mx, 0, ch); }
        // Double-line box drawing
        '═' => { h_seg(pixels, 0, cw, ty); h_seg(pixels, 0, cw, by); }
        '║' => { v_seg(pixels, lx, 0, ch); v_seg(pixels, rx, 0, ch); }
        '╔' | '╒' | '╓' => { h_seg(pixels, lx, cw, ty); h_seg(pixels, rx, cw, by); v_seg(pixels, lx, ty, ch); v_seg(pixels, rx, by, ch); }
        '╗' | '╕' | '╖' => { h_seg(pixels, 0, rx + stroke, ty); h_seg(pixels, 0, lx + stroke, by); v_seg(pixels, lx, by, ch); v_seg(pixels, rx, ty, ch); }
        '╚' | '╘' | '╙' => { h_seg(pixels, rx, cw, ty); h_seg(pixels, lx, cw, by); v_seg(pixels, lx, 0, by + stroke); v_seg(pixels, rx, 0, ty + stroke); }
        '╝' | '╛' | '╜' => { h_seg(pixels, 0, lx + stroke, ty); h_seg(pixels, 0, rx + stroke, by); v_seg(pixels, lx, 0, ty + stroke); v_seg(pixels, rx, 0, by + stroke); }
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
        // Box diagonals (`╱`, `╲`, `╳`) and Powerline diagonal slashes
        '╱' | '╲' | '╳' | '\u{E0B9}' | '\u{E0BF}' | '\u{E0BB}' | '\u{E0BD}' => {
            let draw_fwd = matches!(glyph, '╱' | '╳' | '\u{E0BB}' | '\u{E0BD}');
            let draw_bwd = matches!(glyph, '╲' | '╳' | '\u{E0B9}' | '\u{E0BF}');
            let sw = (stroke as f32).max(1.35);
            let span = (cw.saturating_sub(1) as f32).max(1.0);
            for dy in 0..ch {
                let t = (dy as f32) / (ch.saturating_sub(1).max(1) as f32);
                for dx in 0..cw {
                    let mut cov = 0.0f32;
                    if draw_fwd {
                        cov = cov.max((sw - ((dx as f32) - (1.0 - t) * span).abs()).clamp(0.0, 1.0));
                    }
                    if draw_bwd {
                        cov = cov.max((sw - ((dx as f32) - t * span).abs()).clamp(0.0, 1.0));
                    }
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Block elements (`U+2580..=U+259F`)
        '█' => fill_rect(pixels, 0, 0, cw, ch),
        '▀' => fill_rect(pixels, 0, 0, cw, my),
        '▄' => fill_rect(pixels, 0, my, cw, ch),
        '▁'..='▇' => {
            let level = (glyph as u32 - '▁' as u32 + 1).min(8);
            fill_rect(pixels, 0, ch.saturating_sub((ch * level) / 8), cw, ch);
        }
        '▉'..='▏' => {
            // Left fractional blocks: '▉' (7/8) down to '▌' (4/8), '▍' (3/8), '▎' (2/8), '▏' (1/8)
            let eighths = 7u32.saturating_sub(glyph as u32 - '▉' as u32).max(1);
            fill_rect(pixels, 0, 0, ((cw * eighths + 4) / 8).max(stroke), ch);
        }
        '▐' => fill_rect(pixels, mx, 0, cw, ch),
        '▔' => fill_rect(pixels, 0, 0, cw, (ch / 8).max(stroke)),
        '▕' => fill_rect(pixels, cw.saturating_sub((cw / 8).max(stroke)), 0, cw, ch),
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
        '▖'..='▟' => {
            const MASKS: [u8; 10] = [
                0b0100, 0b1000, 0b0001, 0b1101, 0b1001, 0b0111, 0b1011, 0b0010, 0b0110, 0b1110,
            ];
            let m = MASKS[(glyph as usize - 0x2596).min(9)];
            if m & 0b0001 != 0 { fill_rect(pixels, 0, 0, mx, my); }
            if m & 0b0010 != 0 { fill_rect(pixels, mx, 0, cw, my); }
            if m & 0b0100 != 0 { fill_rect(pixels, 0, my, mx, ch); }
            if m & 0b1000 != 0 { fill_rect(pixels, mx, my, cw, ch); }
        }
        // Squares, sidebars, split windows, and hollow cursor rectangles
        '▪' | '■' => fill_rect(pixels, cw / 4, ch / 3, (cw * 3) / 4, (ch * 2) / 3),
        '▫' | '□' | '▯' | '▣' | '⊞' | '◧' | '⧉' | '❐' | '🖥' | '📁' | '📂' | '📄' | '📝' => {
            let (x1, y1, x2, y2) = if glyph == '▯' {
                (cw / 5, ch / 6, (cw * 4) / 5, (ch * 5) / 6)
            } else {
                (cw / 6, ch / 4, (cw * 5) / 6, (ch * 3) / 4)
            };
            h_seg(pixels, x1, x2, y1);
            h_seg(pixels, x1, x2, y2.saturating_sub(stroke));
            v_seg(pixels, x1, y1, y2);
            v_seg(pixels, x2.saturating_sub(stroke), y1, y2);
            match glyph {
                '▣' | '🖥' => fill_rect(pixels, x1 + stroke + 1, y1 + stroke + 1, x2.saturating_sub(stroke + 1), y2.saturating_sub(stroke + 1)),
                '⊞' => {
                    h_seg(pixels, x1, x2, my);
                    v_seg(pixels, mx, y1, y2);
                }
                '◧' => fill_rect(pixels, x1, y1, x1 + ((x2 - x1) * 2) / 5, y2),
                '⧉' | '❐' => {
                    let ox = stroke + 1;
                    h_seg(pixels, (x1 + ox).min(cw), (x2 + ox).min(cw), y1.saturating_sub(ox));
                    v_seg(pixels, (x2 + ox).saturating_sub(stroke).min(cw), y1.saturating_sub(ox), y2.saturating_sub(ox));
                }
                _ => {}
            }
        }
        // Trigram / menu bars (`≡`, `≣`, `☰`, `⏸`)
        '≡' | '≣' | '☰' => {
            let x1 = cw / 5;
            let x2 = (cw * 4) / 5;
            h_seg(pixels, x1, x2, ch / 4);
            h_seg(pixels, x1, x2, my);
            h_seg(pixels, x1, x2, (ch * 3) / 4);
        }
        '⏸' => {
            v_seg(pixels, cw / 3, ch / 4, (ch * 3) / 4);
            v_seg(pixels, (cw * 2) / 3, ch / 4, (ch * 3) / 4);
        }
        // Corner triangles (`◢`, `◣`, `◤`, `◥`, `\u{E0B8}`, `\u{E0BA}`, `\u{E0BC}`, `\u{E0BE}`)
        '◢' | '◣' | '◤' | '◥' | '\u{E0B8}' | '\u{E0BA}' | '\u{E0BC}' | '\u{E0BE}' => {
            let right = matches!(glyph, '◢' | '◥' | '\u{E0BA}' | '\u{E0BE}');
            let lower = matches!(glyph, '◢' | '◣' | '\u{E0B8}' | '\u{E0BA}');
            for dy in 0..ch {
                let ry = if lower { dy + 1 } else { ch - dy };
                let span = (ry as f32 * cw as f32) / (ch.max(1) as f32);
                for dx in 0..cw {
                    let cov = if right {
                        (dx as f32 + 1.0 - (cw as f32 - span)).clamp(0.0, 1.0)
                    } else {
                        (span - dx as f32).clamp(0.0, 1.0)
                    };
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Pointer triangles (`►`, `▶`, `▸`, `◄`, `◀`, `◂`, `▲`, `▼`)
        '►' | '▶' | '▸' | '▹' | '‣' | '◄' | '◀' | '◂' | '◃' => {
            let left = matches!(glyph, '◄' | '◀' | '◂' | '◃');
            let h_span = (ch / 2).max(4);
            let y_top = (ch.saturating_sub(h_span)) / 2;
            for i in 0..h_span {
                let dist = if i < h_span / 2 { i } else { h_span - 1 - i };
                let w = ((dist + 1) * (cw / 2).max(2)) / (h_span / 2).max(1);
                if left {
                    let x_end = (cw * 3) / 4;
                    fill_rect(pixels, x_end.saturating_sub(w), y_top + i, x_end, y_top + i + 1);
                } else {
                    fill_rect(pixels, cw / 4, y_top + i, (cw / 4 + w).min(cw), y_top + i + 1);
                }
            }
        }
        '▲' | '▴' | '▼' | '▾' | '⚠' => {
            let down = matches!(glyph, '▼' | '▾');
            let rows = (ch / 3).max(4);
            let y_top = (ch.saturating_sub(rows)) / 2;
            for r in 0..rows {
                let idx = if down { rows - 1 - r } else { r };
                let half = ((idx + 1) * cw) / (rows * 2 + 2);
                fill_rect(pixels, mx.saturating_sub(half), y_top + r, (mx + half + 1).min(cw), y_top + r + 1);
            }
        }
        // Braille patterns (`U+2800..=U+28FF`)
        '\u{2800}'..='\u{28FF}' => {
            let mask = (glyph as u32 - 0x2800) as u8;
            let dot_bits: [(u8, u32, u32); 8] = [
                (0x01, 0, 0), (0x02, 0, 1), (0x04, 0, 2), (0x08, 1, 0),
                (0x10, 1, 1), (0x20, 1, 2), (0x40, 0, 3), (0x80, 1, 3),
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
        // Powerline solid right/left triangles (``, ``)
        '\u{E0B0}' | '\u{E0C0}' | '\u{E0C4}' | '\u{E0C6}' | '\u{E0C8}' | '\u{E0CC}'
        | '\u{E0CE}' | '\u{E0D2}' | '\u{E0B2}' | '\u{E0C2}' | '\u{E0C7}' | '\u{E0CA}'
        | '\u{E0CF}' | '\u{E0D4}' => {
            let left = matches!(glyph, '\u{E0B2}' | '\u{E0C2}' | '\u{E0C7}' | '\u{E0CA}' | '\u{E0CF}' | '\u{E0D4}');
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            for dy in 0..ch {
                let dist = ((dy as f32) - cy).abs() / half;
                for dx in 0..cw {
                    let cov = if left {
                        ((dx as f32 + 1.0) - dist * (cw as f32)).clamp(0.0, 1.0)
                    } else {
                        ((1.0 - dist).max(0.0) * (cw as f32) - (dx as f32)).clamp(0.0, 1.0)
                    };
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline & prompt right/left chevrons (``, ``, `❯`, `❮`, `›`, `‹`, `»`, `«`)
        '\u{E0B1}' | '\u{E0C1}' | '\u{E0C5}' | '\u{E0CD}' | '❯' | '›' | '»'
        | '\u{E0B3}' | '\u{E0C3}' | '\u{E0CB}' | '❮' | '‹' | '«' => {
            let left = matches!(glyph, '\u{E0B3}' | '\u{E0C3}' | '\u{E0CB}' | '❮' | '‹' | '«');
            let is_prompt_chev = matches!(glyph, '❯' | '›' | '»' | '❮' | '‹' | '«');
            let double = matches!(glyph, '»' | '«');
            let y_pad = if is_prompt_chev { (ch as f32) * 0.22 } else { 0.0 };
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = (cy - y_pad).max(2.0);
            let sw = if matches!(glyph, '❯' | '❮') {
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
                let xc = x_min + (if left { dist } else { 1.0 - dist }) * span;
                for dx in 0..cw {
                    let cov = (sw - ((dx as f32) - xc).abs()).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                    if double {
                        let shift = if left { span * 0.45 } else { -span * 0.45 };
                        let cov2 = (sw - ((dx as f32) - (xc + shift)).abs()).clamp(0.0, 1.0);
                        if cov2 > 0.0 {
                            blend_px(pixels, x0 + dx, y0 + dy, cov2);
                        }
                    }
                }
            }
        }
        // Powerline rounded pill caps & half-circles (``, ``, ``, ``, `◗`, `◖`)
        '\u{E0B4}'..='\u{E0B7}' | '◗' | '◖' => {
            let left = matches!(glyph, '\u{E0B6}' | '\u{E0B7}' | '◖');
            let hollow = matches!(glyph, '\u{E0B5}' | '\u{E0B7}');
            let cy = (ch as f32 - 1.0) * 0.5;
            let half = cy.max(1.0);
            let sw = (stroke as f32).max(1.4);
            let span = (cw.saturating_sub(1) as f32).max(1.0);
            for dy in 0..ch {
                let ny = ((dy as f32) - cy) / half;
                let arc = (1.0 - ny * ny).max(0.0).sqrt();
                for dx in 0..cw {
                    let cov = if hollow {
                        let xc = if left { span * (1.0 - arc) } else { arc * span };
                        (sw - ((dx as f32) - xc).abs()).clamp(0.0, 1.0)
                    } else if left {
                        ((dx as f32 + 1.0) - (cw as f32) * (1.0 - arc)).clamp(0.0, 1.0)
                    } else {
                        (arc * (cw as f32) - (dx as f32)).clamp(0.0, 1.0)
                    };
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Powerline Git branch (``) & link/hotkey symbols (`⎇`, `🔗`)
        '\u{E0A0}' | '⎇' | '🔗' => {
            let sx = (cw / 4).max(1);
            let bx = ((cw * 3) / 4).min(cw.saturating_sub(stroke + 1));
            let top_y = (ch / 5).max(1);
            let bot_y = ((ch * 4) / 5).min(ch.saturating_sub(stroke + 1));
            v_seg(pixels, sx, top_y, bot_y + stroke);
            fill_rect(pixels, sx.saturating_sub(1), top_y.saturating_sub(1), sx + stroke + 1, top_y + stroke + 1);
            fill_rect(pixels, sx.saturating_sub(1), bot_y.saturating_sub(1), sx + stroke + 1, bot_y + stroke + 1);
            fill_rect(pixels, bx.saturating_sub(1), top_y.saturating_sub(1), bx + stroke + 1, top_y + stroke + 1);
            v_seg(pixels, bx, top_y, my + stroke);
            h_seg(pixels, sx, bx + stroke, my);
        }
        // Padlock / Key / Security icons (``, `🔒`, `🔑`)
        '\u{E0A2}' | '🔒' | '🔑' => {
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
        // Powerline Line/Column icons
        '\u{E0A1}' | '\u{E0A3}' | '\u{E0C0}'..='\u{E0D7}' => {
            v_seg(pixels, lx, ch / 4, (ch * 3) / 4);
            h_seg(pixels, lx, rx + stroke, (ch * 3) / 4);
        }
        // Circles, rings, diamonds, droplets, gems, and status badges
        '⏺' | '●' | '○' | '◉' | '◎' | '◌' | '•' | '◦' | '·'
        | '◆' | '◇' | '◈' | '♦' | '❖' | '⬢' | '💧' | '💎'
        | '⌖' | '⌕' | '🔍' | '📌' | '🌐' | '📡' | '🔔' | '◷' | '⌘' | '⌨'
        | '🎨' | '🎛' | '🌿' | '🐍' | '🐚' | '🐳' | '🤖' | '🦀' | '📈' => {
            let (cx, cy) = (cw as f32 * 0.5, ch as f32 * 0.5);
            let rad = match glyph {
                '·' => (cw.min(ch) as f32 * 0.14).max(1.1),
                '•' | '◦' => (cw.min(ch) as f32 * 0.22).max(1.6),
                _ => (cw.min(ch) as f32 * 0.35).max(2.6),
            };
            let hollow = matches!(glyph, '○' | '◎' | '◌' | '◦' | '◇' | '◈' | '⌖' | '⌕' | '🔍' | '🌐' | '📡' | '◷' | '⌘');
            let inner_dot = matches!(glyph, '◉' | '◎' | '◈' | '❖' | '⌖' | '📌' | '📡');
            let diamond = matches!(glyph, '◆' | '◇' | '◈' | '♦' | '❖' | '💧' | '💎');
            for dy in 0..ch {
                for dx in 0..cw {
                    let (vx, vy_raw) = (dx as f32 + 0.5 - cx, dy as f32 + 0.5 - cy);
                    let vy = if glyph == '💧' && vy_raw < 0.0 { vy_raw * 0.72 } else { vy_raw };
                    let d = if diamond {
                        vx.abs() + vy.abs() * 0.82
                    } else {
                        vx.hypot(vy)
                    };
                    if glyph == '◌' && ((dx / 2 + dy / 2) % 2 == 1) {
                        continue;
                    }
                    let mut cov = if hollow {
                        (1.3 - (d - rad).abs()).clamp(0.0, 1.0)
                    } else {
                        (rad + 0.5 - d).clamp(0.0, 1.0)
                    };
                    if inner_dot {
                        cov = cov.max((rad * 0.42 + 0.5 - d).clamp(0.0, 1.0));
                    }
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
            if matches!(glyph, '⌖' | '🌐') {
                h_seg(pixels, cw / 6, (cw * 5) / 6, my);
                v_seg(pixels, mx, ch / 5, (ch * 4) / 5);
            } else if matches!(glyph, '⌕' | '🔍') {
                // Magnifying glass handle in bottom-right
                for i in 0..(cw / 3).max(2) {
                    put(pixels, x0 + (mx + i + 1).min(cw - 1), y0 + (my + i + 1).min(ch - 1));
                    put(pixels, x0 + (mx + i).min(cw - 1), y0 + (my + i + 1).min(ch - 1));
                }
            }
        }
        '⎿' => {
            v_seg(pixels, mx, ch / 5, my + stroke);
            h_seg(pixels, mx, (cw * 4) / 5, my);
        }
        // Checkmarks (`✓`, `✔`)
        '✓' | '✔' => {
            let sw = (stroke as f32 * 1.2).max(1.5);
            let (x1, y1) = (cw as f32 * 0.18, ch as f32 * 0.55);
            let (x2, y2) = (cw as f32 * 0.42, ch as f32 * 0.74);
            let (x3, y3) = (cw as f32 * 0.84, ch as f32 * 0.26);
            for dy in 0..ch {
                for dx in 0..cw {
                    let (px, py) = (dx as f32, dy as f32);
                    let t1 = (((px - x1) * (x2 - x1) + (py - y1) * (y2 - y1)) / ((x2 - x1).powi(2) + (y2 - y1).powi(2)).max(1.0)).clamp(0.0, 1.0);
                    let d1 = (px - (x1 + t1 * (x2 - x1))).hypot(py - (y1 + t1 * (y2 - y1)));
                    let t2 = (((px - x2) * (x3 - x2) + (py - y2) * (y3 - y2)) / ((x3 - x2).powi(2) + (y3 - y2).powi(2)).max(1.0)).clamp(0.0, 1.0);
                    let d2 = (px - (x2 + t2 * (x3 - x2))).hypot(py - (y2 + t2 * (y3 - y2)));
                    let cov = (sw - d1.min(d2)).clamp(0.0, 1.0);
                    if cov > 0.0 {
                        blend_px(pixels, x0 + dx, y0 + dy, cov);
                    }
                }
            }
        }
        // Crosses (`×`, `✕`, `✖`, `✗`, `✘`), stars, sparkles (`✨`, `✦`, `★`), and spinners
        '×' | '✕' | '✖' | '✗' | '✘' | '✢' | '✣' | '✤' | '✥' | '✦' | '✧' | '✨' | '★' | '☆'
        | '✳' | '✶' | '✷' | '✸' | '✹' | '✺' | '✻' | '✼' | '✽' | '❄' | '⚙' | '☸' | '⚡' => {
            let (cx, cy) = (cw as f32 * 0.5, ch as f32 * 0.5);
            let r = (cw.min(ch) as f32 * 0.34).max(2.5);
            let is_cross = matches!(glyph, '×' | '✕' | '✖' | '✗' | '✘');
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
        // Arrows, refresh (`↻`, `⟳`), and return (`↵`, `⏎`)
        '←' | '⬅' | '↑' | '→' | '↓' | '↔' | '⇄' | '↕' | '⇳' | '↳' | '↪' | '↵' | '⏎' | '➜' | '➔' | '↻' | '⟳' => {
            match glyph {
                '←' | '⬅' => {
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
                '↕' | '⇳' => {
                    v_seg(pixels, mx, ch / 5, (ch * 4) / 5);
                    h_seg(pixels, mx.saturating_sub(stroke * 2), mx + stroke * 3, ch / 4);
                    h_seg(pixels, mx.saturating_sub(stroke * 2), mx + stroke * 3, (ch * 3) / 4);
                }
                '↔' | '⇄' => {
                    h_seg(pixels, cw / 6, (cw * 5) / 6, my);
                    v_seg(pixels, cw / 4, my.saturating_sub(stroke * 2), my + stroke * 3);
                    v_seg(pixels, (cw * 3) / 4, my.saturating_sub(stroke * 2), my + stroke * 3);
                }
                '↳' | '↪' => {
                    v_seg(pixels, cw / 3, ch / 4, my + stroke);
                    h_seg(pixels, cw / 3, (cw * 4) / 5, my);
                }
                '↵' | '⏎' => {
                    v_seg(pixels, (cw * 2) / 3, ch / 4, my + stroke);
                    h_seg(pixels, cw / 5, (cw * 2) / 3 + stroke, my);
                    v_seg(pixels, cw / 4, my.saturating_sub(stroke), my + stroke * 2);
                }
                '↻' | '⟳' => {
                    let (cx, cy) = (cw as f32 * 0.5, ch as f32 * 0.5);
                    let rad = (cw.min(ch) as f32 * 0.32).max(2.2);
                    for dy in 0..ch {
                        for dx in 0..cw {
                            let (vx, vy) = (dx as f32 + 0.5 - cx, dy as f32 + 0.5 - cy);
                            if !(vx > 0.0 && vy > 0.0 && vy < vx) {
                                let cov = (1.3 - (vx.hypot(vy) - rad).abs()).clamp(0.0, 1.0);
                                if cov > 0.0 {
                                    blend_px(pixels, x0 + dx, y0 + dy, cov);
                                }
                            }
                        }
                    }
                    h_seg(pixels, mx, (cw * 4) / 5, ch / 4);
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
