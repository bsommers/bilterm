#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::path::PathBuf;
use std::ptr::{null, null_mut};

use super::geometric::{draw_geometric_cell_bgra, is_geometric_glyph, resolve_candidate_font_faces};
use crate::pane::typography::{
    FontFamilyPreset, default_custom_fonts_dir, register_custom_font_file,
    registered_custom_fonts, scan_and_register_fonts_dir,
};
use crate::ui::renderer::GuiFrameSurface;
use crate::wm::WindowManager;

pub type Hwnd = *mut core::ffi::c_void;
pub type Hdc = *mut core::ffi::c_void;
type Hbitmap = *mut core::ffi::c_void;
type Hfont = *mut core::ffi::c_void;
type Hgdio = *mut core::ffi::c_void;

#[repr(C)]
#[derive(Default)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct BitmapInfoHeader {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    bmi_header: BitmapInfoHeader,
    bmi_colors: [u32; 1],
}

#[repr(C)]
#[derive(Default)]
struct TextMetricW {
    tm_height: i32,
    tm_ascent: i32,
    tm_descent: i32,
    tm_internal_leading: i32,
    tm_external_leading: i32,
    tm_ave_char_width: i32,
    tm_max_char_width: i32,
    tm_weight: i32,
    tm_overhang: i32,
    tm_digitized_aspect_x: i32,
    tm_digitized_aspect_y: i32,
    tm_first_char: u16,
    tm_last_char: u16,
    tm_default_char: u16,
    tm_break_char: u16,
    tm_italic: u8,
    tm_underlined: u8,
    tm_struck_out: u8,
    tm_pitch_and_family: u8,
    tm_char_set: u8,
}

const LOGPIXELSY: i32 = 90;
const TRANSPARENT: i32 = 1;
const FW_NORMAL: i32 = 400;
const FW_BOLD: i32 = 700;
const DEFAULT_CHARSET: u32 = 1;
const OUT_TT_PRECIS: u32 = 4;
const CLIP_DEFAULT_PRECIS: u32 = 0;
const CLEARTYPE_NATURAL_QUALITY: u32 = 6;
const FIXED_PITCH: u32 = 1;
const FF_MODERN: u32 = 1 << 4;
const ETO_CLIPPED: u32 = 0x0004;
const SRCCOPY: u32 = 0x00CC_0020;
const FR_PRIVATE: u32 = 0x10;

#[link(name = "user32")]
unsafe extern "system" {
    fn SetProcessDPIAware() -> i32;
    fn GetDC(h_wnd: Hwnd) -> Hdc;
    fn ReleaseDC(h_wnd: Hwnd, h_dc: Hdc) -> i32;
    fn GetClientRect(h_wnd: Hwnd, lp_rect: *mut WinRect) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
    fn DeleteDC(hdc: Hdc) -> i32;
    fn CreateDIBSection(
        hdc: Hdc,
        pbmi: *const BitmapInfo,
        usage: u32,
        ppv_bits: *mut *mut core::ffi::c_void,
        h_section: *mut core::ffi::c_void,
        offset: u32,
    ) -> Hbitmap;
    fn SelectObject(hdc: Hdc, h: Hgdio) -> Hgdio;
    fn DeleteObject(ho: Hgdio) -> i32;
    fn CreateFontW(
        c_height: i32,
        c_width: i32,
        c_escapement: i32,
        c_orientation: i32,
        c_weight: i32,
        b_italic: u32,
        b_underline: u32,
        b_strike_out: u32,
        i_char_set: u32,
        i_out_precision: u32,
        i_clip_precision: u32,
        i_quality: u32,
        i_pitch_and_family: u32,
        psz_face_name: *const u16,
    ) -> Hfont;
    fn GetTextMetricsW(hdc: Hdc, lptm: *mut TextMetricW) -> i32;
    fn GetTextFaceW(hdc: Hdc, c: i32, lp_name: *mut u16) -> i32;
    fn SetBkMode(hdc: Hdc, mode: i32) -> i32;
    fn SetTextColor(hdc: Hdc, color: u32) -> u32;
    fn ExtTextOutW(
        hdc: Hdc,
        x: i32,
        y: i32,
        options: u32,
        lprect: *const WinRect,
        lp_string: *const u16,
        c: u32,
        lp_dx: *const i32,
    ) -> i32;
    fn BitBlt(
        hdc: Hdc,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        hdc_src: Hdc,
        x1: i32,
        y1: i32,
        rop: u32,
    ) -> i32;
    fn GdiFlush() -> i32;
    fn GetDeviceCaps(hdc: Hdc, index: i32) -> i32;
    fn AddFontResourceExW(name: *const u16, fl: u32, res: *mut core::ffi::c_void) -> i32;
}

