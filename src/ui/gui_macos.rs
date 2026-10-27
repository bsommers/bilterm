//! Native macOS Cocoa / AppKit + CoreText + CoreGraphics GUI Window Runner (`bilterm --gui` / `Bilterm.app`, `GPU-02`).
//!
//! Drives [`App`] + [`GuiWorkspaceRenderer`] + [`GuiFrameSurface`] inside a standalone Retina-aware
//! macOS `NSWindow` backed by [`crate::ui::gui_font::macos_ct::MacosGuiCanvas`] (`CoreText` +
//! `CGBitmapContext` + `CALayer` + `CATransaction`), with full keyboard, `Option`-as-`Alt`,
//! `Cmd+V` clipboard paste, `Cmd++`/`Cmd+-`/`Ctrl+Wheel` font zoom, mouse drag/click/scroll,
//! and dynamic window resizing.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::null_mut;
use std::time::Duration;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::action::Action;
use crate::app::App;
use crate::error::Result;
use crate::ui::gui_font::macos_ct::{
    CGImageRelease, CGPoint, CGRect, CGSize, MacosGuiCanvas, cfstring_to_string, create_cfstring,
};
use crate::ui::gui_window::pixel_to_cell_coords;
use crate::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};

type Id = *mut c_void;
type Sel = *mut c_void;

const NS_APPLICATION_ACTIVATION_POLICY_REGULAR: isize = 0;
const NS_WINDOW_STYLE_MASK_TITLED: usize = 1 << 0;
const NS_WINDOW_STYLE_MASK_CLOSABLE: usize = 1 << 1;
const NS_WINDOW_STYLE_MASK_MINIATURIZABLE: usize = 1 << 2;
const NS_WINDOW_STYLE_MASK_RESIZABLE: usize = 1 << 3;
const NS_BACKING_STORE_BUFFERED: usize = 2;
const NS_EVENT_MASK_ANY: u64 = u64::MAX;

const NS_EVENT_TYPE_LEFT_MOUSE_DOWN: u64 = 1;
const NS_EVENT_TYPE_LEFT_MOUSE_UP: u64 = 2;
const NS_EVENT_TYPE_RIGHT_MOUSE_DOWN: u64 = 3;
const NS_EVENT_TYPE_RIGHT_MOUSE_UP: u64 = 4;
const NS_EVENT_TYPE_LEFT_MOUSE_DRAGGED: u64 = 6;
const NS_EVENT_TYPE_KEY_DOWN: u64 = 10;
const NS_EVENT_TYPE_KEY_UP: u64 = 11;
const NS_EVENT_TYPE_SCROLL_WHEEL: u64 = 22;
const NS_EVENT_TYPE_OTHER_MOUSE_DOWN: u64 = 25;
const NS_EVENT_TYPE_OTHER_MOUSE_UP: u64 = 26;

const NS_EVENT_MODIFIER_SHIFT: u64 = 1 << 17;
const NS_EVENT_MODIFIER_CONTROL: u64 = 1 << 18;
const NS_EVENT_MODIFIER_OPTION: u64 = 1 << 19;
const NS_EVENT_MODIFIER_COMMAND: u64 = 1 << 20;

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(str: *const c_char) -> Sel;
    fn objc_msgSend();
    #[cfg(target_arch = "x86_64")]
    fn objc_msgSend_stret();
}

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    static NSDefaultRunLoopMode: Id;
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {}

fn class(name: &CStr) -> Id {
    unsafe { objc_getClass(name.as_ptr()) }
}

fn sel(name: &CStr) -> Sel {
    unsafe { sel_registerName(name.as_ptr()) }
}

unsafe fn msg_send_id_0(obj: Id, s: Sel) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s) }
}

unsafe fn msg_send_void_0(obj: Id, s: Sel) {
    let f: unsafe extern "C" fn(Id, Sel) =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s) }
}

