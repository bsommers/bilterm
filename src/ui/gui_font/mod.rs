//! Native ClearType / TrueType Font Engine, Custom `.ttf`/`.otf` Loader & Geometric Box Rasterizer (`FONT-01`, `FONT-02`, `GPU-02`).
//!
//! Eliminates low-resolution stretched bitmap text in `bilterm --gui` by combining:
//! 1. **Per-Monitor HiDPI Awareness (`SetProcessDPIAware`)** + **1:1 `CreateDIBSection` + `BitBlt`** (never stretches pixels).
//! 2. **Native ClearType TrueType/OpenType Text Rendering (`CreateFontW` with `CLEARTYPE_NATURAL_QUALITY` + `ExtTextOutW`)**
//!    for all text glyphs (`Cascadia Mono`, `Consolas`, `JetBrains Mono`, `Fira Code`, `Iosevka`, `IBM Plex Mono`,
//!    `3270 Nerd Font`, `Hack`, or any user-supplied `.ttf`/`.otf` file via `AddFontResourceExW`).
//! 3. **Pixel-Perfect Edge-to-Edge Geometric Box-Drawing, Block & Braille Rendering** so Borland Turbo Vision
//!    double-line borders (`╔═╗║╚═╝`), split dividers, and shadows connect with zero 1px gaps at any font size.

pub mod geometric;
pub use geometric::*;

#[cfg(windows)]
pub mod win32_gdi;

#[cfg(target_os = "macos")]
pub mod macos_ct;

#[cfg(not(any(windows, target_os = "macos")))]
pub mod linux_x11;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometric_glyph_classification_and_candidate_faces_work() {
        assert!(is_geometric_glyph('╔'));
        assert!(is_geometric_glyph('═'));
        assert!(is_geometric_glyph('█'));
        assert!(is_geometric_glyph('▒'));
        assert!(is_geometric_glyph('⣿'));
        assert!(!is_geometric_glyph('A'));
        assert!(!is_geometric_glyph('λ'));

        let faces = resolve_candidate_font_faces("jetbrains");
        assert_eq!(faces[0], "JetBrains Mono");
        assert!(faces.contains(&"Consolas".to_string()));

        let custom = resolve_candidate_font_faces("My Custom Monospace");
        assert_eq!(custom[0], "My Custom Monospace");
        assert!(custom.contains(&"Cascadia Mono".to_string()));
    }

    #[test]
    fn powerline_and_pua_glyphs_render_without_missing_boxes() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        use crate::ui::glyphs::GlyphPack;

        for ch in ['\u{E0B0}', '\u{E0B1}', '\u{E0B2}', '\u{E0B3}', '\u{E0B4}', '\u{E0B6}', '\u{E0A0}', '\u{E0A2}'] {
            assert!(is_geometric_glyph(ch));
            let mut pixels = vec![0xFF00_0000u32; 16 * 32];
            draw_geometric_cell_bgra(&mut pixels, 16, 32, 0, 0, 16, 32, ch, [85, 255, 255, 255]);
            assert!(pixels.iter().any(|&p| p != 0xFF00_0000), "expected drawn pixels for {ch:?}");
        }

        let area = Rect::new(0, 0, 24, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, " bill@bigmac \u{e0b0} ~ \u{e0b0}", ratatui::style::Style::default());
        sanitize_tui_pua_buffer(&mut buf, GlyphPack::Unicode);
        assert_eq!(buf.cell((13, 0)).unwrap().symbol(), "▶");
        assert_eq!(buf.cell((17, 0)).unwrap().symbol(), "▶");
    }
}
