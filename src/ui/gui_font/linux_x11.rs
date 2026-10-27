#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::{CStr, CString, c_char, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_ushort, c_void};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::geometric::{draw_geometric_cell_bgra, is_geometric_glyph, resolve_candidate_font_faces};
use crate::action::Action;
use crate::app::App;
use crate::error::Result;
use crate::pane::typography::{
    FontFamilyPreset, default_custom_fonts_dir, register_custom_font_file,
    registered_custom_fonts, scan_and_register_fonts_dir,
};
use crate::ui::gui_window::pixel_to_cell_coords;
use crate::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};
use crate::wm::WindowManager;

const KEY_PRESS_MASK: c_long = 1 << 0;
const BUTTON_PRESS_MASK: c_long = 1 << 2;
const BUTTON_RELEASE_MASK: c_long = 1 << 3;
const POINTER_MOTION_MASK: c_long = 1 << 6;
const EXPOSURE_MASK: c_long = 1 << 15;
const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;

const KEY_PRESS: c_int = 2;
const BUTTON_PRESS: c_int = 4;
const BUTTON_RELEASE: c_int = 5;
const MOTION_NOTIFY: c_int = 6;
const EXPOSE: c_int = 12;
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;
const Z_PIXMAP: c_int = 2;

const SHIFT_MASK: c_uint = 1 << 0;
const CONTROL_MASK: c_uint = 1 << 2;
const MOD1_MASK: c_uint = 1 << 3;

#[repr(C)]
struct XftFont {
    ascent: c_int,
    descent: c_int,
    height: c_int,
    max_advance_width: c_int,
    charset: *mut c_void,
    pattern: *mut c_void,
}