unsafe fn msg_send_void_id(obj: Id, s: Sel, a: Id) {
    let f: unsafe extern "C" fn(Id, Sel, Id) =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s, a) }
}

unsafe fn msg_send_void_bool(obj: Id, s: Sel, a: bool) {
    let f: unsafe extern "C" fn(Id, Sel, bool) =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s, a) }
}

unsafe fn msg_send_bool_isize(obj: Id, s: Sel, a: isize) -> u8 {
    let f: unsafe extern "C" fn(Id, Sel, isize) -> u8 =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s, a) }
}

unsafe fn msg_send_bool_0(obj: Id, s: Sel) -> u8 {
    let f: unsafe extern "C" fn(Id, Sel) -> u8 =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s) }
}

unsafe fn msg_send_u64_0(obj: Id, s: Sel) -> u64 {
    let f: unsafe extern "C" fn(Id, Sel) -> u64 =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s) }
}

unsafe fn msg_send_u16_0(obj: Id, s: Sel) -> u16 {
    let f: unsafe extern "C" fn(Id, Sel) -> u16 =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s) }
}

unsafe fn msg_send_f64_0(obj: Id, s: Sel) -> f64 {
    let f: unsafe extern "C" fn(Id, Sel) -> f64 =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s) }
}

unsafe fn msg_send_void_f64(obj: Id, s: Sel, a: f64) {
    let f: unsafe extern "C" fn(Id, Sel, f64) =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s, a) }
}

unsafe fn msg_send_point_0(obj: Id, s: Sel) -> CGPoint {
    let f: unsafe extern "C" fn(Id, Sel) -> CGPoint =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s) }
}

unsafe fn msg_send_rect_0(obj: Id, s: Sel) -> CGRect {
    #[cfg(target_arch = "aarch64")]
    {
        let f: unsafe extern "C" fn(Id, Sel) -> CGRect =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, s) }
    }
    #[cfg(target_arch = "x86_64")]
    {
        let f: unsafe extern "C" fn(*mut CGRect, Id, Sel) =
            unsafe { std::mem::transmute(objc_msgSend_stret as *const ()) };
        let mut out = CGRect::default();
        unsafe { f(&mut out, obj, s) };
        out
    }
}

unsafe fn msg_send_init_window(
    obj: Id,
    s: Sel,
    rect: CGRect,
    style: usize,
    backing: usize,
    defer: bool,
) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, CGRect, usize, usize, bool) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(obj, s, rect, style, backing, defer) }
}

unsafe fn msg_send_next_event(
    app: Id,
    s: Sel,
    mask: u64,
    until_date: Id,
    mode: Id,
    dequeue: bool,
) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, u64, Id, Id, bool) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(app, s, mask, until_date, mode, dequeue) }
}

/// Translated action produced by a macOS `NSEventTypeKeyDown` event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacGuiKeyAction {
    CrosstermKey(KeyEvent),
    FontZoomIn,
    FontZoomOut,
    FontZoomReset,
    CopySelection,
    CutSelection,
    SelectAll,
    PasteClipboard,
    Quit,
}