/// Declares the process DPI-aware so Windows DWM renders at 1:1 physical pixels.
pub fn enable_hidpi_awareness() {
    unsafe {
        let _ = SetProcessDPIAware();
    }
}

/// Queries the monitor DPI (`96` = 100%, `120` = 125%, `144` = 150%, `192` = 200%).
pub fn query_window_dpi(hwnd: Hwnd) -> u32 {
    let hdc = unsafe { GetDC(hwnd) };
    if hdc.is_null() {
        return 96;
    }
    let dpi = unsafe { GetDeviceCaps(hdc, LOGPIXELSY) };
    unsafe {
        ReleaseDC(hwnd, hdc);
    }
    dpi.clamp(72, 384) as u32
}

/// Hardware ClearType + 1:1 DIBSection canvas for `bilterm --gui`.
pub struct Win32GuiCanvas {
    mem_dc: Hdc,
    dib_bitmap: Hbitmap,
    old_bitmap: Hgdio,
    dib_pixels: *mut u32,
    pub width_px: u32,
    pub height_px: u32,
    active_family_req: String,
    pub resolved_face_name: String,
    active_px_height: i32,
    pub cell_width_px: u16,
    pub cell_height_px: u16,
    hfont_regular: Hfont,
    hfont_bold: Hfont,
    hfont_italic: Hfont,
    hfont_bold_italic: Hfont,
    scaled_fonts: std::collections::BTreeMap<(i32, bool, bool), Hfont>,
    loaded_paths: Vec<PathBuf>,
}

impl Win32GuiCanvas {
    pub fn new(hwnd: Hwnd, wm: &WindowManager) -> Self {
        if let Some(dir) = default_custom_fonts_dir() {
            let _ = scan_and_register_fonts_dir(&dir);
        }
        for f in &wm.config().font_files {
            let _ = register_custom_font_file(std::path::Path::new(f));
        }
        let win_dc = unsafe { GetDC(hwnd) };
        let mem_dc = unsafe { CreateCompatibleDC(win_dc) };
        unsafe {
            if !win_dc.is_null() {
                ReleaseDC(hwnd, win_dc);
            }
        }
        let mut canvas = Self {
            mem_dc,
            dib_bitmap: null_mut(),
            old_bitmap: null_mut(),
            dib_pixels: null_mut(),
            width_px: 0,
            height_px: 0,
            active_family_req: String::new(),
            resolved_face_name: String::new(),
            active_px_height: 0,
            cell_width_px: 10,
            cell_height_px: 20,
            hfont_regular: null_mut(),
            hfont_bold: null_mut(),
            hfont_italic: null_mut(),
            hfont_bold_italic: null_mut(),
            scaled_fonts: std::collections::BTreeMap::new(),
            loaded_paths: Vec::new(),
        };
        let dpi = query_window_dpi(hwnd);
        let _ = canvas.sync_font(wm, dpi);
        canvas
    }

    fn sync_private_font_files(&mut self) {
        for (_, path) in registered_custom_fonts() {
            if !self.loaded_paths.contains(&path) {
                let mut wide: Vec<u16> = path.as_os_str().encode_wide_null();
                unsafe {
                    let _ = AddFontResourceExW(wide.as_mut_ptr(), FR_PRIVATE, null_mut());
                }
                self.loaded_paths.push(path);
            }
        }
    }

    fn destroy_fonts(&mut self) {
        for (_, h) in std::mem::take(&mut self.scaled_fonts) {
            if !h.is_null() {
                unsafe {
                    let _ = DeleteObject(h);
                }
            }
        }
        for h in [
            self.hfont_regular,
            self.hfont_bold,
            self.hfont_italic,
            self.hfont_bold_italic,
        ] {
            if !h.is_null() {
                unsafe {
                    let _ = DeleteObject(h);
                }
            }
        }
        self.hfont_regular = null_mut();
        self.hfont_bold = null_mut();
        self.hfont_italic = null_mut();
        self.hfont_bold_italic = null_mut();
    }

