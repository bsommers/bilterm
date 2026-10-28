//! macOS CoreText + CoreGraphics 1:1 Retina HiDPI Font Engine & Framebuffer Canvas (`FONT-01`, `FONT-02`, `GPU-02`).
//!
//! Renders [`GuiFrameSurface`] on macOS using:
//! 1. **CoreText (`CTFontRef`)** for subpixel/antialiased TrueType/OpenType text (`SF Mono`, `Menlo`,
//!    `Monaco`, `JetBrains Mono`, `Fira Code`, `Cascadia Mono`, `IBM Plex Mono`, `3270 Nerd Font`,
//!    or custom `.ttf`/`.otf` files registered via `CTFontManagerRegisterFontsForURL`) with automatic
//!    system glyph fallback via `CTFontCreateForString`.
//! 2. **Edge-to-Edge Geometric Box-Drawing (`draw_geometric_cell_bgra`)** for zero-gap Borland Turbo Vision
//!    double-line borders (`╔═╗║╚═╝`), blocks, and Braille.
//! 3. **1:1 `CGBitmapContext` -> `CGImageRef`** presented directly on the window's `CALayer` at Retina scale.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use super::geometric::{
    draw_geometric_cell_bgra, is_geometric_glyph, map_pua_icon_fallback,
    resolve_candidate_font_faces,
};
use crate::pane::typography::{
    FontFamilyPreset, default_custom_fonts_dir, register_custom_font_file,
    registered_custom_fonts, scan_and_register_fonts_dir,
};
use crate::ui::renderer::GuiFrameSurface;
use crate::wm::WindowManager;

pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFURLRef = *const c_void;
pub type CTFontRef = *const c_void;
pub type CGColorSpaceRef = *mut c_void;
pub type CGContextRef = *mut c_void;
pub type CGImageRef = *mut c_void;
pub type CGGlyph = u16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CFRange {
    pub location: isize,
    pub length: isize,
}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_STRING_ENCODING_MAC_ROMAN: u32 = 0;
const K_CG_IMAGE_ALPHA_NONE_SKIP_FIRST: u32 = 6;
const K_CG_BITMAP_BYTE_ORDER_32_LITTLE: u32 = 2 << 12;
const K_CT_FONT_TRAIT_ITALIC: u32 = 1 << 0;
const K_CT_FONT_TRAIT_BOLD: u32 = 1 << 1;
const K_CT_FONT_ORIENTATION_DEFAULT: u32 = 0;
const K_CT_FONT_MANAGER_SCOPE_PROCESS: u32 = 1;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        num_bytes: isize,
        encoding: u32,
        is_external_representation: u8,
    ) -> CFStringRef;
    fn CFStringGetLength(the_string: CFStringRef) -> isize;
    fn CFStringGetCharacters(the_string: CFStringRef, range: CFRange, buffer: *mut u16);
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: CFTypeRef,
        buffer: *const u8,
        buf_len: isize,
        is_directory: u8,
    ) -> CFURLRef;
    pub fn CFRelease(cf: CFTypeRef);
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    fn CGColorSpaceRelease(space: CGColorSpaceRef);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: CGColorSpaceRef,
        bitmap_info: u32,
    ) -> CGContextRef;
    fn CGBitmapContextCreateImage(context: CGContextRef) -> CGImageRef;
    fn CGContextRelease(c: CGContextRef);
    pub fn CGImageRelease(image: CGImageRef);
    fn CGContextSetRGBFillColor(c: CGContextRef, red: f64, green: f64, blue: f64, alpha: f64);
    fn CGContextSetShouldAntialias(c: CGContextRef, should_antialias: bool);
    fn CGContextSetShouldSmoothFonts(c: CGContextRef, should_smooth_fonts: bool);
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(name: CFStringRef, size: f64, matrix: *const c_void) -> CTFontRef;
    fn CTFontCreateCopyWithSymbolicTraits(
        font: CTFontRef,
        size: f64,
        matrix: *const c_void,
        sym_trait_value: u32,
        sym_trait_mask: u32,
    ) -> CTFontRef;
    fn CTFontCreateForString(
        current_font: CTFontRef,
        string: CFStringRef,
        range: CFRange,
    ) -> CTFontRef;
    fn CTFontCopyFamilyName(font: CTFontRef) -> CFStringRef;
    fn CTFontGetAscent(font: CTFontRef) -> f64;
    fn CTFontGetDescent(font: CTFontRef) -> f64;
    fn CTFontGetLeading(font: CTFontRef) -> f64;
    fn CTFontGetGlyphsForCharacters(
        font: CTFontRef,
        characters: *const u16,
        glyphs: *mut CGGlyph,
        count: isize,
    ) -> bool;
    fn CTFontGetAdvancesForGlyphs(
        font: CTFontRef,
        orientation: u32,
        glyphs: *const CGGlyph,
        advances: *mut CGSize,
        count: isize,
    ) -> f64;
    fn CTFontDrawGlyphs(
        font: CTFontRef,
        glyphs: *const CGGlyph,
        positions: *const CGPoint,
        count: usize,
        context: CGContextRef,
    );
    fn CTFontManagerRegisterFontsForURL(
        font_url: CFURLRef,
        scope: u32,
        error: *mut CFTypeRef,
    ) -> bool;
}