/// Translates a macOS virtual `key_code`, `NSEventModifierFlags`, `characters`, and
/// `charactersIgnoringModifiers` into a [`MacGuiKeyAction`].
pub fn translate_macos_key_event(
    key_code: u16,
    flags: u64,
    chars: &str,
    chars_ignoring_mods: &str,
) -> Option<MacGuiKeyAction> {
    let shift = (flags & NS_EVENT_MODIFIER_SHIFT) != 0;
    let ctrl = (flags & NS_EVENT_MODIFIER_CONTROL) != 0;
    let alt = (flags & NS_EVENT_MODIFIER_OPTION) != 0;
    let cmd = (flags & NS_EVENT_MODIFIER_COMMAND) != 0;

    if cmd && !ctrl && !alt {
        let ch = chars_ignoring_mods
            .chars()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        return match ch {
            'q' => Some(MacGuiKeyAction::Quit),
            'c' => Some(MacGuiKeyAction::CopySelection),
            'x' => Some(MacGuiKeyAction::CutSelection),
            'a' => Some(MacGuiKeyAction::SelectAll),
            'v' => Some(MacGuiKeyAction::PasteClipboard),
            '=' | '+' => Some(MacGuiKeyAction::FontZoomIn),
            '-' | '_' => Some(MacGuiKeyAction::FontZoomOut),
            '0' => Some(MacGuiKeyAction::FontZoomReset),
            _ => None,
        };
    }

    let mut mods = KeyModifiers::empty();
    if shift {
        mods |= KeyModifiers::SHIFT;
    }
    if ctrl {
        mods |= KeyModifiers::CONTROL;
    }
    if alt {
        mods |= KeyModifiers::ALT;
    }

    let special_code = match key_code {
        0x24 | 0x4C => Some(KeyCode::Enter),
        0x30 => Some(if shift {
            KeyCode::BackTab
        } else {
            KeyCode::Tab
        }),
        0x33 => Some(KeyCode::Backspace),
        0x75 => Some(KeyCode::Delete),
        0x35 => Some(KeyCode::Esc),
        0x7B => Some(KeyCode::Left),
        0x7C => Some(KeyCode::Right),
        0x7D => Some(KeyCode::Down),
        0x7E => Some(KeyCode::Up),
        0x73 => Some(KeyCode::Home),
        0x77 => Some(KeyCode::End),
        0x74 => Some(KeyCode::PageUp),
        0x79 => Some(KeyCode::PageDown),
        0x7A => Some(KeyCode::F(1)),
        0x78 => Some(KeyCode::F(2)),
        0x63 => Some(KeyCode::F(3)),
        0x76 => Some(KeyCode::F(4)),
        0x60 => Some(KeyCode::F(5)),
        0x61 => Some(KeyCode::F(6)),
        0x62 => Some(KeyCode::F(7)),
        0x64 => Some(KeyCode::F(8)),
        0x65 => Some(KeyCode::F(9)),
        0x6D => Some(KeyCode::F(10)),
        0x67 => Some(KeyCode::F(11)),
        0x6F => Some(KeyCode::F(12)),
        _ => None,
    };

    if let Some(code) = special_code {
        return Some(MacGuiKeyAction::CrosstermKey(KeyEvent::new(code, mods)));
    }

    if ctrl || alt {
        if key_code == 0x31 {
            return Some(MacGuiKeyAction::CrosstermKey(KeyEvent::new(
                KeyCode::Char(' '),
                mods,
            )));
        }
        if let Some(raw_ch) = chars_ignoring_mods
            .chars()
            .next()
            .or_else(|| chars.chars().next())
        {
            let ch = if ('\x01'..='\x1a').contains(&raw_ch) {
                (b'a' + (raw_ch as u8 - 1)) as char
            } else {
                raw_ch
            };
            if !ch.is_control() && !('\u{f700}'..='\u{f8ff}').contains(&ch) {
                return Some(MacGuiKeyAction::CrosstermKey(KeyEvent::new(
                    KeyCode::Char(ch),
                    mods,
                )));
            }
        }
        return None;
    }

    if let Some(ch) = chars.chars().next()
        && !ch.is_control()
        && !('\u{f700}'..='\u{f8ff}').contains(&ch)
    {
        return Some(MacGuiKeyAction::CrosstermKey(KeyEvent::new(
            KeyCode::Char(ch),
            mods,
        )));
    }

    None
}

fn read_macos_pasteboard() -> Option<String> {
    crate::clipboard::get_system_clipboard()
}