    /// Synchronizes the active ClearType `HFONT` handles with `wm`'s current font family,
    /// point size, scale percentage, and monitor DPI. Returns `(cell_width_px, cell_height_px, changed)`.
    pub fn sync_font(&mut self, wm: &WindowManager, dpi: u32) -> (u16, u16, bool) {
        self.sync_private_font_files();

        let pane_fam = &wm.active_pane().font_options().family;
        let desired_fam = if *pane_fam != FontFamilyPreset::Ibm3270 {
            pane_fam.display_name().to_string()
        } else if !wm.config().font_family.trim().is_empty() {
            wm.config().font_family.trim().to_string()
        } else {
            "Cascadia Mono".to_string()
        };

        let eff_pt = wm.config().font_size_pt.clamp(6, 72);
        let px_height = ((u32::from(eff_pt) * dpi + 36) / 72).clamp(11, 96) as i32;

        if desired_fam == self.active_family_req
            && px_height == self.active_px_height
            && !self.hfont_regular.is_null()
        {
            return (self.cell_width_px, self.cell_height_px, false);
        }

        self.destroy_fonts();
        self.active_family_req = desired_fam.clone();
        self.active_px_height = px_height;

        let candidates = resolve_candidate_font_faces(&desired_fam);
        let mut chosen_face = candidates
            .first()
            .cloned()
            .unwrap_or_else(|| "Consolas".to_string());

        for cand in &candidates {
            let h_test = Self::create_gdi_font(cand, px_height, false, false);
            if !h_test.is_null() {
                let prev = unsafe { SelectObject(self.mem_dc, h_test) };
                let mut actual_buf = [0u16; 64];
                let len = unsafe { GetTextFaceW(self.mem_dc, 63, actual_buf.as_mut_ptr()) };
                unsafe {
                    SelectObject(self.mem_dc, prev);
                    DeleteObject(h_test);
                }
                if len > 1 {
                    let actual = String::from_utf16_lossy(&actual_buf[..(len as usize - 1)]);
                    if actual.eq_ignore_ascii_case(cand)
                        || actual
                            .to_ascii_lowercase()
                            .starts_with(&cand.to_ascii_lowercase())
                    {
                        chosen_face = actual;
                        break;
                    }
                }
            }
        }

        self.resolved_face_name = chosen_face.clone();
        self.hfont_regular = Self::create_gdi_font(&chosen_face, px_height, false, false);
        self.hfont_bold = Self::create_gdi_font(&chosen_face, px_height, true, false);
        self.hfont_italic = Self::create_gdi_font(&chosen_face, px_height, false, true);
        self.hfont_bold_italic = Self::create_gdi_font(&chosen_face, px_height, true, true);

        let prev = unsafe { SelectObject(self.mem_dc, self.hfont_regular) };
        let mut tm = TextMetricW::default();
        unsafe {
            let _ = GetTextMetricsW(self.mem_dc, &mut tm);
            SelectObject(self.mem_dc, prev);
        }
        self.cell_width_px = tm.tm_ave_char_width.clamp(6, 64) as u16;
        self.cell_height_px =
            (tm.tm_height + tm.tm_external_leading.max(1)).clamp(12, 128) as u16;

        (self.cell_width_px, self.cell_height_px, true)
    }

    fn create_gdi_font(face: &str, px_height: i32, bold: bool, italic: bool) -> Hfont {
        let mut wide: Vec<u16> = face.encode_utf16().take(31).collect();
        wide.push(0);
        unsafe {
            CreateFontW(
                -px_height,
                0,
                0,
                0,
                if bold { FW_BOLD } else { FW_NORMAL },
                u32::from(italic),
                0,
                0,
                DEFAULT_CHARSET,
                OUT_TT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_NATURAL_QUALITY,
                FIXED_PITCH | FF_MODERN,
                wide.as_ptr(),
            )
        }
    }