pub fn create_cfstring(s: &str) -> CFStringRef {
    unsafe {
        let cf = CFStringCreateWithBytes(
            null(),
            s.as_ptr(),
            s.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
            0,
        );
        if !cf.is_null() {
            cf
        } else {
            CFStringCreateWithBytes(
                null(),
                s.as_ptr(),
                s.len() as isize,
                K_CF_STRING_ENCODING_MAC_ROMAN,
                0,
            )
        }
    }
}

pub fn cfstring_to_string(cf: CFStringRef) -> String {
    if cf.is_null() {
        return String::new();
    }
    let len = unsafe { CFStringGetLength(cf) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize];
    unsafe {
        CFStringGetCharacters(
            cf,
            CFRange {
                location: 0,
                length: len,
            },
            buf.as_mut_ptr(),
        );
    }
    String::from_utf16_lossy(&buf)
}

fn is_last_resort_font(font: CTFontRef) -> bool {
    if font.is_null() {
        return true;
    }
    let cf = unsafe { CTFontCopyFamilyName(font) };
    let name = cfstring_to_string(cf);
    if !cf.is_null() {
        unsafe { CFRelease(cf) };
    }
    name.to_ascii_lowercase().contains("lastresort")
}

fn try_draw_with_coretext_fallback(
    font: CTFontRef,
    text: &str,
    pos: CGPoint,
    ctx: CGContextRef,
) -> bool {
    let utf16: Vec<u16> = text.encode_utf16().collect();
    if utf16.is_empty() {
        return false;
    }
    let mut glyphs = vec![0u16; utf16.len()];
    let primary_ok = unsafe {
        CTFontGetGlyphsForCharacters(
            font,
            utf16.as_ptr(),
            glyphs.as_mut_ptr(),
            utf16.len() as isize,
        )
    };
    if primary_ok && glyphs[0] != 0 && !is_last_resort_font(font) {
        unsafe {
            CTFontDrawGlyphs(font, glyphs.as_ptr(), &pos, 1, ctx);
        }
        return true;
    }
    let cf_str = create_cfstring(text);
    if cf_str.is_null() {
        return false;
    }
    let cf_len = unsafe { CFStringGetLength(cf_str) };
    let fb_font = unsafe {
        CTFontCreateForString(
            font,
            cf_str,
            CFRange {
                location: 0,
                length: cf_len,
            },
        )
    };
    let mut drew = false;
    if !fb_font.is_null() {
        if !is_last_resort_font(fb_font) {
            let fb_ok = unsafe {
                CTFontGetGlyphsForCharacters(
                    fb_font,
                    utf16.as_ptr(),
                    glyphs.as_mut_ptr(),
                    utf16.len() as isize,
                )
            };
            if fb_ok && glyphs[0] != 0 {
                unsafe {
                    CTFontDrawGlyphs(fb_font, glyphs.as_ptr(), &pos, 1, ctx);
                }
                drew = true;
            }
        }
        unsafe { CFRelease(fb_font) };
    }
    unsafe { CFRelease(cf_str) };
    drew
}

/// CoreText + CoreGraphics 1:1 Retina HiDPI canvas for `bilterm --gui` on macOS.
pub struct MacosGuiCanvas {
    pub pixels: Vec<u32>,
    pub width_px: u32,
    pub height_px: u32,
    active_family_req: String,
    pub resolved_face_name: String,
    active_px_height: i32,
    pub cell_width_px: u16,
    pub cell_height_px: u16,
    ascent_px: f64,
    ct_regular: CTFontRef,
    ct_bold: CTFontRef,
    ct_italic: CTFontRef,
    ct_bold_italic: CTFontRef,
    loaded_paths: Vec<PathBuf>,
}