/// Runs Bilterm inside a standalone native macOS Cocoa / CoreText GUI window (`--gui` / `Bilterm.app`).
pub fn run_macos_gui_window(mut app: App) -> Result<()> {
    let ns_app = unsafe { msg_send_id_0(class(c"NSApplication"), sel(c"sharedApplication")) };
    if ns_app.is_null() {
        return Ok(());
    }
    unsafe {
        let _ = msg_send_bool_isize(
            ns_app,
            sel(c"setActivationPolicy:"),
            NS_APPLICATION_ACTIVATION_POLICY_REGULAR,
        );
    }

    let style_mask = NS_WINDOW_STYLE_MASK_TITLED
        | NS_WINDOW_STYLE_MASK_CLOSABLE
        | NS_WINDOW_STYLE_MASK_MINIATURIZABLE
        | NS_WINDOW_STYLE_MASK_RESIZABLE;
    let init_rect = CGRect {
        origin: CGPoint { x: 100.0, y: 100.0 },
        size: CGSize {
            width: 1280.0,
            height: 800.0,
        },
    };

    let alloc_win = unsafe { msg_send_id_0(class(c"NSWindow"), sel(c"alloc")) };
    let window = unsafe {
        msg_send_init_window(
            alloc_win,
            sel(c"initWithContentRect:styleMask:backing:defer:"),
            init_rect,
            style_mask,
            NS_BACKING_STORE_BUFFERED,
            false,
        )
    };
    if window.is_null() {
        return Ok(());
    }

    let title = format!(
        "Bilterm v{} — IBM 3270 x Borland Turbo Vision [Native CoreText GUI]",
        env!("CARGO_PKG_VERSION")
    );
    let cf_title = create_cfstring(&title);
    unsafe {
        msg_send_void_bool(window, sel(c"setReleasedWhenClosed:"), false);
        if !cf_title.is_null() {
            msg_send_void_id(window, sel(c"setTitle:"), cf_title as Id);
            crate::ui::gui_font::macos_ct::CFRelease(cf_title);
        }
        msg_send_void_0(window, sel(c"center"));
    }

    let content_view = unsafe { msg_send_id_0(window, sel(c"contentView")) };
    unsafe {
        msg_send_void_bool(content_view, sel(c"setWantsLayer:"), true);
        msg_send_void_id(window, sel(c"makeKeyAndOrderFront:"), null_mut());
        msg_send_void_0(window, sel(c"display"));
        crate::ui::gui_font::apply_macos_dock_icon(ns_app);
        msg_send_void_bool(ns_app, sel(c"activateIgnoringOtherApps:"), true);
        msg_send_void_0(ns_app, sel(c"finishLaunching"));
    }

    let mut scale_factor = unsafe { msg_send_f64_0(window, sel(c"backingScaleFactor")) }.clamp(1.0, 4.0);
    let mut ct_canvas = MacosGuiCanvas::new(&app.wm, scale_factor);
    let mut gui_renderer = GuiWorkspaceRenderer::new(ct_canvas.cell_width_px, ct_canvas.cell_height_px);
    let mut surface = GuiFrameSurface::new(ct_canvas.cell_width_px, ct_canvas.cell_height_px);

    let view_bounds = unsafe { msg_send_rect_0(content_view, sel(c"bounds")) };
    let mut view_w_pt = view_bounds.size.width.max(320.0);
    let mut view_h_pt = view_bounds.size.height.max(200.0);
    let mut pixel_w = (view_w_pt * scale_factor).round().max(320.0) as u32;
    let mut pixel_h = (view_h_pt * scale_factor).round().max(200.0) as u32;

    let init_cols = (pixel_w / u32::from(ct_canvas.cell_width_px.max(1))).clamp(20, 500) as u16;
    let init_rows = (pixel_h / u32::from(ct_canvas.cell_height_px.max(1))).clamp(6, 200) as u16;
    let init_vp = Rect::new(0, 0, init_cols, init_rows);
    app.wm.resize_all(init_vp)?;
    app.last_viewport = init_vp;

    let font_pt = app.wm.config().scaled_font_size_pt();
    app.wm.active_pane_mut().set_status_message(Some(format!(
        "GUI Font: {} ({}pt CoreText {:.0}x Retina) — :font <jetbrains|firacode|cascadia|plex|vga|cycle> or Cmd++/Cmd+-",
        ct_canvas.resolved_face_name, font_pt, scale_factor
    )));
    app.needs_redraw = true;

    let sel_alloc = sel(c"alloc");
    let sel_init = sel(c"init");
    let sel_drain = sel(c"drain");
    let sel_distant_past = sel(c"distantPast");
    let sel_next_event = sel(c"nextEventMatchingMask:untilDate:inMode:dequeue:");
    let sel_send_event = sel(c"sendEvent:");
    let sel_type = sel(c"type");
    let sel_mod_flags = sel(c"modifierFlags");
    let sel_key_code = sel(c"keyCode");
    let sel_chars = sel(c"characters");
    let sel_chars_ign = sel(c"charactersIgnoringModifiers");
    let sel_loc_in_win = sel(c"locationInWindow");
    let sel_delta_y = sel(c"scrollingDeltaY");
    let sel_is_visible = sel(c"isVisible");
    let sel_bounds = sel(c"bounds");
    let sel_backing_scale = sel(c"backingScaleFactor");
    let sel_layer = sel(c"layer");
    let sel_set_contents = sel(c"setContents:");
    let sel_set_contents_scale = sel(c"setContentsScale:");
    let sel_begin = sel(c"begin");
    let sel_set_disable_actions = sel(c"setDisableActions:");
    let sel_commit = sel(c"commit");
    let sel_flush = sel(c"flush");
    let sel_close = sel(c"close");

    let cls_pool = class(c"NSAutoreleasePool");
    let cls_date = class(c"NSDate");
    let cls_ca_transaction = class(c"CATransaction");

    loop {
        let pool = unsafe { msg_send_id_0(msg_send_id_0(cls_pool, sel_alloc), sel_init) };

        loop {
            let distant_past = unsafe { msg_send_id_0(cls_date, sel_distant_past) };
            let ev = unsafe {
                msg_send_next_event(
                    ns_app,
                    sel_next_event,
                    NS_EVENT_MASK_ANY,
                    distant_past,
                    NSDefaultRunLoopMode,
                    true,
                )
            };
            if ev.is_null() {
                break;
            }

            let ev_type = unsafe { msg_send_u64_0(ev, sel_type) };
            let flags = unsafe { msg_send_u64_0(ev, sel_mod_flags) };

            match ev_type {
                NS_EVENT_TYPE_KEY_DOWN => {
                    let key_code = unsafe { msg_send_u16_0(ev, sel_key_code) };
                    let ns_chars = unsafe { msg_send_id_0(ev, sel_chars) };
                    let ns_chars_ign = unsafe { msg_send_id_0(ev, sel_chars_ign) };
                    let chars_s = cfstring_to_string(ns_chars.cast());
                    let chars_ign_s = cfstring_to_string(ns_chars_ign.cast());
                    if let Some(action) =
                        translate_macos_key_event(key_code, flags, &chars_s, &chars_ign_s)
                    {
                        match action {
                            MacGuiKeyAction::CrosstermKey(kev) => {
                                app.handle_crossterm_event(Event::Key(kev))?;
                                app.needs_redraw = true;
                            }
                            MacGuiKeyAction::FontZoomIn => {
                                let _ = app.wm.apply(Action::FontZoomIn);
                                app.needs_redraw = true;
                            }
                            MacGuiKeyAction::FontZoomOut => {
                                let _ = app.wm.apply(Action::FontZoomOut);
                                app.needs_redraw = true;
                            }
                            MacGuiKeyAction::FontZoomReset => {
                                let _ = app.wm.apply(Action::FontZoomReset);
                                app.needs_redraw = true;
                            }
                            MacGuiKeyAction::CopySelection => {
                                let pane = app.wm.active_pane_mut();
                                if pane.prompt().has_selection() {
                                    if let Some(copied) = pane.prompt_mut().copy_selection() {
                                        pane.prompt_mut().clear_selection();
                                        pane.set_status_message(Some(format!(
                                            "Copied {} chars to clipboard",
                                            copied.chars().count()
                                        )));
                                    }
                                } else if pane.copy_mode().has_selection() {
                                    let _ = pane.yank_copy_selection();
                                }
                                app.needs_redraw = true;
                            }
                            MacGuiKeyAction::CutSelection => {
                                let pane = app.wm.active_pane_mut();
                                if pane.prompt().has_selection() {
                                    if let Some(cut) = pane.prompt_mut().cut_selection_or_line() {
                                        pane.set_status_message(Some(format!(
                                            "Cut {} chars to clipboard",
                                            cut.chars().count()
                                        )));
                                    }
                                } else if pane.copy_mode().has_selection() {
                                    let _ = pane.cut_copy_selection_to_prompt();
                                }
                                app.needs_redraw = true;
                            }
                            MacGuiKeyAction::SelectAll => {
                                let pane = app.wm.active_pane_mut();
                                pane.prompt_mut().select_all();
                                pane.set_status_message(Some("Selected all prompt text".to_string()));
                                app.needs_redraw = true;
                            }
                            MacGuiKeyAction::PasteClipboard => {
                                if let Some(text) = read_macos_pasteboard() {
                                    app.handle_crossterm_event(Event::Paste(text))?;
                                    app.needs_redraw = true;
                                }
                            }
                            MacGuiKeyAction::Quit => {
                                app.should_quit = true;
                            }
                        }
                    }
                }
                NS_EVENT_TYPE_KEY_UP => {}
                NS_EVENT_TYPE_LEFT_MOUSE_DOWN
                | NS_EVENT_TYPE_LEFT_MOUSE_UP
                | NS_EVENT_TYPE_LEFT_MOUSE_DRAGGED
                | NS_EVENT_TYPE_RIGHT_MOUSE_DOWN
                | NS_EVENT_TYPE_RIGHT_MOUSE_UP
                | NS_EVENT_TYPE_OTHER_MOUSE_DOWN
                | NS_EVENT_TYPE_OTHER_MOUSE_UP => {
                    unsafe { msg_send_void_id(ns_app, sel_send_event, ev) };
                    let loc = unsafe { msg_send_point_0(ev, sel_loc_in_win) };
                    if loc.x >= 0.0 && loc.x < view_w_pt && loc.y >= 0.0 && loc.y < view_h_pt {
                        let px = (loc.x * scale_factor) as i32;
                        let py = ((view_h_pt - loc.y) * scale_factor) as i32;
                        let (col, row) = pixel_to_cell_coords(
                            px,
                            py,
                            ct_canvas.cell_width_px,
                            ct_canvas.cell_height_px,
                            app.last_viewport,
                        );
                        let mut modifiers = KeyModifiers::empty();
                        if (flags & NS_EVENT_MODIFIER_SHIFT) != 0 {
                            modifiers |= KeyModifiers::SHIFT;
                        }
                        if (flags & NS_EVENT_MODIFIER_CONTROL) != 0 {
                            modifiers |= KeyModifiers::CONTROL;
                        }
                        if (flags & NS_EVENT_MODIFIER_OPTION) != 0 {
                            modifiers |= KeyModifiers::ALT;
                        }
                        let kind = match ev_type {
                            NS_EVENT_TYPE_LEFT_MOUSE_DOWN => MouseEventKind::Down(MouseButton::Left),
                            NS_EVENT_TYPE_LEFT_MOUSE_UP => MouseEventKind::Up(MouseButton::Left),
                            NS_EVENT_TYPE_LEFT_MOUSE_DRAGGED => {
                                MouseEventKind::Drag(MouseButton::Left)
                            }
                            NS_EVENT_TYPE_RIGHT_MOUSE_DOWN => {
                                MouseEventKind::Down(MouseButton::Right)
                            }
                            NS_EVENT_TYPE_RIGHT_MOUSE_UP => MouseEventKind::Up(MouseButton::Right),
                            NS_EVENT_TYPE_OTHER_MOUSE_DOWN => {
                                MouseEventKind::Down(MouseButton::Middle)
                            }
                            _ => MouseEventKind::Up(MouseButton::Middle),
                        };
                        app.handle_crossterm_event(Event::Mouse(MouseEvent {
                            kind,
                            column: col,
                            row,
                            modifiers,
                        }))?;
                        app.needs_redraw = true;
                    }
                }
                NS_EVENT_TYPE_SCROLL_WHEEL => {
                    unsafe { msg_send_void_id(ns_app, sel_send_event, ev) };
                    let dy = unsafe { msg_send_f64_0(ev, sel_delta_y) };
                    if dy.abs() > 0.01 {
                        let ctrl_or_cmd = (flags
                            & (NS_EVENT_MODIFIER_CONTROL | NS_EVENT_MODIFIER_COMMAND))
                            != 0;
                        if ctrl_or_cmd {
                            let _ = app.wm.apply(if dy > 0.0 {
                                Action::FontZoomIn
                            } else {
                                Action::FontZoomOut
                            });
                            app.needs_redraw = true;
                        } else {
                            let loc = unsafe { msg_send_point_0(ev, sel_loc_in_win) };
                            let px = (loc.x.max(0.0) * scale_factor) as i32;
                            let py = ((view_h_pt - loc.y).max(0.0) * scale_factor) as i32;
                            let (col, row) = pixel_to_cell_coords(
                                px,
                                py,
                                ct_canvas.cell_width_px,
                                ct_canvas.cell_height_px,
                                app.last_viewport,
                            );
                            let kind = if dy > 0.0 {
                                MouseEventKind::ScrollUp
                            } else {
                                MouseEventKind::ScrollDown
                            };
                            app.handle_crossterm_event(Event::Mouse(MouseEvent {
                                kind,
                                column: col,
                                row,
                                modifiers: KeyModifiers::empty(),
                            }))?;
                            app.needs_redraw = true;
                        }
                    }
                }
                _ => {
                    unsafe { msg_send_void_id(ns_app, sel_send_event, ev) };
                }
            }
        }

        let visible = unsafe { (msg_send_bool_0(window, sel_is_visible) & 1) != 0 };
        if !visible {
            app.should_quit = true;
        }

        let cur_bounds = unsafe { msg_send_rect_0(content_view, sel_bounds) };
        let cur_scale = unsafe { msg_send_f64_0(window, sel_backing_scale) }.clamp(1.0, 4.0);
        let new_w_pt = cur_bounds.size.width.max(160.0);
        let new_h_pt = cur_bounds.size.height.max(100.0);
        let new_px_w = (new_w_pt * cur_scale).round().max(160.0) as u32;
        let new_px_h = (new_h_pt * cur_scale).round().max(100.0) as u32;

        if new_px_w != pixel_w || new_px_h != pixel_h || (cur_scale - scale_factor).abs() > 0.01 {
            view_w_pt = new_w_pt;
            view_h_pt = new_h_pt;
            pixel_w = new_px_w;
            pixel_h = new_px_h;
            scale_factor = cur_scale;
            let (cw, ch, _) = ct_canvas.sync_font(&app.wm, scale_factor);
            gui_renderer.cell_width_px = cw;
            gui_renderer.cell_height_px = ch;
            let c = (pixel_w / u32::from(cw.max(1))).clamp(20, 500) as u16;
            let r = (pixel_h / u32::from(ch.max(1))).clamp(6, 200) as u16;
            let new_vp = Rect::new(0, 0, c, r);
            app.wm.resize_all(new_vp)?;
            app.last_viewport = new_vp;
            app.needs_redraw = true;
        }

        if app.poll_all_ptys() {
            app.needs_redraw = true;
        }

        if app.should_quit || app.all_panes_exited() {
            unsafe { msg_send_void_0(pool, sel_drain) };
            break;
        }

        if app.needs_redraw {
            let (cw, ch, font_changed) = ct_canvas.sync_font(&app.wm, scale_factor);
            if font_changed {
                gui_renderer.cell_width_px = cw;
                gui_renderer.cell_height_px = ch;
                let c = (pixel_w / u32::from(cw.max(1))).clamp(20, 500) as u16;
                let r = (pixel_h / u32::from(ch.max(1))).clamp(6, 200) as u16;
                let new_vp = Rect::new(0, 0, c, r);
                app.wm.resize_all(new_vp)?;
                app.last_viewport = new_vp;
            }
            gui_renderer.render_workspace_with_cell_size(
                &app.wm,
                app.last_viewport,
                cw,
                ch,
                &mut surface,
            );
            let pal = app.wm.config().palette();
            let default_bg = match pal.window_frame_bg {
                ratatui::style::Color::Rgb(r, g, b) => [r, g, b, 255],
                _ => [0, 0, 0x84, 255],
            };
            let cg_img = ct_canvas.render_frame_cgimage(pixel_w, pixel_h, &surface, default_bg);
            if !cg_img.is_null() {
                let layer = unsafe { msg_send_id_0(content_view, sel_layer) };
                if !layer.is_null() {
                    unsafe {
                        msg_send_void_0(cls_ca_transaction, sel_begin);
                        msg_send_void_bool(cls_ca_transaction, sel_set_disable_actions, true);
                        msg_send_void_f64(layer, sel_set_contents_scale, scale_factor);
                        msg_send_void_id(layer, sel_set_contents, cg_img.cast());
                        msg_send_void_0(cls_ca_transaction, sel_commit);
                        msg_send_void_0(cls_ca_transaction, sel_flush);
                    }
                }
                unsafe { CGImageRelease(cg_img) };
            }
            app.needs_redraw = false;
        }

        unsafe { msg_send_void_0(pool, sel_drain) };
        std::thread::sleep(Duration::from_millis(16));
    }

    unsafe { msg_send_void_0(window, sel_close) };
    let _ = CString::default();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_macos_virtual_keys_modifiers_and_cmd_shortcuts() {
        // Cmd+Q -> Quit, Cmd+V -> Paste, Cmd+= -> FontZoomIn
        assert_eq!(
            translate_macos_key_event(0x0C, NS_EVENT_MODIFIER_COMMAND, "q", "q"),
            Some(MacGuiKeyAction::Quit)
        );
        assert_eq!(
            translate_macos_key_event(0x09, NS_EVENT_MODIFIER_COMMAND, "v", "v"),
            Some(MacGuiKeyAction::PasteClipboard)
        );
        assert_eq!(
            translate_macos_key_event(0x18, NS_EVENT_MODIFIER_COMMAND, "=", "="),
            Some(MacGuiKeyAction::FontZoomIn)
        );

        // Ctrl+X -> Control + 'x'
        assert_eq!(
            translate_macos_key_event(0x07, NS_EVENT_MODIFIER_CONTROL, "\x18", "x"),
            Some(MacGuiKeyAction::CrosstermKey(KeyEvent::new(
                KeyCode::Char('x'),
                KeyModifiers::CONTROL
            )))
        );

        // Option+B (macOS produces '∫' in characters, 'b' in charactersIgnoringModifiers) -> Alt + 'b'
        assert_eq!(
            translate_macos_key_event(0x0B, NS_EVENT_MODIFIER_OPTION, "∫", "b"),
            Some(MacGuiKeyAction::CrosstermKey(KeyEvent::new(
                KeyCode::Char('b'),
                KeyModifiers::ALT
            )))
        );

        // F4 (0x76)
        assert_eq!(
            translate_macos_key_event(0x76, 0, "\u{f707}", "\u{f707}"),
            Some(MacGuiKeyAction::CrosstermKey(KeyEvent::new(
                KeyCode::F(4),
                KeyModifiers::empty()
            )))
        );
    }
}
