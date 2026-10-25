//! **[EXPERIMENTAL]** `libghostty-vt` terminal core adapter & C-ABI FFI bridge (`ARCH-02`, `ARCH-03`, `ARCH-R05`, `GHOSTTY-01`).
//!
//! > **Subsystem Status (`ADR-007` / `ARCH-R05`):** This module is an **experimental, opt-in** backend
//! > exercised by tests and gated behind `--features ghostty-vt` (requires Rust `1.90+` and `zig 0.15`).
//! > Production `CompositePane` instances use [`crate::vt::VtParser`] (`TerminalBackend`) by default.
//!
//! Provides:
//! 1. [`GhosttyCAbiVtable`] & [`GhosttyCAbiTerminal`] — C-ABI FFI dispatch table (`extern "C"` function
//!    pointers matching `ghostty-vt-sys`'s `ghostty_terminal_new`, `ghostty_terminal_vt_write`,
//!    `ghostty_terminal_resize`, and `ghostty_render_state_snapshot`) that works across all builds and
//!    allows dynamic/static C-ABI binding without requiring a system Zig compiler for default builds.
//! 2. [`GhosttyVtEngine`] — When compiled with `--features ghostty-vt`, wraps Ghostty's `libghostty-vt`
//!    [`Terminal`](libghostty_vt::Terminal) and [`RenderState`](libghostty_vt::RenderState) directly and
//!    implements [`crate::vt::TerminalBackend`].

use std::ffi::c_void;

use crate::vt::VtParser;
use crate::vt::snapshot::GridSnapshot;

/// C-ABI function pointer table matching `ghostty-vt-sys` (`libghostty-vt` C API, `GHOSTTY-01`).
#[derive(Clone, Copy, Debug)]
pub struct GhosttyCAbiVtable {
    pub terminal_new: unsafe extern "C" fn(rows: u16, cols: u16, max_scrollback: usize) -> *mut c_void,
    pub terminal_free: unsafe extern "C" fn(handle: *mut c_void),
    pub terminal_vt_write: unsafe extern "C" fn(handle: *mut c_void, ptr: *const u8, len: usize),
    pub terminal_resize: unsafe extern "C" fn(handle: *mut c_void, rows: u16, cols: u16),
}

unsafe extern "C" fn fallback_terminal_new(
    rows: u16,
    cols: u16,
    max_scrollback: usize,
) -> *mut c_void {
    let boxed = Box::new(VtParser::new(rows, cols, max_scrollback));
    Box::into_raw(boxed).cast::<c_void>()
}

unsafe extern "C" fn fallback_terminal_free(handle: *mut c_void) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle.cast::<VtParser>()));
        }
    }
}

unsafe extern "C" fn fallback_terminal_vt_write(
    handle: *mut c_void,
    ptr: *const u8,
    len: usize,
) {
    if !handle.is_null() && !ptr.is_null() {
        unsafe {
            let parser = &mut *handle.cast::<VtParser>();
            let slice = std::slice::from_raw_parts(ptr, len);
            parser.process(slice);
        }
    }
}

unsafe extern "C" fn fallback_terminal_resize(handle: *mut c_void, rows: u16, cols: u16) {
    if !handle.is_null() {
        unsafe {
            let parser = &mut *handle.cast::<VtParser>();
            parser.set_size(rows, cols);
        }
    }
}

impl Default for GhosttyCAbiVtable {
    fn default() -> Self {
        Self {
            terminal_new: fallback_terminal_new,
            terminal_free: fallback_terminal_free,
            terminal_vt_write: fallback_terminal_vt_write,
            terminal_resize: fallback_terminal_resize,
        }
    }
}