impl MacosGuiCanvas {
    pub fn new(wm: &WindowManager, scale_factor: f64) -> Self {
        if let Some(dir) = default_custom_fonts_dir() {
            let _ = scan_and_register_fonts_dir(&dir);
        }
        for f in &wm.config().font_files {
            let _ = register_custom_font_file(Path::new(f));
        }
        let mut canvas = Self {
            pixels: Vec::new(),
            width_px: 0,
            height_px: 0,
            active_family_req: String::new(),
            resolved_face_name: String::new(),
            active_px_height: 0,
            cell_width_px: 10,
            cell_height_px: 20,
            ascent_px: 15.0,
            ct_regular: null(),
            ct_bold: null(),
            ct_italic: null(),
            ct_bold_italic: null(),
            loaded_paths: Vec::new(),
        };
        let _ = canvas.sync_font(wm, scale_factor);
        canvas
    }

    fn sync_private_font_files(&mut self) {
        use std::os::unix::ffi::OsStrExt;
        for (_, path) in registered_custom_fonts() {
            if !self.loaded_paths.contains(&path) {
                let bytes = path.as_os_str().as_bytes();
                let url = unsafe {
                    CFURLCreateFromFileSystemRepresentation(
                        null(),
                        bytes.as_ptr(),
                        bytes.len() as isize,
                        0,
                    )
                };
                if !url.is_null() {
                    unsafe {
                        let _ = CTFontManagerRegisterFontsForURL(
                            url,
                            K_CT_FONT_MANAGER_SCOPE_PROCESS,
                            null_mut(),
                        );
                        CFRelease(url);
                    }
                }
                self.loaded_paths.push(path);
            }
        }
    }

    fn destroy_fonts(&mut self) {
        for f in [
            self.ct_regular,
            self.ct_bold,
            self.ct_italic,
            self.ct_bold_italic,
        ] {
            if !f.is_null() {
                unsafe {
                    CFRelease(f);
                }
            }
        }
        self.ct_regular = null();
        self.ct_bold = null();
        self.ct_italic = null();
        self.ct_bold_italic = null();
    }

    /// Synchronizes CoreText `CTFontRef` instances with `wm`'s active font family, point size,
    /// zoom percentage, and Retina `scale_factor`. Returns `(cell_width_px, cell_height_px, changed)`.
    pub fn sync_font(&mut self, wm: &WindowManager, scale_factor: f64) -> (u16, u16, bool) {
        self.sync_private_font_files();

        let pane_fam = &wm.active_pane().font_options().family;
        let desired_fam = if *pane_fam != FontFamilyPreset::Ibm3270 {
            pane_fam.display_name().to_string()
        } else if !wm.config().font_family.trim().is_empty() {
            wm.config().font_family.trim().to_string()
        } else {
            "Menlo".to_string()
        };

        let eff_pt = wm.active_pane().font_options().effective_size_pt(wm.config());
        let scale = scale_factor.clamp(1.0, 4.0);
        let px_size = (f64::from(eff_pt) * scale).round().clamp(10.0, 128.0);
        let px_height_key = px_size as i32;

        if desired_fam == self.active_family_req
            && px_height_key == self.active_px_height
            && !self.ct_regular.is_null()
        {
            return (self.cell_width_px, self.cell_height_px, false);
        }

        self.destroy_fonts();
        self.active_family_req = desired_fam.clone();
        self.active_px_height = px_height_key;

        let mut candidates = resolve_candidate_font_faces(&desired_fam);
        for mac_fb in ["SF Mono", "Menlo", "Monaco", "Andale Mono", "Courier New"] {
            if !candidates.iter().any(|s| s.eq_ignore_ascii_case(mac_fb)) {
                candidates.push(mac_fb.to_string());
            }
        }

        let mut chosen_font: CTFontRef = null();
        let mut chosen_name = "Menlo".to_string();

        for cand in &candidates {
            let cf_name = create_cfstring(cand);
            if cf_name.is_null() {
                continue;
            }
            let ct = unsafe { CTFontCreateWithName(cf_name, px_size, null()) };
            unsafe { CFRelease(cf_name) };
            if !ct.is_null() {
                let actual_cf = unsafe { CTFontCopyFamilyName(ct) };
                let actual = cfstring_to_string(actual_cf);
                if !actual_cf.is_null() {
                    unsafe { CFRelease(actual_cf) };
                }
                let cand_lower = cand.to_ascii_lowercase();
                let actual_lower = actual.to_ascii_lowercase();
                if actual_lower == cand_lower
                    || actual_lower.starts_with(&cand_lower)
                    || cand_lower.starts_with(&actual_lower)
                {
                    chosen_font = ct;
                    chosen_name = actual;
                    break;
                }
                unsafe { CFRelease(ct) };
            }
        }

        if chosen_font.is_null() {
            let cf_menlo = create_cfstring("Menlo");
            chosen_font = unsafe { CTFontCreateWithName(cf_menlo, px_size, null()) };
            unsafe { CFRelease(cf_menlo) };
            chosen_name = "Menlo".to_string();
        }

        self.resolved_face_name = chosen_name;
        self.ct_regular = chosen_font;
        self.ct_bold = Self::create_styled_variant(chosen_font, px_size, K_CT_FONT_TRAIT_BOLD);
        self.ct_italic = Self::create_styled_variant(chosen_font, px_size, K_CT_FONT_TRAIT_ITALIC);
        self.ct_bold_italic = Self::create_styled_variant(
            chosen_font,
            px_size,
            K_CT_FONT_TRAIT_BOLD | K_CT_FONT_TRAIT_ITALIC,
        );

        let ascent = unsafe { CTFontGetAscent(self.ct_regular) }.max(8.0);
        let descent = unsafe { CTFontGetDescent(self.ct_regular) }.max(2.0);
        let leading = unsafe { CTFontGetLeading(self.ct_regular) }.max(1.0);
        self.ascent_px = ascent;

        let ch_m: u16 = b'M' as u16;
        let mut glyph_m: CGGlyph = 0;
        let advance = unsafe {
            if CTFontGetGlyphsForCharacters(self.ct_regular, &ch_m, &mut glyph_m, 1) {
                CTFontGetAdvancesForGlyphs(
                    self.ct_regular,
                    K_CT_FONT_ORIENTATION_DEFAULT,
                    &glyph_m,
                    null_mut(),
                    1,
                )
            } else {
                px_size * 0.6
            }
        };

        self.cell_width_px = advance.round().clamp(6.0, 96.0) as u16;
        self.cell_height_px = (ascent + descent + leading).ceil().clamp(12.0, 192.0) as u16;

        (self.cell_width_px, self.cell_height_px, true)
    }

