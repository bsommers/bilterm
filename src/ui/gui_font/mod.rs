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

/// Embedded 256×256 RGBA PNG of the `bil;*` / `==> term█` application icon.
pub const EMBEDDED_APP_ICON_PNG: &[u8] =
    include_bytes!("../../../docs/assets/icons/bilterm-256.png");

/// Sets the running macOS `NSApplication` Dock & Cmd+Tab icon from [`EMBEDDED_APP_ICON_PNG`].
#[cfg(target_os = "macos")]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn apply_macos_dock_icon(ns_app: *mut std::ffi::c_void) {
    use std::ffi::{c_char, c_void};
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    if ns_app.is_null() {
        return;
    }
    unsafe {
        let cls_data = objc_getClass(c"NSData".as_ptr());
        let cls_image = objc_getClass(c"NSImage".as_ptr());
        if cls_data.is_null() || cls_image.is_null() {
            return;
        }
        let sel_data: *mut c_void = sel_registerName(c"dataWithBytes:length:".as_ptr());
        let f_data: unsafe extern "C" fn(*mut c_void, *mut c_void, *const u8, usize) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let ns_data = f_data(
            cls_data,
            sel_data,
            EMBEDDED_APP_ICON_PNG.as_ptr(),
            EMBEDDED_APP_ICON_PNG.len(),
        );
        if ns_data.is_null() {
            return;
        }
        let f_alloc: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let f_init_data: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let f_set_icon: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const ());
        let raw_img = f_alloc(cls_image, sel_registerName(c"alloc".as_ptr()));
        let ns_img = f_init_data(raw_img, sel_registerName(c"initWithData:".as_ptr()), ns_data);
        if !ns_img.is_null() {
            f_set_icon(
                ns_app,
                sel_registerName(c"setApplicationIconImage:".as_ptr()),
                ns_img,
            );
        }
    }
}

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

    #[test]
    fn claude_and_agent_markers_render_in_geometric_and_bitmap_engines() {
        for ch in [
            '❯', '❮', '›', '‹', '»', '«', '▸', '▹', '◂', '◃', '▴', '▾', '‣', '⏺', '⎿', '●', '○', '◉',
            '◎', '•', '◦', '·', '◆', '◇', '▫', '□', '✓', '✔', '✗', '✘', '✢', '✣', '✤', '✥', '✦', '✧',
            '✳', '✶', '✷', '✸', '✹', '✺', '✻', '✼', '✽', '←', '↑', '→', '↓', '↔', '↕', '↳', '↪', '↵',
            '➜', '➔',
        ] {
            assert!(is_geometric_glyph(ch), "expected {ch:?} (U+{:04X}) to be geometric", ch as u32);
            let mut pixels = vec![0xFF00_0000u32; 12 * 24];
            draw_geometric_cell_bgra(&mut pixels, 12, 24, 0, 0, 12, 24, ch, [215, 119, 87, 255]);
            assert!(
                pixels.iter().any(|&p| p != 0xFF00_0000),
                "expected drawn geometric pixels for {ch:?} (U+{:04X})",
                ch as u32
            );
        }
    }
}