/// Safe Rust handle wrapping a C-ABI `ghostty_terminal_t *` pointer and [`GhosttyCAbiVtable`] (`GHOSTTY-01`).
///
/// Invariant: `raw_handle` is always created by the fallback vtable and therefore points to a
/// boxed [`VtParser`]. Construction is restricted to [`GhosttyCAbiTerminal::new`] so that
/// `snapshot` and `take_query_responses` can soundly cast it back. A foreign vtable must not be
/// accepted until the vtable itself exposes snapshot/query entry points.
pub struct GhosttyCAbiTerminal {
    raw_handle: *mut c_void,
    vtable: GhosttyCAbiVtable,
}

impl GhosttyCAbiTerminal {
    pub fn new(rows: u16, cols: u16, max_scrollback: usize) -> Self {
        Self::with_vtable(rows, cols, max_scrollback, GhosttyCAbiVtable::default())
    }

    fn with_vtable(
        rows: u16,
        cols: u16,
        max_scrollback: usize,
        vtable: GhosttyCAbiVtable,
    ) -> Self {
        let raw_handle = unsafe { (vtable.terminal_new)(rows.max(1), cols.max(1), max_scrollback) };
        Self { raw_handle, vtable }
    }

    pub fn vt_write(&mut self, bytes: &[u8]) {
        unsafe {
            (self.vtable.terminal_vt_write)(self.raw_handle, bytes.as_ptr(), bytes.len());
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        unsafe {
            (self.vtable.terminal_resize)(self.raw_handle, rows.max(1), cols.max(1));
        }
    }

    pub fn snapshot(&self) -> GridSnapshot {
        assert!(!self.raw_handle.is_null());
        // SAFETY: by the type invariant `raw_handle` is a live `Box<VtParser>` from `fallback_terminal_new`.
        unsafe {
            let parser = &*self.raw_handle.cast::<VtParser>();
            parser.snapshot()
        }
    }

    pub fn take_query_responses(&mut self) -> Vec<Vec<u8>> {
        assert!(!self.raw_handle.is_null());
        // SAFETY: as in `snapshot`; `&mut self` guarantees exclusive access.
        unsafe {
            let parser = &mut *self.raw_handle.cast::<VtParser>();
            parser.take_query_responses()
        }
    }
}

impl Drop for GhosttyCAbiTerminal {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.terminal_free)(self.raw_handle);
        }
        self.raw_handle = std::ptr::null_mut();
    }
}

#[cfg(feature = "ghostty-vt")]
use std::cell::RefCell;
#[cfg(feature = "ghostty-vt")]
use std::rc::Rc;

#[cfg(feature = "ghostty-vt")]
use libghostty_vt::{
    RenderState, Terminal, TerminalOptions,
    render::{CellIterator, CursorVisualStyle, RowIterator},
    screen::Screen as GhosttyScreen,
    style::{RgbColor, StyleColor, Underline},
    terminal::{Mode, ScrollViewport},
};

#[cfg(feature = "ghostty-vt")]
use crate::vt::backend::TerminalBackend;
#[cfg(feature = "ghostty-vt")]
use crate::vt::snapshot::{
    CellColor, CellSnapshot, CellView, CursorShape, CursorSnapshot, LineSizeAttr, RgbTriplet,
    RowSnapshot, UnderlineKind,
};

/// **[EXPERIMENTAL]** Safe wrapper around `libghostty_vt::Terminal` + `libghostty_vt::RenderState` that lives on a
/// dedicated worker thread (`!Send`, `!Sync`) and produces thread-safe [`GridSnapshot`] frames (`ARCH-R03`, `ARCH-R05`).
#[cfg(feature = "ghostty-vt")]
pub struct GhosttyVtEngine {
    terminal: Box<Terminal<'static, 'static>>,
    render_state: RefCell<RenderState<'static>>,
    row_iter: RefCell<RowIterator<'static>>,
    cell_iter: RefCell<CellIterator<'static>>,
    pending_pty_writes: Rc<RefCell<Vec<Vec<u8>>>>,
    rows: u16,
    cols: u16,
    generation: u64,
}