    fn create_styled_variant(base: CTFontRef, px_size: f64, traits: u32) -> CTFontRef {
        if base.is_null() {
            return null();
        }
        let styled =
            unsafe { CTFontCreateCopyWithSymbolicTraits(base, px_size, null(), traits, traits) };
        if !styled.is_null() {
            styled
        } else {
            // Retain a second copy by re-creating with no trait mask
            unsafe { CTFontCreateCopyWithSymbolicTraits(base, px_size, null(), 0, 0) }
        }
    }

    /// Renders `surface` into `self.pixels` at `(pixel_w, pixel_h)` using geometric box-drawing and
    /// CoreText `CTFontDrawGlyphs`, returning a retained `CGImageRef` (caller must call `CGImageRelease`).
    pub fn render_frame_cgimage(
        &mut self,
        pixel_w: u32,
        pixel_h: u32,
        surface: &GuiFrameSurface,
        default_bg_rgba: [u8; 4],
    ) -> CGImageRef {
        let fb_w = pixel_w.max(64);
        let fb_h = pixel_h.max(64);
        self.width_px = fb_w;
        self.height_px = fb_h;
        let total_px = (fb_w * fb_h) as usize;
        let bg_u32 = 0xFF00_0000
            | (u32::from(default_bg_rgba[0]) << 16)
            | (u32::from(default_bg_rgba[1]) << 8)
            | u32::from(default_bg_rgba[2]);

        self.pixels.resize(total_px, bg_u32);
        self.pixels.fill(bg_u32);

        // Pass 1: Background & underline quads
        for q in &surface.quads {
            let x0 = (q.x_px.max(0.0) as u32).min(fb_w);
            let y0 = (q.y_px.max(0.0) as u32).min(fb_h);
            let x1 = ((q.x_px + q.width_px).max(0.0) as u32).min(fb_w);
            let y1 = ((q.y_px + q.height_px).max(0.0) as u32).min(fb_h);
            let col = 0xFF00_0000
                | (u32::from(q.rgba[0]) << 16)
                | (u32::from(q.rgba[1]) << 8)
                | u32::from(q.rgba[2]);
            for py in y0..y1 {
                let row_off = (py * fb_w) as usize;
                self.pixels[row_off + x0 as usize..row_off + x1 as usize].fill(col);
            }
        }

        // Pass 2: Full-resolution Kitty/Sixel/PixelCanvas RGBA images
        for img in &surface.images {
            let src_w = u32::from(img.pixel_width.max(1));
            let src_h = u32::from(img.pixel_height.max(1));
            let dst_w = img.width_px.max(1);
            let dst_h = img.height_px.max(1);
            for dy in 0..dst_h {
                let py = img.y_px + dy;
                if py >= fb_h {
                    break;
                }
                let sy = ((dy * src_h) / dst_h).min(src_h - 1);
                for dx in 0..dst_w {
                    let px = img.x_px + dx;
                    if px >= fb_w {
                        break;
                    }
                    let sx = ((dx * src_w) / dst_w).min(src_w - 1);
                    let s_off = ((sy * src_w + sx) * 4) as usize;
                    if s_off + 3 < img.rgba_bytes.len() && img.rgba_bytes[s_off + 3] > 0 {
                        let r = u32::from(img.rgba_bytes[s_off]);
                        let g = u32::from(img.rgba_bytes[s_off + 1]);
                        let b = u32::from(img.rgba_bytes[s_off + 2]);
                        self.pixels[(py * fb_w + px) as usize] =
                            0xFF00_0000 | (r << 16) | (g << 8) | b;
                    }
                }
            }
        }

        // Pass 3: Geometric box-drawing, block elements & Braille
        let default_cw = u32::from(surface.cell_width_px.max(4));
        let default_ch = u32::from(surface.cell_height_px.max(8));
        for g in &surface.glyphs {
            let Some(ch) = g.text.chars().next() else {
                continue;
            };
            if is_geometric_glyph(ch) {
                let cw = if g.cell_width_px > 0 {
                    u32::from(g.cell_width_px)
                } else {
                    default_cw
                };
                let ch_px = if g.cell_height_px > 0 {
                    u32::from(g.cell_height_px)
                } else {
                    default_ch
                };
                draw_geometric_cell_bgra(
                    &mut self.pixels,
                    fb_w,
                    fb_h,
                    g.x_px.max(0.0) as u32,
                    g.y_px.max(0.0) as u32,
                    cw,
                    ch_px,
                    ch,
                    g.fg_rgba,
                );
            }
        }

        // Pass 4: Native CoreText TrueType/OpenType glyph rendering
        let color_space = unsafe { CGColorSpaceCreateDeviceRGB() };
        let bitmap_info = K_CG_IMAGE_ALPHA_NONE_SKIP_FIRST | K_CG_BITMAP_BYTE_ORDER_32_LITTLE;
        let ctx = unsafe {
            CGBitmapContextCreate(
                self.pixels.as_mut_ptr().cast::<c_void>(),
                fb_w as usize,
                fb_h as usize,
                8,
                (fb_w as usize) * 4,
                color_space,
                bitmap_info,
            )
        };

        if !ctx.is_null() {
            unsafe {
                CGContextSetShouldAntialias(ctx, true);
                CGContextSetShouldSmoothFonts(ctx, true);
            }
            let mut utf16_buf: Vec<u16> = Vec::with_capacity(4);
            let mut glyph_buf: Vec<CGGlyph> = Vec::with_capacity(4);
            let mut pos_buf: Vec<CGPoint> = Vec::with_capacity(4);
            let mut last_rgba = [0u8; 4];
            let mut has_color = false;

            for g in &surface.glyphs {
                let Some(ch) = g.text.chars().next() else {
                    continue;
                };
                if ch == ' ' || is_geometric_glyph(ch) {
                    continue;
                }
                let base_font = match (g.bold, g.italic) {
                    (false, false) => self.ct_regular,
                    (true, false) => self.ct_bold,
                    (false, true) => self.ct_italic,
                    (true, true) => self.ct_bold_italic,
                };
                let font = if !base_font.is_null() {
                    base_font
                } else {
                    self.ct_regular
                };
                if font.is_null() {
                    continue;
                }

                if !has_color || g.fg_rgba != last_rgba {
                    unsafe {
                        CGContextSetRGBFillColor(
                            ctx,
                            f64::from(g.fg_rgba[0]) / 255.0,
                            f64::from(g.fg_rgba[1]) / 255.0,
                            f64::from(g.fg_rgba[2]) / 255.0,
                            1.0,
                        );
                    }
                    last_rgba = g.fg_rgba;
                    has_color = true;
                }

                utf16_buf.clear();
                utf16_buf.extend(g.text.encode_utf16());
                let count = utf16_buf.len();
                if count == 0 {
                    continue;
                }
                glyph_buf.resize(count, 0);
                let baseline_y = (f64::from(fb_h) - (f64::from(g.y_px.max(0.0)) + self.ascent_px))
                    .max(0.0);
                let x = f64::from(g.x_px.max(0.0));
                pos_buf.clear();
                pos_buf.resize(count, CGPoint { x, y: baseline_y });

                let pos = CGPoint { x, y: baseline_y };
                let found = unsafe {
                    CTFontGetGlyphsForCharacters(
                        font,
                        utf16_buf.as_ptr(),
                        glyph_buf.as_mut_ptr(),
                        count as isize,
                    )
                };
                if found && glyph_buf[0] != 0 {
                    unsafe {
                        CTFontDrawGlyphs(font, glyph_buf.as_ptr(), pos_buf.as_ptr(), 1, ctx);
                    }
                } else if !try_draw_with_coretext_fallback(font, &g.text, pos, ctx) {
                    let mapped = map_pua_icon_fallback(ch);
                    if is_geometric_glyph(mapped) {
                        let cw = if g.cell_width_px > 0 {
                            u32::from(g.cell_width_px)
                        } else {
                            default_cw
                        };
                        let ch_px = if g.cell_height_px > 0 {
                            u32::from(g.cell_height_px)
                        } else {
                            default_ch
                        };
                        draw_geometric_cell_bgra(
                            &mut self.pixels,
                            fb_w,
                            fb_h,
                            g.x_px.max(0.0) as u32,
                            g.y_px.max(0.0) as u32,
                            cw,
                            ch_px,
                            mapped,
                            g.fg_rgba,
                        );
                    } else {
                        let mut buf_s = [0u8; 4];
                        let mapped_str = mapped.encode_utf8(&mut buf_s);
                        if !try_draw_with_coretext_fallback(font, mapped_str, pos, ctx) {
                            let _ = try_draw_with_coretext_fallback(font, "•", pos, ctx);
                        }
                    }
                }
            }
        }

        // Pass 5: Optional CRT phosphor scanlines
        if surface.crt_scanlines {
            for py in (1..fb_h).step_by(2) {
                let row_off = (py * fb_w) as usize;
                for px in 0..fb_w as usize {
                    let c = self.pixels[row_off + px];
                    let r = (((c >> 16) & 0xFF) * 3) / 4;
                    let g = (((c >> 8) & 0xFF) * 3) / 4;
                    let b = ((c & 0xFF) * 3) / 4;
                    self.pixels[row_off + px] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                }
            }
        }

        let cg_image = if !ctx.is_null() {
            unsafe { CGBitmapContextCreateImage(ctx) }
        } else {
            null_mut()
        };

        unsafe {
            if !ctx.is_null() {
                CGContextRelease(ctx);
            }
            if !color_space.is_null() {
                CGColorSpaceRelease(color_space);
            }
        }

        cg_image
    }
}