    fn ensure_dib_size(&mut self, w: u32, h: u32) {
        let w = w.max(64);
        let h = h.max(64);
        if self.width_px == w && self.height_px == h && !self.dib_pixels.is_null() {
            return;
        }
        if !self.dib_bitmap.is_null() {
            unsafe {
                SelectObject(self.mem_dc, self.old_bitmap);
                DeleteObject(self.dib_bitmap);
            }
            self.dib_bitmap = null_mut();
            self.dib_pixels = null_mut();
        }
        let bmi = BitmapInfo {
            bmi_header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: w as i32,
                bi_height: -(h as i32), // top-down DIB
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: 0,
                bi_size_image: 0,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            bmi_colors: [0],
        };
        let mut bits: *mut core::ffi::c_void = null_mut();
        let hbm = unsafe { CreateDIBSection(self.mem_dc, &bmi, 0, &mut bits, null_mut(), 0) };
        if !hbm.is_null() && !bits.is_null() {
            self.old_bitmap = unsafe { SelectObject(self.mem_dc, hbm) };
            self.dib_bitmap = hbm;
            self.dib_pixels = bits.cast::<u32>();
            self.width_px = w;
            self.height_px = h;
        }
    }

    /// Renders `surface` onto the 1:1 `DIBSection` using geometric box-drawing + ClearType `ExtTextOutW`
    /// and presents it to `hwnd` via `BitBlt`.
    pub fn present_frame(
        &mut self,
        hwnd: Hwnd,
        surface: &GuiFrameSurface,
        default_bg_rgba: [u8; 4],
    ) {
        let mut client = WinRect::default();
        unsafe { GetClientRect(hwnd, &mut client) };
        let cw_win = (client.right - client.left).max(1) as u32;
        let ch_win = (client.bottom - client.top).max(1) as u32;
        self.ensure_dib_size(cw_win, ch_win);
        if self.dib_pixels.is_null() {
            return;
        }

        let fb_w = self.width_px;
        let fb_h = self.height_px;
        let total_px = (fb_w * fb_h) as usize;
        let bg_u32 = 0xFF00_0000
            | (u32::from(default_bg_rgba[0]) << 16)
            | (u32::from(default_bg_rgba[1]) << 8)
            | u32::from(default_bg_rgba[2]);

        {
            let pixels = unsafe { std::slice::from_raw_parts_mut(self.dib_pixels, total_px) };
            pixels.fill(bg_u32);

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
                    pixels[row_off + x0 as usize..row_off + x1 as usize].fill(col);
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
                            pixels[(py * fb_w + px) as usize] =
                                0xFF00_0000 | (r << 16) | (g << 8) | b;
                        }
                    }
                }
            }

            // Pass 3: Geometric box-drawing, block elements & Braille
            let cell_w = u32::from(surface.cell_width_px.max(4));
            let cell_h = u32::from(surface.cell_height_px.max(8));
            for g in &surface.glyphs {
                let Some(ch) = g.text.chars().next() else {
                    continue;
                };
                if is_geometric_glyph(ch) {
                    let gw = u32::from(g.cell_width_px.max(4)).max(cell_w.min(4));
                    let gh = u32::from(g.cell_height_px.max(8)).max(cell_h.min(8));
                    draw_geometric_cell_bgra(
                        pixels,
                        fb_w,
                        fb_h,
                        g.x_px.max(0.0) as u32,
                        g.y_px.max(0.0) as u32,
                        gw,
                        gh,
                        ch,
                        g.fg_rgba,
                    );
                }
            }
        }

        // Pass 4: Hardware ClearType TrueType text rendering via GDI ExtTextOutW
        let dpi = query_window_dpi(hwnd);
        unsafe {
            let _ = GdiFlush();
            let _ = SetBkMode(self.mem_dc, TRANSPARENT);
        }
        let cell_w_i = i32::from(surface.cell_width_px.max(4));
        let cell_h_i = i32::from(surface.cell_height_px.max(8));
        let mut cur_font_key = (self.active_px_height, false, false);
        let mut old_font = unsafe { SelectObject(self.mem_dc, self.hfont_regular) };
        let mut utf16_buf: Vec<u16> = Vec::with_capacity(8);

        for g in &surface.glyphs {
            let Some(ch) = g.text.chars().next() else {
                continue;
            };
            if ch == ' ' || is_geometric_glyph(ch) {
                continue;
            }
            let g_px_h = ((u32::from(g.font_size_pt.max(6)) * dpi + 36) / 72).clamp(9, 96) as i32;
            let key = (g_px_h, g.bold, g.italic);
            if key != cur_font_key {
                let hf = if g_px_h == self.active_px_height {
                    match (g.bold, g.italic) {
                        (false, false) => self.hfont_regular,
                        (true, false) => self.hfont_bold,
                        (false, true) => self.hfont_italic,
                        (true, true) => self.hfont_bold_italic,
                    }
                } else if let Some(&cached) = self.scaled_fonts.get(&key) {
                    cached
                } else {
                    let created = Self::create_gdi_font(&self.resolved_face_name, g_px_h, g.bold, g.italic);
                    let use_hf = if !created.is_null() { created } else { self.hfont_regular };
                    self.scaled_fonts.insert(key, use_hf);
                    use_hf
                };
                if !hf.is_null() {
                    unsafe {
                        SelectObject(self.mem_dc, hf);
                    }
                }
                cur_font_key = key;
            }
            let colorref = u32::from(g.fg_rgba[0])
                | (u32::from(g.fg_rgba[1]) << 8)
                | (u32::from(g.fg_rgba[2]) << 16);
            let x = g.x_px.max(0.0) as i32;
            let y = g.y_px.max(0.0) as i32;
            let gw_i = i32::from(g.cell_width_px.max(4)).max(cell_w_i.min(4));
            let gh_i = i32::from(g.cell_height_px.max(8)).max(cell_h_i.min(8));
            let clip = WinRect {
                left: x,
                top: y,
                right: x + gw_i * 2,
                bottom: y + gh_i,
            };
            utf16_buf.clear();
            utf16_buf.extend(g.text.encode_utf16());
            unsafe {
                SetTextColor(self.mem_dc, colorref);
                let _ = ExtTextOutW(
                    self.mem_dc,
                    x,
                    y,
                    ETO_CLIPPED,
                    &clip,
                    utf16_buf.as_ptr(),
                    utf16_buf.len() as u32,
                    null(),
                );
            }
        }
        unsafe {
            SelectObject(self.mem_dc, old_font);
            let _ = GdiFlush();
        }

        // Pass 5: Optional CRT phosphor scanlines
        if surface.crt_scanlines {
            let pixels = unsafe { std::slice::from_raw_parts_mut(self.dib_pixels, total_px) };
            for py in (1..fb_h).step_by(2) {
                let row_off = (py * fb_w) as usize;
                for px in 0..fb_w as usize {
                    let c = pixels[row_off + px];
                    let r = (((c >> 16) & 0xFF) * 3) / 4;
                    let g = (((c >> 8) & 0xFF) * 3) / 4;
                    let b = ((c & 0xFF) * 3) / 4;
                    pixels[row_off + px] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                }
            }
        }

        // Pass 6: 1:1 BitBlt to window client area (zero stretching)
        let win_dc = unsafe { GetDC(hwnd) };
        if !win_dc.is_null() {
            unsafe {
                let _ = BitBlt(
                    win_dc,
                    0,
                    0,
                    cw_win as i32,
                    ch_win as i32,
                    self.mem_dc,
                    0,
                    0,
                    SRCCOPY,
                );
                ReleaseDC(hwnd, win_dc);
            }
        }
        let _ = &mut old_font;
    }
}

impl Drop for Win32GuiCanvas {
    fn drop(&mut self) {
        self.destroy_fonts();
        if !self.dib_bitmap.is_null() {
            unsafe {
                SelectObject(self.mem_dc, self.old_bitmap);
                DeleteObject(self.dib_bitmap);
            }
        }
        if !self.mem_dc.is_null() {
            unsafe {
                DeleteDC(self.mem_dc);
            }
        }
    }
}

trait OsStrWideNull {
    fn encode_wide_null(&self) -> Vec<u16>;
}

impl OsStrWideNull for std::ffi::OsStr {
    fn encode_wide_null(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        let mut v: Vec<u16> = self.encode_wide().collect();
        v.push(0);
        v
    }
}
