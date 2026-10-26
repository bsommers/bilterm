//! Standalone Native OS GUI Window Runner (`bilterm --gui` / `-G`, `GPU-02`).
//!
//! Drives [`App`] + [`GuiWorkspaceRenderer`] + [`GuiFrameSurface`] inside a native desktop window
//! without requiring an external terminal emulator:
//! - **Windows (`#[cfg(windows)]`)**: Native HiDPI Win32 (`user32.dll` + `gdi32.dll`) window backed by
//!   [`crate::ui::gui_font::win32_gdi::Win32GuiCanvas`] (ClearType TrueType/OpenType text rendering +
//!   1:1 `CreateDIBSection` + `BitBlt` + edge-to-edge geometric box-drawing), full keyboard (`WM_KEYDOWN` /
//!   `WM_SYSKEYDOWN` / `WM_CHAR`), mouse drag/click/scroll (`WM_LBUTTONDOWN`, `WM_MOUSEMOVE`,
//!   `WM_MOUSEWHEEL` + `Ctrl+Wheel` font zoom), and live pixel-to-cell window resizing (`WM_SIZE`).
//! - **Linux / macOS (`#[cfg(not(windows))]`)**: Renders initial [`GuiFrameSurface`] verification
//!   and launches a dedicated standalone window via [`crate::launcher::ensure_terminal_or_relaunch`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
#[cfg(windows)]
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::App;
use crate::error::Result;
#[cfg(not(windows))]
use crate::ui::renderer::WorkspaceFrameRenderer;
use crate::ui::renderer::{GuiFrameSurface, GuiWorkspaceRenderer};

/// Converts an RGBA `[R, G, B, A]` pixel buffer into a 32-bit `0x00RRGGBB` (`[B, G, R, 255]` in
/// little-endian) buffer suitable for Win32 DIBs and X11 `XPutImage`.
pub fn rgba_to_bgra_u32(rgba: &[u8], out: &mut Vec<u32>) {
    let pixel_count = rgba.len() / 4;
    out.clear();
    out.reserve(pixel_count);
    for px in rgba.chunks_exact(4) {
        let r = u32::from(px[0]);
        let g = u32::from(px[1]);
        let b = u32::from(px[2]);
        out.push(0xFF00_0000 | (r << 16) | (g << 8) | b);
    }
}

/// Maps pixel coordinates `(px, py)` inside a client area to 0-indexed character cell `(col, row)`.
pub fn pixel_to_cell_coords(
    px: i32,
    py: i32,
    cell_width_px: u16,
    cell_height_px: u16,
    viewport: Rect,
) -> (u16, u16) {
    let cw = i32::from(cell_width_px.max(1));
    let ch = i32::from(cell_height_px.max(1));
    let col = (px.max(0) / cw) as u16;
    let row = (py.max(0) / ch) as u16;
    (
        col.min(viewport.width.saturating_sub(1)),
        row.min(viewport.height.saturating_sub(1)),
    )
}