impl Drop for MacosGuiCanvas {
    fn drop(&mut self) {
        self.destroy_fonts();
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::*;
    use crate::ui::renderer::GuiWorkspaceRenderer;

    #[test]
    fn coretext_canvas_measures_font_and_renders_cgimage() {
        let wm = WindowManager::new_headless(24, 80);
        let mut canvas = MacosGuiCanvas::new(&wm, 2.0);
        assert!(canvas.cell_width_px >= 6);
        assert!(canvas.cell_height_px >= 12);
        assert!(!canvas.resolved_face_name.is_empty());

        let mut renderer = GuiWorkspaceRenderer::new(canvas.cell_width_px, canvas.cell_height_px);
        let mut surface = GuiFrameSurface::new(canvas.cell_width_px, canvas.cell_height_px);
        let vp = Rect::new(0, 0, 80, 24);
        renderer.render_workspace_with_cell_size(
            &wm,
            vp,
            canvas.cell_width_px,
            canvas.cell_height_px,
            &mut surface,
        );
        let w = u32::from(vp.width) * u32::from(canvas.cell_width_px);
        let h = u32::from(vp.height) * u32::from(canvas.cell_height_px);
        let img = canvas.render_frame_cgimage(w, h, &surface, [0, 0, 0x84, 255]);
        assert!(!img.is_null());
        unsafe {
            CGImageRelease(img);
        }
    }
}
