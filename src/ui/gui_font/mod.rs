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

#[cfg(not(windows))]
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
}