#[repr(C)]
#[derive(Default)]
struct XGlyphInfo {
    width: c_ushort,
    height: c_ushort,
    x: c_short,
    y: c_short,
    x_off: c_short,
    y_off: c_short,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XRenderColor {
    red: c_ushort,
    green: c_ushort,
    blue: c_ushort,
    alpha: c_ushort,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XftColor {
    pixel: c_ulong,
    color: XRenderColor,
}

#[repr(C)]
struct XImage {
    width: c_int,
    height: c_int,
    xoffset: c_int,
    format: c_int,
    data: *mut c_char,
    byte_order: c_int,
    bitmap_unit: c_int,
    bitmap_bit_order: c_int,
    bitmap_pad: c_int,
    depth: c_int,
    bytes_per_line: c_int,
    bits_per_pixel: c_int,
    red_mask: c_ulong,
    green_mask: c_ulong,
    blue_mask: c_ulong,
    obdata: *mut c_char,
    f_pad: [*mut c_void; 6],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XKeyEvent {
    type_: c_int, serial: c_ulong, send_event: c_int, display: *mut c_void,
    window: c_ulong, root: c_ulong, subwindow: c_ulong, time: c_ulong,
    x: c_int, y: c_int, x_root: c_int, y_root: c_int,
    state: c_uint, keycode: c_uint, same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XButtonEvent {
    type_: c_int, serial: c_ulong, send_event: c_int, display: *mut c_void,
    window: c_ulong, root: c_ulong, subwindow: c_ulong, time: c_ulong,
    x: c_int, y: c_int, x_root: c_int, y_root: c_int,
    state: c_uint, button: c_uint, same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XMotionEvent {
    type_: c_int, serial: c_ulong, send_event: c_int, display: *mut c_void,
    window: c_ulong, root: c_ulong, subwindow: c_ulong, time: c_ulong,
    x: c_int, y: c_int, x_root: c_int, y_root: c_int,
    state: c_uint, is_hint: c_char, same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XConfigureEvent {
    type_: c_int, serial: c_ulong, send_event: c_int, display: *mut c_void,
    event_window: c_ulong, window: c_ulong, x: c_int, y: c_int,
    width: c_int, height: c_int, border_width: c_int, above: c_ulong, override_redirect: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XClientMessageEvent {
    type_: c_int, serial: c_ulong, send_event: c_int, display: *mut c_void,
    window: c_ulong, message_type: c_ulong, format: c_int, data_l: [c_long; 5],
}

#[repr(C)]
union XEvent {
    type_: c_int,
    key: XKeyEvent,
    button: XButtonEvent,
    motion: XMotionEvent,
    configure: XConfigureEvent,
    client_message: XClientMessageEvent,
    pad: [c_long; 24],
}

type FnCreateWin = unsafe extern "C" fn(*mut c_void, c_ulong, c_int, c_int, c_uint, c_uint, c_uint, c_ulong, c_ulong) -> c_ulong;
type FnCreateImg = unsafe extern "C" fn(*mut c_void, *mut c_void, c_uint, c_int, c_int, *mut c_char, c_uint, c_uint, c_int, c_int) -> *mut XImage;
type FnPutImg = unsafe extern "C" fn(*mut c_void, c_ulong, *mut c_void, *mut XImage, c_int, c_int, c_int, c_int, c_uint, c_uint) -> c_int;
type FnCopyArea = unsafe extern "C" fn(*mut c_void, c_ulong, c_ulong, *mut c_void, c_int, c_int, c_uint, c_uint, c_int, c_int) -> c_int;
type FnLookupStr = unsafe extern "C" fn(*mut XKeyEvent, *mut c_char, c_int, *mut c_ulong, *mut c_void) -> c_int;
type FnXftDrawStr = unsafe extern "C" fn(*mut c_void, *const XftColor, *mut XftFont, c_int, c_int, *const c_uchar, c_int);

pub struct X11Api {
    _lib_x11: *mut c_void,
    _lib_xft: *mut c_void,
    _lib_fc: *mut c_void,
    x_open_display: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    x_close_display: unsafe extern "C" fn(*mut c_void) -> c_int,
    x_default_screen: unsafe extern "C" fn(*mut c_void) -> c_int,
    x_default_root_window: unsafe extern "C" fn(*mut c_void) -> c_ulong,
    x_default_visual: unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void,
    x_default_colormap: unsafe extern "C" fn(*mut c_void, c_int) -> c_ulong,
    x_default_depth: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    x_default_gc: unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void,
    x_create_simple_window: FnCreateWin,
    x_destroy_window: unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int,
    x_store_name: unsafe extern "C" fn(*mut c_void, c_ulong, *const c_char) -> c_int,
    x_select_input: unsafe extern "C" fn(*mut c_void, c_ulong, c_long) -> c_int,
    x_map_window: unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int,
    x_intern_atom: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_ulong,
    x_set_wm_protocols: unsafe extern "C" fn(*mut c_void, c_ulong, *mut c_ulong, c_int) -> c_int,
    x_pending: unsafe extern "C" fn(*mut c_void) -> c_int,
    x_next_event: unsafe extern "C" fn(*mut c_void, *mut XEvent) -> c_int,
    x_lookup_string: FnLookupStr,
    x_create_pixmap: unsafe extern "C" fn(*mut c_void, c_ulong, c_uint, c_uint, c_uint) -> c_ulong,
    x_free_pixmap: unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int,
    x_create_image: FnCreateImg,
    x_free: unsafe extern "C" fn(*mut c_void) -> c_int,
    x_put_image: FnPutImg,
    x_copy_area: FnCopyArea,
    x_flush: unsafe extern "C" fn(*mut c_void) -> c_int,
    xft_font_open_name: Option<unsafe extern "C" fn(*mut c_void, c_int, *const c_char) -> *mut XftFont>,
    xft_font_close: Option<unsafe extern "C" fn(*mut c_void, *mut XftFont)>,
    xft_text_extents_utf8: Option<unsafe extern "C" fn(*mut c_void, *mut XftFont, *const c_uchar, c_int, *mut XGlyphInfo)>,
    xft_draw_create: Option<unsafe extern "C" fn(*mut c_void, c_ulong, *mut c_void, c_ulong) -> *mut c_void>,
    xft_draw_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    xft_draw_string_utf8: Option<FnXftDrawStr>,
    fc_config_app_font_add_file: Option<unsafe extern "C" fn(*mut c_void, *const c_uchar) -> c_int>,
    fc_pattern_get_string: Option<unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut *const c_char) -> c_int>,
}

unsafe fn load_sym<T: Copy>(lib: *mut c_void, name: &[u8]) -> Option<T> {
    if lib.is_null() {
        return None;
    }
    let sym = unsafe { libc::dlsym(lib, name.as_ptr().cast()) };
    (!sym.is_null()).then(|| unsafe { std::mem::transmute_copy(&sym) })
}

impl X11Api {
    fn load() -> Option<Self> {
        let lib_x11 = unsafe { libc::dlopen(c"libX11.so.6".as_ptr(), libc::RTLD_LAZY) };
        if lib_x11.is_null() {
            return None;
        }
        let lib_xft = unsafe { libc::dlopen(c"libXft.so.2".as_ptr(), libc::RTLD_LAZY) };
        let lib_fc = unsafe { libc::dlopen(c"libfontconfig.so.1".as_ptr(), libc::RTLD_LAZY) };
        unsafe {
            Some(Self {
                _lib_x11: lib_x11,
                _lib_xft: lib_xft,
                _lib_fc: lib_fc,
                x_open_display: load_sym(lib_x11, b"XOpenDisplay\0")?,
                x_close_display: load_sym(lib_x11, b"XCloseDisplay\0")?,
                x_default_screen: load_sym(lib_x11, b"XDefaultScreen\0")?,
                x_default_root_window: load_sym(lib_x11, b"XDefaultRootWindow\0")?,
                x_default_visual: load_sym(lib_x11, b"XDefaultVisual\0")?,
                x_default_colormap: load_sym(lib_x11, b"XDefaultColormap\0")?,
                x_default_depth: load_sym(lib_x11, b"XDefaultDepth\0")?,
                x_default_gc: load_sym(lib_x11, b"XDefaultGC\0")?,
                x_create_simple_window: load_sym(lib_x11, b"XCreateSimpleWindow\0")?,
                x_destroy_window: load_sym(lib_x11, b"XDestroyWindow\0")?,
                x_store_name: load_sym(lib_x11, b"XStoreName\0")?,
                x_select_input: load_sym(lib_x11, b"XSelectInput\0")?,
                x_map_window: load_sym(lib_x11, b"XMapWindow\0")?,
                x_intern_atom: load_sym(lib_x11, b"XInternAtom\0")?,
                x_set_wm_protocols: load_sym(lib_x11, b"XSetWMProtocols\0")?,
                x_pending: load_sym(lib_x11, b"XPending\0")?,
                x_next_event: load_sym(lib_x11, b"XNextEvent\0")?,
                x_lookup_string: load_sym(lib_x11, b"XLookupString\0")?,
                x_create_pixmap: load_sym(lib_x11, b"XCreatePixmap\0")?,
                x_free_pixmap: load_sym(lib_x11, b"XFreePixmap\0")?,
                x_create_image: load_sym(lib_x11, b"XCreateImage\0")?,
                x_free: load_sym(lib_x11, b"XFree\0")?,
                x_put_image: load_sym(lib_x11, b"XPutImage\0")?,
                x_copy_area: load_sym(lib_x11, b"XCopyArea\0")?,
                x_flush: load_sym(lib_x11, b"XFlush\0")?,
                xft_font_open_name: load_sym(lib_xft, b"XftFontOpenName\0"),
                xft_font_close: load_sym(lib_xft, b"XftFontClose\0"),
                xft_text_extents_utf8: load_sym(lib_xft, b"XftTextExtentsUtf8\0"),
                xft_draw_create: load_sym(lib_xft, b"XftDrawCreate\0"),
                xft_draw_destroy: load_sym(lib_xft, b"XftDrawDestroy\0"),
                xft_draw_string_utf8: load_sym(lib_xft, b"XftDrawStringUtf8\0"),
                fc_config_app_font_add_file: load_sym(lib_fc, b"FcConfigAppFontAddFile\0"),
                fc_pattern_get_string: load_sym(lib_fc, b"FcPatternGetString\0"),
            })
        }
    }
}

pub struct X11GuiCanvas<'a> {
    api: &'a X11Api,
    dpy: *mut c_void,
    screen: c_int,
    visual: *mut c_void,
    colormap: c_ulong,
    depth: c_int,
    gc: *mut c_void,
    win: c_ulong,
    pixmap: c_ulong,
    xft_draw: *mut c_void,
    pixels: Vec<u32>,
    width_px: u32,
    height_px: u32,
    font_regular: *mut XftFont,
    font_bold: *mut XftFont,
    font_italic: *mut XftFont,
    font_bold_italic: *mut XftFont,
    font_ascent: i32,
    pub cell_width_px: u16,
    pub cell_height_px: u16,
    pub resolved_face_name: String,
    active_family_req: String,
    active_px_height: i32,
    loaded_font_paths: Vec<PathBuf>,
    scaled_fonts: std::collections::BTreeMap<(i32, bool, bool), (*mut XftFont, i32)>,
}

impl<'a> X11GuiCanvas<'a> {
    pub fn new(api: &'a X11Api, dpy: *mut c_void, win: c_ulong, wm: &WindowManager) -> Self {
        let screen = unsafe { (api.x_default_screen)(dpy) };
        let visual = unsafe { (api.x_default_visual)(dpy, screen) };
        let colormap = unsafe { (api.x_default_colormap)(dpy, screen) };
        let depth = unsafe { (api.x_default_depth)(dpy, screen) };
        let gc = unsafe { (api.x_default_gc)(dpy, screen) };
        let mut canvas = Self {
            api, dpy, screen, visual, colormap, depth, gc, win,
            pixmap: 0, xft_draw: null_mut(), pixels: Vec::new(), width_px: 0, height_px: 0,
            font_regular: null_mut(), font_bold: null_mut(), font_italic: null_mut(), font_bold_italic: null_mut(),
            font_ascent: 14,
            cell_width_px: wm.config().cell_width_px.max(8),
            cell_height_px: wm.config().cell_height_px.max(16),
            resolved_face_name: "Monospace".to_string(),
            active_family_req: String::new(),
            active_px_height: 0,
            loaded_font_paths: Vec::new(),
            scaled_fonts: std::collections::BTreeMap::new(),
        };
        if let Some(dir) = default_custom_fonts_dir() {
            let _ = scan_and_register_fonts_dir(&dir);
        }
        for path_str in &wm.config().font_files {
            let _ = register_custom_font_file(std::path::Path::new(path_str));
        }
        canvas.sync_font(wm);
        canvas
    }

    fn sync_private_font_files(&mut self) {
        let Some(add_fn) = self.api.fc_config_app_font_add_file else { return };
        for (_, path) in registered_custom_fonts() {
            if !self.loaded_font_paths.contains(&path) {
                if let Some(s) = path.to_str() && let Ok(cs) = CString::new(s) {
                    unsafe { let _ = add_fn(null_mut(), cs.as_ptr().cast()); }
                }
                self.loaded_font_paths.push(path);
            }
        }
    }

    fn destroy_fonts(&mut self) {
        if let Some(close_fn) = self.api.xft_font_close {
            let base = [self.font_regular, self.font_bold, self.font_italic, self.font_bold_italic];
            for f in base.into_iter().chain(self.scaled_fonts.values().map(|&(f, _)| f)) {
                if !f.is_null() { unsafe { close_fn(self.dpy, f) }; }
            }
        }
        self.scaled_fonts.clear();
        (self.font_regular, self.font_bold, self.font_italic, self.font_bold_italic) = (null_mut(), null_mut(), null_mut(), null_mut());
    }

    fn open_xft_font(&self, family: &str, px_height: i32, bold: bool, italic: bool) -> *mut XftFont {
        let Some(open_fn) = self.api.xft_font_open_name else { return null_mut() };
        let weight = if bold { ":weight=bold" } else { ":weight=regular" };
        let slant = if italic { ":slant=italic" } else { ":slant=roman" };
        let spec = format!("{family}:pixelsize={px_height}{weight}{slant}:antialias=true:hinting=true");
        CString::new(spec).ok().map_or(null_mut(), |cs| unsafe { open_fn(self.dpy, self.screen, cs.as_ptr()) })
    }

    fn query_font_family_name(&self, font: *mut XftFont) -> Option<String> {
        let get_str = self.api.fc_pattern_get_string?;
        if font.is_null() || unsafe { (*font).pattern }.is_null() {
            return None;
        }
        let mut s_ptr: *const c_char = null();
        let rc = unsafe { get_str((*font).pattern, c"family".as_ptr(), 0, &mut s_ptr) };
        (rc == 0 && !s_ptr.is_null()).then(|| unsafe { CStr::from_ptr(s_ptr) }.to_string_lossy().into_owned())
    }

    pub fn sync_font(&mut self, wm: &WindowManager) -> (u16, u16, bool) {
        self.sync_private_font_files();
        let pane_fam = &wm.active_pane().font_options().family;
        let desired_fam = if *pane_fam != FontFamilyPreset::Ibm3270 {
            pane_fam.display_name().to_string()
        } else if !wm.config().font_family.trim().is_empty() {
            wm.config().font_family.trim().to_string()
        } else {
            "JetBrains Mono".to_string()
        };

        let eff_pt = wm.config().font_size_pt.clamp(6, 72);
        let px_height = ((u32::from(eff_pt) * 96 + 36) / 72).clamp(11, 96) as i32;

        if desired_fam == self.active_family_req
            && px_height == self.active_px_height
            && (!self.font_regular.is_null() || self.api.xft_font_open_name.is_none())
        {
            return (self.cell_width_px, self.cell_height_px, false);
        }

        self.destroy_fonts();
        self.active_family_req = desired_fam.clone();
        self.active_px_height = px_height;

        if self.api.xft_font_open_name.is_none() {
            self.cell_width_px = (px_height as u16 / 2).max(8);
            self.cell_height_px = (px_height as u16).max(16);
            return (self.cell_width_px, self.cell_height_px, true);
        }

        let mut candidates = resolve_candidate_font_faces(&desired_fam);
        for fb in ["JetBrains Mono", "Fira Code", "DejaVu Sans Mono", "Ubuntu Mono", "Liberation Mono", "Noto Sans Mono", "monospace"] {
            if !candidates.iter().any(|c| c.eq_ignore_ascii_case(fb)) {
                candidates.push(fb.to_string());
            }
        }

        let mut chosen_face = "monospace".to_string();
        for cand in &candidates {
            let f_test = self.open_xft_font(cand, px_height, false, false);
            if !f_test.is_null() {
                let actual = self.query_font_family_name(f_test).unwrap_or_default();
                if let Some(close_fn) = self.api.xft_font_close {
                    unsafe { close_fn(self.dpy, f_test) };
                }
                if actual.eq_ignore_ascii_case(cand)
                    || actual.to_ascii_lowercase().starts_with(&cand.to_ascii_lowercase())
                    || cand == "monospace"
                {
                    chosen_face = if cand == "monospace" && !actual.is_empty() { actual } else { cand.clone() };
                    break;
                }
            }
        }

        self.font_regular = self.open_xft_font(&chosen_face, px_height, false, false);
        self.font_bold = self.open_xft_font(&chosen_face, px_height, true, false);
        self.font_italic = self.open_xft_font(&chosen_face, px_height, false, true);
        self.font_bold_italic = self.open_xft_font(&chosen_face, px_height, true, true);
        self.resolved_face_name = chosen_face;

        if !self.font_regular.is_null() {
            let (ascent, descent, height) = unsafe {
                ((*self.font_regular).ascent, (*self.font_regular).descent, (*self.font_regular).height)
            };
            self.font_ascent = ascent.max(1);
            let mut cw = (px_height / 2).max(6) as u16;
            if let Some(ext_fn) = self.api.xft_text_extents_utf8 {
                let mut ext = XGlyphInfo::default();
                unsafe { ext_fn(self.dpy, self.font_regular, b"M".as_ptr(), 1, &mut ext) };
                if ext.x_off > 0 {
                    cw = ext.x_off as u16;
                }
            }
            self.cell_width_px = cw.clamp(5, 64);
            self.cell_height_px = height.max(ascent + descent).clamp(10, 128) as u16;
        }
        (self.cell_width_px, self.cell_height_px, true)
    }

    fn ensure_backbuffer(&mut self, width_px: u32, height_px: u32) {
        let (width_px, height_px) = (width_px.max(64), height_px.max(64));
        if self.width_px == width_px && self.height_px == height_px && self.pixmap != 0 {
            return;
        }
        if let Some(destroy_draw) = self.api.xft_draw_destroy && !self.xft_draw.is_null() {
            unsafe { destroy_draw(self.xft_draw) };
            self.xft_draw = null_mut();
        }
        if self.pixmap != 0 {
            unsafe { (self.api.x_free_pixmap)(self.dpy, self.pixmap) };
        }
        self.pixmap = unsafe { (self.api.x_create_pixmap)(self.dpy, self.win, width_px, height_px, self.depth as c_uint) };
        if let Some(create_draw) = self.api.xft_draw_create {
            self.xft_draw = unsafe { create_draw(self.dpy, self.pixmap, self.visual, self.colormap) };
        }
        self.width_px = width_px;
        self.height_px = height_px;
        self.pixels.resize((width_px * height_px) as usize, 0xFF00_0000);
    }

    pub fn present_frame(&mut self, surface: &GuiFrameSurface, default_bg_rgba: [u8; 4], win_w: u32, win_h: u32) {
        let surf_w = u32::from(surface.viewport_cells.width) * u32::from(surface.cell_width_px.max(4));
        let surf_h = u32::from(surface.viewport_cells.height) * u32::from(surface.cell_height_px.max(8));
        let (fb_w, fb_h) = (surf_w.max(win_w).max(64), surf_h.max(win_h).max(64));
        self.ensure_backbuffer(fb_w, fb_h);

        let bg_u32 = 0xFF00_0000 | (u32::from(default_bg_rgba[0]) << 16) | (u32::from(default_bg_rgba[1]) << 8) | u32::from(default_bg_rgba[2]);
        self.pixels.fill(bg_u32);

        for q in &surface.quads {
            let (x0, y0) = ((q.x_px.max(0.0) as u32).min(fb_w), (q.y_px.max(0.0) as u32).min(fb_h));
            let (x1, y1) = (((q.x_px + q.width_px).max(0.0) as u32).min(fb_w), ((q.y_px + q.height_px).max(0.0) as u32).min(fb_h));
            let col = 0xFF00_0000 | (u32::from(q.rgba[0]) << 16) | (u32::from(q.rgba[1]) << 8) | u32::from(q.rgba[2]);
            for py in y0..y1 {
                let row_off = (py * fb_w) as usize;
                self.pixels[row_off + x0 as usize..row_off + x1 as usize].fill(col);
            }
        }

        for img in &surface.images {
            let (src_w, src_h) = (u32::from(img.pixel_width.max(1)), u32::from(img.pixel_height.max(1)));
            let (dst_w, dst_h) = (img.width_px.max(1), img.height_px.max(1));
            for dy in 0..dst_h {
                let py = img.y_px + dy;
                if py >= fb_h { break; }
                let sy = ((dy * src_h) / dst_h).min(src_h - 1);
                for dx in 0..dst_w {
                    let px = img.x_px + dx;
                    if px >= fb_w { break; }
                    let sx = ((dx * src_w) / dst_w).min(src_w - 1);
                    let s_off = ((sy * src_w + sx) * 4) as usize;
                    if s_off + 3 < img.rgba_bytes.len() && img.rgba_bytes[s_off + 3] > 0 {
                        let (r, g, b) = (u32::from(img.rgba_bytes[s_off]), u32::from(img.rgba_bytes[s_off + 1]), u32::from(img.rgba_bytes[s_off + 2]));
                        self.pixels[(py * fb_w + px) as usize] = 0xFF00_0000 | (r << 16) | (g << 8) | b;
                    }
                }
            }
        }

        let (cell_w, cell_h) = (u32::from(surface.cell_width_px.max(4)), u32::from(surface.cell_height_px.max(8)));
        for g in &surface.glyphs {
            if let Some(ch) = g.text.chars().next() && is_geometric_glyph(ch) {
                let gw = u32::from(g.cell_width_px.max(4)).max(cell_w.min(4));
                let gh = u32::from(g.cell_height_px.max(8)).max(cell_h.min(8));
                draw_geometric_cell_bgra(&mut self.pixels, fb_w, fb_h, g.x_px.max(0.0) as u32, g.y_px.max(0.0) as u32, gw, gh, ch, g.fg_rgba);
            }
        }

        if surface.crt_scanlines {
            for py in (1..fb_h).step_by(2) {
                let row_off = (py * fb_w) as usize;
                for px in 0..fb_w as usize {
                    let c = self.pixels[row_off + px];
                    self.pixels[row_off + px] = 0xFF00_0000 | ((((c >> 16) & 0xFF) * 3 / 4) << 16) | ((((c >> 8) & 0xFF) * 3 / 4) << 8) | ((c & 0xFF) * 3 / 4);
                }
            }
        }

        let ximg = unsafe {
            (self.api.x_create_image)(self.dpy, self.visual, self.depth as c_uint, Z_PIXMAP, 0, self.pixels.as_mut_ptr().cast(), fb_w, fb_h, 32, (fb_w * 4) as c_int)
        };
        if !ximg.is_null() {
            unsafe {
                (self.api.x_put_image)(self.dpy, self.pixmap, self.gc, ximg, 0, 0, 0, 0, fb_w, fb_h);
                (*ximg).data = null_mut();
                (self.api.x_free)(ximg.cast());
            }
        }

        if let Some(draw_str) = self.api.xft_draw_string_utf8
            && !self.xft_draw.is_null()
            && !self.font_regular.is_null()
        {
            for g in &surface.glyphs {
                let Some(ch) = g.text.chars().next() else { continue };
                if ch == ' ' || is_geometric_glyph(ch) { continue; }
                let g_px_h = ((u32::from(g.font_size_pt.max(6)) * 96 + 36) / 72).clamp(9, 96) as i32;
                let (font, ascent) = if g_px_h == self.active_px_height {
                    let f = match (g.bold, g.italic) {
                        (false, false) => self.font_regular,
                        (true, false) if !self.font_bold.is_null() => self.font_bold,
                        (false, true) if !self.font_italic.is_null() => self.font_italic,
                        (true, true) if !self.font_bold_italic.is_null() => self.font_bold_italic,
                        _ => self.font_regular,
                    };
                    (f, self.font_ascent)
                } else if let Some(&cached) = self.scaled_fonts.get(&(g_px_h, g.bold, g.italic)) {
                    cached
                } else {
                    let f = self.open_xft_font(&self.resolved_face_name, g_px_h, g.bold, g.italic);
                    let (f_use, asc) = if !f.is_null() {
                        (f, unsafe { (*f).ascent }.max(1))
                    } else {
                        (self.font_regular, self.font_ascent)
                    };
                    self.scaled_fonts.insert((g_px_h, g.bold, g.italic), (f_use, asc));
                    (f_use, asc)
                };
                let (r, gr, b) = (g.fg_rgba[0], g.fg_rgba[1], g.fg_rgba[2]);
                let col = XftColor {
                    pixel: ((c_ulong::from(r)) << 16) | ((c_ulong::from(gr)) << 8) | c_ulong::from(b),
                    color: XRenderColor { red: u16::from(r) * 257, green: u16::from(gr) * 257, blue: u16::from(b) * 257, alpha: 0xFFFF },
                };
                let (x, y) = (g.x_px.max(0.0) as c_int, g.y_px.max(0.0) as c_int + ascent);
                unsafe { draw_str(self.xft_draw, &col, font, x, y, g.text.as_ptr(), g.text.len() as c_int) };
            }
        }

        unsafe {
            (self.api.x_copy_area)(self.dpy, self.pixmap, self.win, self.gc, 0, 0, win_w, win_h, 0, 0);
            (self.api.x_flush)(self.dpy);
        }
    }
}

impl Drop for X11GuiCanvas<'_> {
    fn drop(&mut self) {
        self.destroy_fonts();
        if let Some(destroy_draw) = self.api.xft_draw_destroy && !self.xft_draw.is_null() {
            unsafe { destroy_draw(self.xft_draw) };
        }
        if self.pixmap != 0 {
            unsafe { (self.api.x_free_pixmap)(self.dpy, self.pixmap) };
        }
    }
}

fn translate_x11_key_event(api: &X11Api, kev: &mut XKeyEvent) -> Option<KeyEvent> {
    let mut buf = [0 as c_char; 32];
    let mut keysym: c_ulong = 0;
    let n = unsafe { (api.x_lookup_string)(kev, buf.as_mut_ptr(), buf.len() as c_int - 1, &mut keysym, null_mut()) };
    let (ctrl, alt, shift) = ((kev.state & CONTROL_MASK) != 0, (kev.state & MOD1_MASK) != 0, (kev.state & SHIFT_MASK) != 0);
    let mut mods = KeyModifiers::empty();
    if ctrl { mods |= KeyModifiers::CONTROL; }
    if alt { mods |= KeyModifiers::ALT; }
    if shift { mods |= KeyModifiers::SHIFT; }

    let code = match keysym as u32 {
        0xFF08 => Some(KeyCode::Backspace),
        0xFF09 => Some(if shift { KeyCode::BackTab } else { KeyCode::Tab }),
        0xFE20 => Some(KeyCode::BackTab),
        0xFF0D | 0xFF8D => Some(KeyCode::Enter),
        0xFF1B => Some(KeyCode::Esc),
        0xFF50 => Some(KeyCode::Home),
        0xFF51 => Some(KeyCode::Left),
        0xFF52 => Some(KeyCode::Up),
        0xFF53 => Some(KeyCode::Right),
        0xFF54 => Some(KeyCode::Down),
        0xFF55 => Some(KeyCode::PageUp),
        0xFF56 => Some(KeyCode::PageDown),
        0xFF57 => Some(KeyCode::End),
        0xFF63 => Some(KeyCode::Insert),
        0xFFFF => Some(KeyCode::Delete),
        ks @ 0xFFBE..=0xFFC9 => Some(KeyCode::F((ks - 0xFFBE + 1) as u8)),
        ks if ctrl && (ks == b'+' as u32 || ks == b'=' as u32 || ks == 0xFFAB) => {
            return Some(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::CONTROL));
        }
        ks if ctrl && (ks == b'-' as u32 || ks == b'_' as u32 || ks == 0xFFAD) => {
            return Some(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::CONTROL));
        }
        ks if ctrl && (ks == b'0' as u32 || ks == b')' as u32 || ks == 0xFFB0 || ks == 0xFF9E) => {
            return Some(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::CONTROL));
        }
        ks @ 0x20..=0x7E if ctrl || alt => Some(KeyEvent::new(KeyCode::Char((ks as u8 as char).to_ascii_lowercase()), mods).code),
        _ => None,
    };
    if let Some(c) = code {
        return Some(KeyEvent::new(c, mods));
    }
    if n > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), n as usize) };
        if ctrl && bytes.len() == 1 && (1..=26).contains(&bytes[0]) {
            return Some(KeyEvent::new(KeyCode::Char((b'a' + bytes[0] - 1) as char), mods));
        }
        if let Ok(s) = std::str::from_utf8(bytes) && let Some(ch) = s.chars().next() && !ch.is_control() {
            return Some(KeyEvent::new(KeyCode::Char(ch), if alt { KeyModifiers::ALT } else { KeyModifiers::NONE }));
        }
    }
    None
}