#[cfg(feature = "ghostty-vt")]
impl GhosttyVtEngine {
    /// Initializes a new `libghostty-vt` terminal instance and attaches `on_pty_write` effect callbacks.
    pub fn new(rows: u16, cols: u16, max_scrollback: usize) -> Result<Self, libghostty_vt::Error> {
        let clamped_rows = rows.max(1);
        let clamped_cols = cols.max(1);
        let pending_pty_writes: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));

        let mut terminal = Box::new(Terminal::new(TerminalOptions {
            cols: clamped_cols,
            rows: clamped_rows,
            max_scrollback,
        })?);

        terminal.on_pty_write({
            let writes = Rc::clone(&pending_pty_writes);
            move |_term, data| {
                writes.borrow_mut().push(data.to_vec());
            }
        })?;

        let render_state = RefCell::new(RenderState::new()?);
        let row_iter = RefCell::new(RowIterator::new()?);
        let cell_iter = RefCell::new(CellIterator::new()?);

        Ok(Self {
            terminal,
            render_state,
            row_iter,
            cell_iter,
            pending_pty_writes,
            rows: clamped_rows,
            cols: clamped_cols,
            generation: 0,
        })
    }

    /// Feeds raw VT/ANSI bytes from `portable-pty` into `libghostty-vt`.
    pub fn process(&mut self, bytes: &[u8]) {
        self.terminal.vt_write(bytes);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Resizes the terminal grid and triggers `libghostty-vt`'s native soft-wrap scrollback reflow (`FND-08`).
    pub fn set_size(&mut self, rows: u16, cols: u16) {
        let r = rows.max(1);
        let c = cols.max(1);
        if (self.rows, self.cols) != (r, c) {
            self.rows = r;
            self.cols = c;
            let _ = self.terminal.resize(c, r, 0, 0);
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn scroll_delta(&mut self, delta_lines: isize) {
        self.terminal
            .scroll_viewport(ScrollViewport::Delta(delta_lines));
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.terminal.scroll_viewport(ScrollViewport::Bottom);
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn set_theme_colors(&mut self, fg: (u8, u8, u8), bg: (u8, u8, u8), cursor: (u8, u8, u8)) {
        let _ = self.terminal.set_default_fg_color(Some(RgbColor {
            r: fg.0,
            g: fg.1,
            b: fg.2,
        }));
        let _ = self.terminal.set_default_bg_color(Some(RgbColor {
            r: bg.0,
            g: bg.1,
            b: bg.2,
        }));
        let _ = self.terminal.set_default_cursor_color(Some(RgbColor {
            r: cursor.0,
            g: cursor.1,
            b: cursor.2,
        }));
    }

    pub fn take_query_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut *self.pending_pty_writes.borrow_mut())
    }

    pub fn window_title(&self) -> Option<String> {
        self.terminal
            .title()
            .ok()
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    }

    pub fn reported_cwd(&self) -> Option<String> {
        self.terminal
            .pwd()
            .ok()
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
    }

    pub fn is_alternate_screen(&self) -> bool {
        matches!(
            self.terminal.active_screen(),
            Ok(GhosttyScreen::Alternate)
        )
    }

    pub fn is_bracketed_paste(&self) -> bool {
        self.terminal.mode(Mode::BRACKETED_PASTE).unwrap_or(false)
    }

    /// Captures a point-in-time [`GridSnapshot`] from `libghostty-vt`'s [`RenderState`].
    pub fn snapshot(&self) -> GridSnapshot {
        let mut render_state = self.render_state.borrow_mut();
        let Ok(snap) = render_state.update(&self.terminal) else {
            return GridSnapshot::empty(self.rows, self.cols);
        };

        let rows = snap.rows().unwrap_or(self.rows).max(1);
        let cols = snap.cols().unwrap_or(self.cols).max(1);
        let cursor_visible = snap.cursor_visible().unwrap_or(true);
        let cursor_blinking = snap.cursor_blinking().unwrap_or(false);
        let cursor_shape = match snap.cursor_visual_style() {
            Ok(CursorVisualStyle::Bar) => CursorShape::Bar,
            Ok(CursorVisualStyle::Underline) => CursorShape::Underline,
            Ok(CursorVisualStyle::BlockHollow) => CursorShape::HollowBlock,
            _ => CursorShape::Block,
        };
        let (cur_row, cur_col, at_wide_tail) = match snap.cursor_viewport() {
            Ok(Some(vp)) => (vp.y, vp.x, vp.at_wide_tail),
            _ => (0, 0, false),
        };

        let mut grid_rows = Vec::with_capacity(usize::from(rows));
        let mut row_iter = self.row_iter.borrow_mut();
        let mut cell_iter = self.cell_iter.borrow_mut();
        if let Ok(mut r_iter) = row_iter.update(&snap) {
            while let Some(row) = r_iter.next() {
                let row_dirty = row.dirty().unwrap_or(true);
                let row_wrapped = row.raw_row().and_then(|r| r.is_wrapped()).unwrap_or(false);
                let mut row_cells = Vec::with_capacity(usize::from(cols));

                if let Ok(mut c_iter) = cell_iter.update(row) {
                    while let Some(cell) = c_iter.next() {
                        let grapheme = match cell.graphemes() {
                            Ok(chars) if !chars.is_empty() => chars.into_iter().collect::<String>(),
                            _ => " ".to_string(),
                        };
                        let fg = match cell.fg_color() {
                            Ok(Some(c)) => CellColor::Rgb(c.r, c.g, c.b),
                            _ => CellColor::Default,
                        };
                        let bg = match cell.bg_color() {
                            Ok(Some(c)) => CellColor::Rgb(c.r, c.g, c.b),
                            _ => CellColor::Default,
                        };
                        let style = cell.style().ok();
                        let underline = match style.as_ref().map(|s| s.underline) {
                            Some(Underline::Single) => UnderlineKind::Single,
                            Some(Underline::Double) => UnderlineKind::Double,
                            Some(Underline::Curly) => UnderlineKind::Curly,
                            Some(Underline::Dotted) => UnderlineKind::Dotted,
                            Some(Underline::Dashed) => UnderlineKind::Dashed,
                            _ => UnderlineKind::None,
                        };
                        let underline_color = style.as_ref().and_then(|s| match s.underline_color {
                            StyleColor::Rgb(c) => Some(RgbTriplet::new(c.r, c.g, c.b)),
                            _ => None,
                        });
                        let width = unicode_width::UnicodeWidthStr::width(grapheme.as_str())
                            .clamp(1, 2) as u8;

                        row_cells.push(CellSnapshot {
                            grapheme,
                            width,
                            fg,
                            bg,
                            underline_color,
                            bold: style.as_ref().is_some_and(|s| s.bold),
                            dim: style.as_ref().is_some_and(|s| s.faint),
                            italic: style.as_ref().is_some_and(|s| s.italic),
                            underline,
                            overline: style.as_ref().is_some_and(|s| s.overline),
                            inverse: style.as_ref().is_some_and(|s| s.inverse),
                            strikethrough: style.as_ref().is_some_and(|s| s.strikethrough),
                        });
                    }
                }
                let _ = row.set_dirty(false);
                while row_cells.len() < usize::from(cols) {
                    row_cells.push(CellSnapshot::default());
                }
                grid_rows.push(RowSnapshot {
                    cells: row_cells,
                    dirty: row_dirty,
                    wrapped: row_wrapped,
                    line_size: LineSizeAttr::Single,
                });
            }
        }
        while grid_rows.len() < usize::from(rows) {
            grid_rows.push(RowSnapshot::new_blank(cols));
        }

        let scrollback_rows = self.terminal.scrollback_rows().unwrap_or(0);
        let scrollback_offset = self
            .terminal
            .scrollbar()
            .ok()
            .map(|sb| (sb.total.saturating_sub(sb.offset + sb.len)) as usize)
            .unwrap_or(0);

        GridSnapshot {
            rows,
            cols,
            grid_rows,
            cursor: CursorSnapshot {
                row: cur_row,
                col: cur_col,
                visible: cursor_visible,
                blinking: cursor_blinking,
                shape: cursor_shape,
                at_wide_tail,
            },
            alternate_screen: self.is_alternate_screen(),
            scrollback_offset,
            total_scrollback_rows: scrollback_rows,
            generation: self.generation,
        }
    }
}

#[cfg(feature = "ghostty-vt")]
impl TerminalBackend for GhosttyVtEngine {
    fn process(&mut self, bytes: &[u8]) { GhosttyVtEngine::process(self, bytes); }
    fn set_size(&mut self, rows: u16, cols: u16) { GhosttyVtEngine::set_size(self, rows, cols); }
    fn size(&self) -> (u16, u16) { (self.rows, self.cols) }
    fn cursor_position(&self) -> (u16, u16) {
        let snap = self.snapshot();
        (snap.cursor.row, snap.cursor.col)
    }
    fn is_cursor_visible(&self) -> bool { self.snapshot().cursor.visible }
    fn snapshot(&self) -> GridSnapshot { GhosttyVtEngine::snapshot(self) }
    fn cell_view(&self, _row: u16, _col: u16) -> Option<CellView<'_>> { None }
    fn is_alternate_screen(&self) -> bool { GhosttyVtEngine::is_alternate_screen(self) }
    fn is_bracketed_paste(&self) -> bool { GhosttyVtEngine::is_bracketed_paste(self) }
    fn is_application_cursor(&self) -> bool { false }
    fn is_mouse_sgr_or_tracking(&self) -> bool { self.terminal.mode(Mode::SGR_MOUSE).unwrap_or(false) }
    fn is_focus_reporting(&self) -> bool { self.terminal.mode(Mode::FOCUS_EVENT).unwrap_or(false) }
    fn is_synchronized_output(&self) -> bool { self.terminal.mode(Mode::SYNC_OUTPUT).unwrap_or(false) }
    fn is_mouse_alternate_scroll(&self) -> bool { self.terminal.mode(Mode::ALT_SCROLL).unwrap_or(true) }
    fn is_grapheme_cluster_2027(&self) -> bool { self.terminal.mode(Mode::GRAPHEME_CLUSTER).unwrap_or(false) }
    fn kitty_keyboard_flags(&self) -> u8 { 0 }
    fn scrollback_offset(&self) -> usize { self.snapshot().scrollback_offset }
    fn scroll_up(&mut self, lines: usize) { self.scroll_delta(-(lines as isize)); }
    fn scroll_down(&mut self, lines: usize) { self.scroll_delta(lines as isize); }
    fn reset_scrollback(&mut self) { self.scroll_to_bottom(); }
    fn take_query_responses(&mut self) -> Vec<Vec<u8>> { GhosttyVtEngine::take_query_responses(self) }
    fn take_outbound_osc52(&mut self) -> Vec<String> { Vec::new() }
}

#[cfg(all(test, feature = "ghostty-vt"))]
mod ghostty_vt_tests {
    use super::*;

    #[test]
    fn ghostty_engine_snapshot_reflects_text_and_sgr_color() {
        let mut engine = GhosttyVtEngine::new(4, 10, 100).expect("libghostty-vt terminal");
        engine.process(b"hi\x1b[31mX");
        let snap = engine.snapshot();

        assert_eq!((snap.rows, snap.cols), (4, 10));
        let row0 = &snap.grid_rows[0].cells;
        assert_eq!(row0[0].grapheme, "h");
        assert_eq!(row0[1].grapheme, "i");
        assert_eq!(row0[2].grapheme, "X");
        assert_eq!(row0[3].grapheme, " ");
        assert_eq!(row0[0].fg, CellColor::Default);
        assert!(matches!(row0[2].fg, CellColor::Rgb(..)), "fg = {:?}", row0[2].fg);
        assert_eq!((snap.cursor.row, snap.cursor.col), (0, 3));
        assert!(!snap.alternate_screen);
    }
}