/// Translates a Win32 Virtual-Key code (`VK_*`) and modifier state `(ctrl, alt, shift)` into a
/// non-printable/navigation [`KeyEvent`] when applicable (`WM_KEYDOWN` / `WM_SYSKEYDOWN`).
/// Printable ASCII/Unicode characters without `Ctrl`/`Alt` are handled by `WM_CHAR`.
pub fn translate_vk_to_key_event(
    vk: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
) -> Option<KeyEvent> {
    let mut mods = KeyModifiers::empty();
    if ctrl {
        mods |= KeyModifiers::CONTROL;
    }
    if alt {
        mods |= KeyModifiers::ALT;
    }
    if shift {
        mods |= KeyModifiers::SHIFT;
    }

    let code = match vk {
        0x08 => KeyCode::Backspace,
        0x09 => {
            if shift {
                KeyCode::BackTab
            } else {
                KeyCode::Tab
            }
        }
        0x0D => KeyCode::Enter,
        0x1B => KeyCode::Esc,
        0x21 => KeyCode::PageUp,
        0x22 => KeyCode::PageDown,
        0x23 => KeyCode::End,
        0x24 => KeyCode::Home,
        0x25 => KeyCode::Left,
        0x26 => KeyCode::Up,
        0x27 => KeyCode::Right,
        0x28 => KeyCode::Down,
        0x2D => KeyCode::Insert,
        0x2E => KeyCode::Delete,
        0x70..=0x7B => KeyCode::F((vk - 0x70 + 1) as u8),
        0x20 if ctrl || alt => KeyCode::Char(' '),
        0x30..=0x39 if ctrl || alt => KeyCode::Char((vk as u8) as char),
        0x41..=0x5A if ctrl || alt => {
            let base = (vk as u8 - b'A' + b'a') as char;
            KeyCode::Char(if shift {
                base.to_ascii_uppercase()
            } else {
                base
            })
        }
        0xBB if ctrl || alt => KeyCode::Char(if shift { '+' } else { '=' }),
        0xBD if ctrl || alt => KeyCode::Char(if shift { '_' } else { '-' }),
        0xC0 if ctrl || alt => KeyCode::Char(if shift { '~' } else { '`' }),
        0xDB if ctrl || alt => KeyCode::Char(if shift { '{' } else { '[' }),
        0xDD if ctrl || alt => KeyCode::Char(if shift { '}' } else { ']' }),
        _ => return None,
    };

    Some(KeyEvent::new(code, mods))
}