/// Runs Bilterm in a native X11 + Xft/Fontconfig desktop GUI window on Linux/Unix (`GPU-02`).
pub fn run_x11_gui_window(mut app: App) -> Result<()> {
    let Some(api) = X11Api::load() else {
        let _ = crate::launcher::ensure_terminal_or_relaunch(&[])?;
        return Ok(());
    };
    let dpy = unsafe { (api.x_open_display)(null()) };
    if dpy.is_null() {
        let _ = crate::launcher::ensure_terminal_or_relaunch(&[])?;
        return Ok(());
    }

    let root = unsafe { (api.x_default_root_window)(dpy) };
    let (mut win_w, mut win_h) = (1120u32, 720u32);
    let win = unsafe { (api.x_create_simple_window)(dpy, root, 80, 60, win_w, win_h, 0, 0, 0x0000_0084) };
    let title = CString::new("Bilterm — IBM 3270 x Borland Turbo Vision (Native GUI)").unwrap();
    let mut wm_delete = unsafe { (api.x_intern_atom)(dpy, c"WM_DELETE_WINDOW".as_ptr(), 0) };
    unsafe {
        (api.x_store_name)(dpy, win, title.as_ptr());
        (api.x_set_wm_protocols)(dpy, win, &mut wm_delete, 1);
        (api.x_select_input)(dpy, win, KEY_PRESS_MASK | BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK | POINTER_MOTION_MASK | EXPOSURE_MASK | STRUCTURE_NOTIFY_MASK);
        (api.x_map_window)(dpy, win);
    }

    let mut x11_canvas = X11GuiCanvas::new(&api, dpy, win, &app.wm);
    let mut gui_renderer = GuiWorkspaceRenderer::new(x11_canvas.cell_width_px, x11_canvas.cell_height_px);
    let mut surface = GuiFrameSurface::new(x11_canvas.cell_width_px, x11_canvas.cell_height_px);

    let cols = (win_w / u32::from(x11_canvas.cell_width_px.max(1))).clamp(20, 500) as u16;
    let rows = (win_h / u32::from(x11_canvas.cell_height_px.max(1))).clamp(6, 200) as u16;
    let vp = Rect::new(0, 0, cols, rows);
    app.wm.resize_all(vp)?;
    app.last_viewport = vp;
    let font_pt = app.wm.config().scaled_font_size_pt();
    app.wm.active_pane_mut().set_status_message(Some(format!(
        "GUI Font: {} ({}pt Xft/FreeType) — :font <jetbrains|firacode|cascadia|vga|cycle|add> or Ctrl+Wheel",
        x11_canvas.resolved_face_name, font_pt
    )));
    app.needs_redraw = true;
    let mut left_down = false;

    loop {
        while unsafe { (api.x_pending)(dpy) } > 0 {
            let mut xev = XEvent { pad: [0; 24] };
            unsafe { (api.x_next_event)(dpy, &mut xev) };
            match unsafe { xev.type_ } {
                KEY_PRESS => {
                    let mut kev = unsafe { xev.key };
                    if let Some(key_event) = translate_x11_key_event(&api, &mut kev) {
                        app.handle_crossterm_event(Event::Key(key_event))?;
                        app.needs_redraw = true;
                    }
                }
                BUTTON_PRESS => {
                    let bev = unsafe { xev.button };
                    let (col, row) = pixel_to_cell_coords(bev.x, bev.y, x11_canvas.cell_width_px, x11_canvas.cell_height_px, app.last_viewport);
                    let ctrl = (bev.state & CONTROL_MASK) != 0;
                    match bev.button {
                        1 => {
                            left_down = true;
                            app.handle_crossterm_event(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: col, row, modifiers: KeyModifiers::empty() }))?;
                            app.needs_redraw = true;
                        }
                        3 => {
                            app.handle_crossterm_event(Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Right), column: col, row, modifiers: KeyModifiers::empty() }))?;
                            app.needs_redraw = true;
                        }
                        4 | 5 => {
                            let up = bev.button == 4;
                            if ctrl {
                                if let Some((crate::wm::Focus::Pane(id), _, _)) = app.wm.hit_test_pane(col, row, app.last_viewport) {
                                    app.wm.focus_and_raise(id);
                                }
                                let _ = app.wm.apply(if up { Action::FontZoomIn } else { Action::FontZoomOut });
                            } else {
                                let kind = if up { MouseEventKind::ScrollUp } else { MouseEventKind::ScrollDown };
                                app.handle_crossterm_event(Event::Mouse(MouseEvent { kind, column: col, row, modifiers: KeyModifiers::empty() }))?;
                            }
                            app.needs_redraw = true;
                        }
                        _ => {}
                    }
                }
                BUTTON_RELEASE => {
                    let bev = unsafe { xev.button };
                    if bev.button == 1 {
                        left_down = false;
                        let (col, row) = pixel_to_cell_coords(bev.x, bev.y, x11_canvas.cell_width_px, x11_canvas.cell_height_px, app.last_viewport);
                        app.handle_crossterm_event(Event::Mouse(MouseEvent { kind: MouseEventKind::Up(MouseButton::Left), column: col, row, modifiers: KeyModifiers::empty() }))?;
                        app.needs_redraw = true;
                    }
                }
                MOTION_NOTIFY => {
                    let mev = unsafe { xev.motion };
                    let (col, row) = pixel_to_cell_coords(mev.x, mev.y, x11_canvas.cell_width_px, x11_canvas.cell_height_px, app.last_viewport);
                    let kind = if left_down { MouseEventKind::Drag(MouseButton::Left) } else { MouseEventKind::Moved };
                    app.handle_crossterm_event(Event::Mouse(MouseEvent { kind, column: col, row, modifiers: KeyModifiers::empty() }))?;
                    app.needs_redraw = true;
                }
                CONFIGURE_NOTIFY => {
                    let cev = unsafe { xev.configure };
                    let (nw, nh) = (cev.width.max(64) as u32, cev.height.max(64) as u32);
                    if nw != win_w || nh != win_h {
                        (win_w, win_h) = (nw, nh);
                        let c = (win_w / u32::from(x11_canvas.cell_width_px.max(1))).clamp(20, 500) as u16;
                        let r = (win_h / u32::from(x11_canvas.cell_height_px.max(1))).clamp(6, 200) as u16;
                        let new_vp = Rect::new(0, 0, c, r);
                        app.wm.resize_all(new_vp)?;
                        app.last_viewport = new_vp;
                        app.needs_redraw = true;
                    }
                }
                EXPOSE => app.needs_redraw = true,
                CLIENT_MESSAGE => {
                    let cme = unsafe { xev.client_message };
                    if cme.data_l[0] as c_ulong == wm_delete {
                        app.should_quit = true;
                    }
                }
                _ => {}
            }
        }

        if app.poll_all_ptys() {
            app.needs_redraw = true;
        }
        if app.should_quit || app.all_panes_exited() {
            break;
        }
        if app.needs_redraw {
            let (cw, ch, font_changed) = x11_canvas.sync_font(&app.wm);
            if font_changed {
                (gui_renderer.cell_width_px, gui_renderer.cell_height_px) = (cw, ch);
                let c = (win_w / u32::from(cw.max(1))).clamp(20, 500) as u16;
                let r = (win_h / u32::from(ch.max(1))).clamp(6, 200) as u16;
                let new_vp = Rect::new(0, 0, c, r);
                app.wm.resize_all(new_vp)?;
                app.last_viewport = new_vp;
            }
            gui_renderer.render_workspace_with_cell_size(&app.wm, app.last_viewport, cw, ch, &mut surface);
            let default_bg = match app.wm.config().palette().window_frame_bg {
                ratatui::style::Color::Rgb(r, g, b) => [r, g, b, 255],
                _ => [0, 0, 0x84, 255],
            };
            x11_canvas.present_frame(&surface, default_bg, win_w, win_h);
            app.needs_redraw = false;
        }
        std::thread::sleep(Duration::from_millis(16));
    }

    drop(x11_canvas);
    unsafe {
        (api.x_destroy_window)(dpy, win);
        (api.x_close_display)(dpy);
    }
    Ok(())
}