/// Runs Bilterm inside a standalone native GUI window (`--gui` / `-G`).
#[cfg(windows)]
pub fn run_native_gui_window(mut app: App) -> Result<()> {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::ptr::{null, null_mut};
    use std::time::Duration;

    use crate::action::Action;
    use crate::ui::gui_font::win32_gdi::{
        Hwnd, Win32GuiCanvas, enable_hidpi_awareness, query_window_dpi,
    };

    type Hinstance = *mut core::ffi::c_void;
    type Hdc = *mut core::ffi::c_void;
    type Hicon = *mut core::ffi::c_void;
    type Hcursor = *mut core::ffi::c_void;
    type Hbrush = *mut core::ffi::c_void;
    type Hmenu = *mut core::ffi::c_void;
    type Lresult = isize;
    type Wparam = usize;
    type Lparam = isize;

    #[repr(C)]
    struct WndClassExW {
        cb_size: u32,
        style: u32,
        lpfn_wnd_proc: unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: Hinstance,
        h_icon: Hicon,
        h_cursor: Hcursor,
        hbr_background: Hbrush,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
        h_icon_sm: Hicon,
    }

    #[repr(C)]
    #[derive(Default)]
    struct WinPoint {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct WinMsg {
        hwnd: Hwnd,
        message: u32,
        w_param: Wparam,
        l_param: Lparam,
        time: u32,
        pt: WinPoint,
    }

    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct PaintStruct {
        hdc: Hdc,
        f_erase: i32,
        rc_paint: WinRect,
        f_restore: i32,
        f_inc_update: i32,
        rgb_reserved: [u8; 32],
    }

    const CS_HREDRAW: u32 = 0x0002;
    const CS_VREDRAW: u32 = 0x0001;
    const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    const CW_USEDEFAULT: i32 = -2_147_483_648;
    const SW_SHOW: i32 = 5;
    const PM_REMOVE: u32 = 0x0001;
    const WM_DESTROY: u32 = 0x0002;
    const WM_SIZE: u32 = 0x0005;
    const WM_PAINT: u32 = 0x000F;
    const WM_CLOSE: u32 = 0x0010;
    const WM_QUIT: u32 = 0x0012;
    const WM_ERASEBKGND: u32 = 0x0014;
    const WM_KEYDOWN: u32 = 0x0100;
    const WM_CHAR: u32 = 0x0102;
    const WM_SYSKEYDOWN: u32 = 0x0104;
    const WM_MOUSEMOVE: u32 = 0x0200;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_LBUTTONUP: u32 = 0x0202;
    const WM_RBUTTONDOWN: u32 = 0x0204;
    const WM_RBUTTONUP: u32 = 0x0205;
    const WM_MBUTTONDOWN: u32 = 0x0207;
    const WM_MBUTTONUP: u32 = 0x0208;
    const WM_MOUSEWHEEL: u32 = 0x020A;
    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12;
    const IDC_ARROW: *const u16 = 32512 as *const u16;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(lp_module_name: *const u16) -> Hinstance;
        fn FreeConsole() -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassExW(lpwcx: *const WndClassExW) -> u16;
        fn CreateWindowExW(
            dw_ex_style: u32,
            lp_class_name: *const u16,
            lp_window_name: *const u16,
            dw_style: u32,
            x: i32,
            y: i32,
            n_width: i32,
            n_height: i32,
            h_wnd_parent: Hwnd,
            h_menu: Hmenu,
            h_instance: Hinstance,
            lp_param: *mut core::ffi::c_void,
        ) -> Hwnd;
        fn ShowWindow(h_wnd: Hwnd, n_cmd_show: i32) -> i32;
        fn UpdateWindow(h_wnd: Hwnd) -> i32;
        fn PeekMessageW(
            lp_msg: *mut WinMsg,
            h_wnd: Hwnd,
            w_msg_filter_min: u32,
            w_msg_filter_max: u32,
            w_remove_msg: u32,
        ) -> i32;
        fn TranslateMessage(lp_msg: *const WinMsg) -> i32;
        fn DispatchMessageW(lp_msg: *const WinMsg) -> Lresult;
        fn DefWindowProcW(h_wnd: Hwnd, msg: u32, w_param: Wparam, l_param: Lparam) -> Lresult;
        fn PostQuitMessage(n_exit_code: i32);
        fn GetClientRect(h_wnd: Hwnd, lp_rect: *mut WinRect) -> i32;
        fn BeginPaint(h_wnd: Hwnd, lp_paint: *mut PaintStruct) -> Hdc;
        fn EndPaint(h_wnd: Hwnd, lp_paint: *const PaintStruct) -> i32;
        fn LoadCursorW(h_instance: Hinstance, lp_cursor_name: *const u16) -> Hcursor;
        fn GetKeyState(n_virt_key: i32) -> i16;
    }

    enum GuiWinEvent {
        Crossterm(Event),
        FontZoomDelta(i16),
        ResizePixels(u32, u32),
        Paint,
        Quit,
    }

    thread_local! {
        static EVENT_QUEUE: RefCell<VecDeque<GuiWinEvent>> = const { RefCell::new(VecDeque::new()) };
        static CELL_GEOM: RefCell<(u16, u16, Rect)> = const { RefCell::new((10, 20, Rect::new(0, 0, 100, 30))) };
        static LEFT_DRAGGING: RefCell<bool> = const { RefCell::new(false) };
    }

    fn current_modifiers() -> (bool, bool, bool, KeyModifiers) {
        let ctrl = unsafe { (GetKeyState(VK_CONTROL) as u16 & 0x8000) != 0 };
        let alt = unsafe { (GetKeyState(VK_MENU) as u16 & 0x8000) != 0 };
        let shift = unsafe { (GetKeyState(VK_SHIFT) as u16 & 0x8000) != 0 };
        let mut mods = KeyModifiers::empty();
        if ctrl {
            mods |= KeyModifiers::CONTROL;
        }
        if alt {
            mods |= KeyModifiers::ALT;
        }
        if shift {
            mods |= KeyModifiers::SHIFT;
        }
        (ctrl, alt, shift, mods)
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: Hwnd,
        msg: u32,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult {
        match msg {
            WM_CLOSE | WM_DESTROY => {
                EVENT_QUEUE.with(|q| q.borrow_mut().push_back(GuiWinEvent::Quit));
                unsafe { PostQuitMessage(0) };
                0
            }
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut ps: PaintStruct = unsafe { std::mem::zeroed() };
                let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };
                unsafe { EndPaint(hwnd, &ps) };
                EVENT_QUEUE.with(|q| q.borrow_mut().push_back(GuiWinEvent::Paint));
                0
            }
            WM_SIZE => {
                let w = (l_param as usize & 0xFFFF) as u32;
                let h = ((l_param as usize >> 16) & 0xFFFF) as u32;
                if w > 0 && h > 0 {
                    EVENT_QUEUE.with(|q| q.borrow_mut().push_back(GuiWinEvent::ResizePixels(w, h)));
                }
                0
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let (ctrl, alt, shift, _) = current_modifiers();
                if let Some(kev) = translate_vk_to_key_event(w_param as u32, ctrl, alt, shift) {
                    EVENT_QUEUE.with(|q| {
                        q.borrow_mut()
                            .push_back(GuiWinEvent::Crossterm(Event::Key(kev)))
                    });
                    return 0;
                }
                unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) }
            }
            WM_CHAR => {
                let (ctrl, alt, shift, _) = current_modifiers();
                if !ctrl
                    && !alt
                    && let Some(ch) = char::from_u32(w_param as u32)
                    && !ch.is_control()
                {
                    let mut mods = KeyModifiers::empty();
                    if shift {
                        mods |= KeyModifiers::SHIFT;
                    }
                    let kev = KeyEvent::new(KeyCode::Char(ch), mods);
                    EVENT_QUEUE.with(|q| {
                        q.borrow_mut()
                            .push_back(GuiWinEvent::Crossterm(Event::Key(kev)))
                    });
                }
                0
            }
            WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN
            | WM_MBUTTONUP | WM_MOUSEMOVE => {
                let px = (l_param & 0xFFFF) as i16 as i32;
                let py = ((l_param >> 16) & 0xFFFF) as i16 as i32;
                let (cw, ch, vp) = CELL_GEOM.with(|g| *g.borrow());
                let (col, row) = pixel_to_cell_coords(px, py, cw, ch, vp);
                let (_, _, _, modifiers) = current_modifiers();
                let kind = match msg {
                    WM_LBUTTONDOWN => {
                        LEFT_DRAGGING.with(|d| *d.borrow_mut() = true);
                        Some(MouseEventKind::Down(MouseButton::Left))
                    }
                    WM_LBUTTONUP => {
                        LEFT_DRAGGING.with(|d| *d.borrow_mut() = false);
                        Some(MouseEventKind::Up(MouseButton::Left))
                    }
                    WM_RBUTTONDOWN => Some(MouseEventKind::Down(MouseButton::Right)),
                    WM_RBUTTONUP => Some(MouseEventKind::Up(MouseButton::Right)),
                    WM_MBUTTONDOWN => Some(MouseEventKind::Down(MouseButton::Middle)),
                    WM_MBUTTONUP => Some(MouseEventKind::Up(MouseButton::Middle)),
                    WM_MOUSEMOVE => {
                        let dragging = LEFT_DRAGGING.with(|d| *d.borrow());
                        if dragging {
                            Some(MouseEventKind::Drag(MouseButton::Left))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    let mev = MouseEvent {
                        kind,
                        column: col,
                        row,
                        modifiers,
                    };
                    EVENT_QUEUE.with(|q| {
                        q.borrow_mut()
                            .push_back(GuiWinEvent::Crossterm(Event::Mouse(mev)))
                    });
                }
                0
            }
            WM_MOUSEWHEEL => {
                let delta = ((w_param >> 16) & 0xFFFF) as i16;
                let (ctrl, _, _, modifiers) = current_modifiers();
                if ctrl {
                    let step = if delta > 0 { 10 } else { -10 };
                    EVENT_QUEUE.with(|q| q.borrow_mut().push_back(GuiWinEvent::FontZoomDelta(step)));
                    return 0;
                }
                let (cw, ch, vp) = CELL_GEOM.with(|g| *g.borrow());
                let px = (l_param & 0xFFFF) as i16 as i32;
                let py = ((l_param >> 16) & 0xFFFF) as i16 as i32;
                let (col, row) = pixel_to_cell_coords(px, py, cw, ch, vp);
                let kind = if delta > 0 {
                    MouseEventKind::ScrollUp
                } else {
                    MouseEventKind::ScrollDown
                };
                let mev = MouseEvent {
                    kind,
                    column: col,
                    row,
                    modifiers,
                };
                EVENT_QUEUE.with(|q| {
                    q.borrow_mut()
                        .push_back(GuiWinEvent::Crossterm(Event::Mouse(mev)))
                });
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) },
        }
    }

    enable_hidpi_awareness();

    if std::env::var("BILTERM_KEEP_CONSOLE").as_deref() != Ok("1") {
        unsafe {
            let _ = FreeConsole();
        }
    }

    let class_name: Vec<u16> = "BiltermGuiWindowClass\0".encode_utf16().collect();
    let title_str = format!(
        "Bilterm v{} — IBM 3270 x Borland Turbo Vision [Native ClearType GUI]\0",
        env!("CARGO_PKG_VERSION")
    );
    let window_title: Vec<u16> = title_str.encode_utf16().collect();

    let h_instance = unsafe { GetModuleHandleW(null()) };
    let wc = WndClassExW {
        cb_size: std::mem::size_of::<WndClassExW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfn_wnd_proc: wnd_proc,
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance,
        h_icon: null_mut(),
        h_cursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbr_background: null_mut(),
        lpsz_menu_name: null(),
        lpsz_class_name: class_name.as_ptr(),
        h_icon_sm: null_mut(),
    };
    unsafe {
        RegisterClassExW(&wc);
    }

    let dpi_init = 96u32;
    let init_w = 1280;
    let init_h = 800;
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            init_w,
            init_h,
            null_mut(),
            null_mut(),
            h_instance,
            null_mut(),
        )
    };
    if hwnd.is_null() {
        return Err(crate::error::BiltermError::Io(std::io::Error::last_os_error()));
    }

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    }

    let mut gdi_canvas = Win32GuiCanvas::new(hwnd, &app.wm);
    let mut gui_renderer = GuiWorkspaceRenderer::new(
        gdi_canvas.cell_width_px,
        gdi_canvas.cell_height_px,
    );
    let mut surface = GuiFrameSurface::new(gdi_canvas.cell_width_px, gdi_canvas.cell_height_px);
    let mut msg: WinMsg = unsafe { std::mem::zeroed() };

    let mut rect = WinRect::default();
    unsafe { GetClientRect(hwnd, &mut rect) };
    let mut client_w = (rect.right - rect.left).max(640) as u32;
    let mut client_h = (rect.bottom - rect.top).max(400) as u32;
    let cols = (client_w / u32::from(gdi_canvas.cell_width_px.max(1))).clamp(20, 500) as u16;
    let rows = (client_h / u32::from(gdi_canvas.cell_height_px.max(1))).clamp(6, 200) as u16;
    let vp = Rect::new(0, 0, cols, rows);
    app.wm.resize_all(vp)?;
    app.last_viewport = vp;
    let font_pt = app.wm.config().scaled_font_size_pt();
    app.wm.active_pane_mut().set_status_message(Some(format!(
        "GUI Font: {} ({}pt ClearType) — :font <cascadia|consolas|jetbrains|firacode|vga|cycle|add> or Ctrl+Wheel",
        gdi_canvas.resolved_face_name,
        font_pt
    )));
    app.needs_redraw = true;
    let _ = dpi_init;

    loop {
        while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if msg.message == WM_QUIT {
                app.should_quit = true;
                break;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        while let Some(ev) = EVENT_QUEUE.with(|q| q.borrow_mut().pop_front()) {
            match ev {
                GuiWinEvent::Crossterm(cev) => {
                    app.handle_crossterm_event(cev)?;
                    app.needs_redraw = true;
                }
                GuiWinEvent::FontZoomDelta(delta) => {
                    let act = if delta > 0 {
                        Action::FontZoomIn
                    } else {
                        Action::FontZoomOut
                    };
                    let _ = app.wm.apply(act);
                    app.needs_redraw = true;
                }
                GuiWinEvent::ResizePixels(w, h) => {
                    client_w = w.max(64);
                    client_h = h.max(64);
                    let c = (client_w / u32::from(gdi_canvas.cell_width_px.max(1))).clamp(20, 500) as u16;
                    let r = (client_h / u32::from(gdi_canvas.cell_height_px.max(1))).clamp(6, 200) as u16;
                    let new_vp = Rect::new(0, 0, c, r);
                    app.wm.resize_all(new_vp)?;
                    app.last_viewport = new_vp;
                    app.needs_redraw = true;
                }
                GuiWinEvent::Paint => {
                    app.needs_redraw = true;
                }
                GuiWinEvent::Quit => {
                    app.should_quit = true;
                }
            }
        }

        if app.poll_all_ptys() {
            app.needs_redraw = true;
        }

        if app.should_quit || app.all_panes_exited() {
            break;
        }

        if app.needs_redraw {
            let dpi = query_window_dpi(hwnd);
            let (cw, ch, font_changed) = gdi_canvas.sync_font(&app.wm, dpi);
            if font_changed {
                gui_renderer.cell_width_px = cw;
                gui_renderer.cell_height_px = ch;
                let c = (client_w / u32::from(cw.max(1))).clamp(20, 500) as u16;
                let r = (client_h / u32::from(ch.max(1))).clamp(6, 200) as u16;
                let new_vp = Rect::new(0, 0, c, r);
                app.wm.resize_all(new_vp)?;
                app.last_viewport = new_vp;
            }
            CELL_GEOM.with(|g| *g.borrow_mut() = (cw, ch, app.last_viewport));
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
            gdi_canvas.present_frame(hwnd, &surface, default_bg);
            app.needs_redraw = false;
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

/// Runs Bilterm in standalone GUI mode on Unix platforms (`Linux` / `macOS`).
#[cfg(not(windows))]
pub fn run_native_gui_window(app: App) -> Result<()> {
    let mut renderer = GuiWorkspaceRenderer::new(
        app.wm.config().cell_width_px.max(8),
        app.wm.config().cell_height_px.max(16),
    );
    let mut surface = GuiFrameSurface::new(renderer.cell_width_px, renderer.cell_height_px);
    renderer.render_workspace(&app.wm, app.last_viewport, &mut surface);
    let _ = surface.rasterize_rgba();
    let _ = crate::launcher::ensure_terminal_or_relaunch(&[])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgba_to_bgra_u32_and_maps_pixel_coords() {
        let rgba = vec![0x12, 0x34, 0x56, 0xFF, 0xAA, 0xBB, 0xCC, 0xFF];
        let mut out = Vec::new();
        rgba_to_bgra_u32(&rgba, &mut out);
        assert_eq!(out, vec![0xFF12_3456, 0xFFAA_BBCC]);

        let vp = Rect::new(0, 0, 80, 24);
        assert_eq!(pixel_to_cell_coords(0, 0, 8, 16, vp), (0, 0));
        assert_eq!(pixel_to_cell_coords(80, 32, 8, 16, vp), (10, 2));
        assert_eq!(pixel_to_cell_coords(9999, 9999, 8, 16, vp), (79, 23));
    }

    #[test]
    fn translates_virtual_keys_and_modifiers_to_crossterm_events() {
        let f4 = translate_vk_to_key_event(0x73, false, false, false).expect("F4");
        assert_eq!(f4.code, KeyCode::F(4));

        let ctrl_p = translate_vk_to_key_event(0x50, true, false, false).expect("Ctrl+P");
        assert_eq!(ctrl_p.code, KeyCode::Char('p'));
        assert!(ctrl_p.modifiers.contains(KeyModifiers::CONTROL));

        let alt_t = translate_vk_to_key_event(0x54, false, true, false).expect("Alt+T");
        assert_eq!(alt_t.code, KeyCode::Char('t'));
        assert!(alt_t.modifiers.contains(KeyModifiers::ALT));

        // Plain printable letter without Ctrl/Alt defers to WM_CHAR
        assert!(translate_vk_to_key_event(0x41, false, false, false).is_none());
    }
}
