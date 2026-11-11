//! First-Class `FONTS` utility window (`WIN-01`, Phase 51.2).
//!
//! Non-modal utility window for live font family selection, size/scale adjustment, custom font directory scanning,
//! and instant live previewing across composite terminal panes. Follows the non-modal pattern of `AgentsList`.

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

use crate::pane::FontFamilyPreset;
use crate::wm::{WindowManager, WindowResizeEdge};

/// Operations performed on the `FONTS` window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontsWindowOp {
    Open,
    Close,
    Toggle,
    Dismiss,
    Blur,
    Pin(Option<bool>),
    Dock(Option<bool>),
    Step(i32),
    Page(i32),
    First,
    Last,
    Scroll(i32),
    SelectRow(usize),
    ApplySelected,
    ApplyAll,
    SizeStep(i16),
    ScaleStep(i16),
    Scan,
    Move { dx: i16, dy: i16 },
    Resize { dw: i16, dh: i16 },
    SetRect { x: u16, y: u16, w: u16, h: u16 },
}

impl FontsWindowOp {
    /// Canonical colon command twin for every operation.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Open => ":fonts open".into(),
            Self::Close => ":fonts close".into(),
            Self::Toggle => ":fonts toggle".into(),
            Self::Dismiss => ":fonts dismiss".into(),
            Self::Blur => ":fonts blur".into(),
            Self::Pin(Some(true)) => ":fonts pin on".into(),
            Self::Pin(Some(false)) => ":fonts pin off".into(),
            Self::Pin(None) => ":fonts pin".into(),
            Self::Dock(Some(true)) => ":fonts dock on".into(),
            Self::Dock(Some(false)) => ":fonts dock off".into(),
            Self::Dock(None) => ":fonts dock".into(),
            Self::Step(n) => format!(":fonts step {n}"),
            Self::Page(n) => format!(":fonts page {n}"),
            Self::First => ":fonts first".into(),
            Self::Last => ":fonts last".into(),
            Self::Scroll(n) => format!(":fonts scroll {n}"),
            Self::SelectRow(idx) => format!(":fonts select {idx}"),
            Self::ApplySelected => ":fonts apply".into(),
            Self::ApplyAll => ":fonts all".into(),
            Self::SizeStep(d) => format!(":fonts size-step {d}"),
            Self::ScaleStep(d) => format!(":fonts scale-step {d}"),
            Self::Scan => ":fonts scan".into(),
            Self::Move { dx, dy } => format!(":fonts move {dx} {dy}"),
            Self::Resize { dw, dh } => format!(":fonts resize {dw} {dh}"),
            Self::SetRect { x, y, w, h } => format!(":fonts rect {x} {y} {w} {h}"),
        }
    }
}

/// One row in the fonts list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontsWindowRow {
    pub index: usize,
    pub family: FontFamilyPreset,
    pub display_name: String,
    pub badge: String,
    pub active_on_focused: bool,
    pub sample: &'static str,
}

/// Title-bar clickable badges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontsWindowBadge {
    Scan,
    Pin,
    Dock,
    Close,
}

/// Hit-test target under pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontsWindowHit {
    Outside,
    Title,
    Badge(FontsWindowBadge),
    Border(WindowResizeEdge),
    Subheader,
    Row(usize),
    Footer,
}

/// State of the `FONTS` utility window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontsWindowState {
    pub active: bool,
    pub focused: bool,
    pub pinned: bool,
    pub docked: bool,
    pub selected: usize,
    pub scroll: usize,
    pub float_pos: Option<(u16, u16)>,
    pub float_size: Option<(u16, u16)>,
    pub drag: Option<crate::wm::agents_list::ListDrag>,
    pub last_status: Option<String>,
}

impl FontsWindowState {
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }

    pub fn blur(&mut self) {
        self.focused = false;
        self.drag = None;
    }
}

impl WindowManager {
    pub fn fonts_window(&self) -> &FontsWindowState {
        &self.fonts_window
    }

    pub fn fonts_window_mut(&mut self) -> &mut FontsWindowState {
        &mut self.fonts_window
    }

    pub fn open_fonts_window(&mut self) {
        self.fonts_window.active = true;
        self.take_keyboard(crate::wm::keyboard_focus::KeyboardOwner::Fonts);
        let rows = self.fonts_window_rows();
        if let Some(pos) = rows.iter().position(|r| r.active_on_focused) {
            self.fonts_window.selected = pos;
        } else if self.fonts_window.selected >= rows.len() && !rows.is_empty() {
            self.fonts_window.selected = 0;
        }
        self.ensure_fonts_window_selected_visible();
    }

    pub fn close_fonts_window(&mut self) {
        self.fonts_window.active = false;
        self.fonts_window.focused = false;
        self.fonts_window.drag = None;
    }

    pub fn fonts_window_rows(&self) -> Vec<FontsWindowRow> {
        let builtins = [
            FontFamilyPreset::Ibm3270, FontFamilyPreset::BorlandDosVga, FontFamilyPreset::Tektronix4014,
            FontFamilyPreset::Vt323, FontFamilyPreset::LazenbyComputer, FontFamilyPreset::LazenbyLiquid,
            FontFamilyPreset::JetBrainsMono, FontFamilyPreset::CascadiaCode, FontFamilyPreset::FiraCode,
            FontFamilyPreset::IosevkaTerm, FontFamilyPreset::IbmPlexMono, FontFamilyPreset::Hack,
            FontFamilyPreset::Consolas, FontFamilyPreset::LucidaConsole, FontFamilyPreset::CourierNew,
        ];

        let focused_family = self.active_pane().font_options().family.clone();
        let mut rows = Vec::with_capacity(builtins.len() + 4);

        for family in builtins {
            let badge = match family {
                FontFamilyPreset::JetBrainsMono | FontFamilyPreset::LazenbyComputer
                | FontFamilyPreset::LazenbyLiquid | FontFamilyPreset::Vt323 => "BUNDLED",
                FontFamilyPreset::Ibm3270 | FontFamilyPreset::BorlandDosVga
                | FontFamilyPreset::Tektronix4014 => "RETRO",
                _ => "PRESET",
            }
            .to_string();

            let active_on_focused = family == focused_family;
            let display_name = family.display_name().to_string();
            rows.push(FontsWindowRow {
                index: rows.len(),
                family,
                display_name,
                badge,
                active_on_focused,
                sample: "AaBbCc 0123 ─│┌┐",
            });
        }

        for (custom_name, _) in crate::pane::typography::registered_custom_fonts() {
            if !rows.iter().any(|r| r.display_name.eq_ignore_ascii_case(&custom_name)) {
                let family = FontFamilyPreset::Custom(custom_name.clone());
                let active_on_focused = family == focused_family;
                rows.push(FontsWindowRow {
                    index: rows.len(),
                    family,
                    display_name: custom_name,
                    badge: "CUSTOM".to_string(),
                    active_on_focused,
                    sample: "AaBbCc 0123 ─│┌┐",
                });
            }
        }

        rows
    }

    pub fn fonts_window_rect(&self, ws: Rect) -> Option<Rect> {
        if !self.fonts_window.active || ws.width < 24 || ws.height < 8 {
            return None;
        }
        let mut w = 58.min(ws.width.saturating_sub(2));
        let mut h = 18.min(ws.height.saturating_sub(2));
        if let Some((sw, sh)) = self.fonts_window.float_size {
            w = sw.clamp(20, ws.width.saturating_sub(2).max(20));
            h = sh.clamp(6, ws.height.saturating_sub(2).max(6));
        }
        let (x, y) = if let Some((px, py)) = self.fonts_window.float_pos {
            let max_x = ws.x + ws.width.saturating_sub(w);
            let max_y = ws.y + ws.height.saturating_sub(h);
            (px.clamp(ws.x, max_x), py.clamp(ws.y, max_y))
        } else {
            (
                ws.x + (ws.width.saturating_sub(w)) / 2,
                ws.y + (ws.height.saturating_sub(h)) / 2,
            )
        };
        Some(Rect::new(x, y, w, h))
    }

    pub fn ensure_fonts_window_selected_visible(&mut self) {
        let (rows, cols) = self.default_size;
        let ws = Rect::new(0, 0, cols.max(24), rows.max(8));
        let vis = self
            .fonts_window_rect(ws)
            .map(|r| r.height.saturating_sub(5).max(1) as usize)
            .unwrap_or(10);
        let sel = self.fonts_window.selected;
        if sel < self.fonts_window.scroll {
            self.fonts_window.scroll = sel;
        } else if sel >= self.fonts_window.scroll + vis {
            self.fonts_window.scroll = sel + 1 - vis;
        }
    }

    pub fn fonts_window_hit(&self, ws: Rect, col: u16, row: u16) -> FontsWindowHit {
        let Some(outer) = self.fonts_window_rect(ws) else {
            return FontsWindowHit::Outside;
        };
        if col < outer.x || col >= outer.x + outer.width || row < outer.y || row >= outer.y + outer.height {
            return FontsWindowHit::Outside;
        }

        let right = outer.x + outer.width.saturating_sub(1);
        let bottom = outer.y + outer.height.saturating_sub(1);

        if row == outer.y {
            if col == outer.x { return FontsWindowHit::Border(WindowResizeEdge::TopLeft); }
            if col == right { return FontsWindowHit::Border(WindowResizeEdge::TopRight); }
            let r_edge = outer.x + outer.width;
            if col >= r_edge.saturating_sub(4) && col < r_edge.saturating_sub(1) {
                return FontsWindowHit::Badge(FontsWindowBadge::Close);
            }
            if col >= r_edge.saturating_sub(8) && col < r_edge.saturating_sub(5) {
                return FontsWindowHit::Badge(FontsWindowBadge::Dock);
            }
            if col >= r_edge.saturating_sub(12) && col < r_edge.saturating_sub(9) {
                return FontsWindowHit::Badge(FontsWindowBadge::Pin);
            }
            if col >= r_edge.saturating_sub(21) && col < r_edge.saturating_sub(13) {
                return FontsWindowHit::Badge(FontsWindowBadge::Scan);
            }
            return FontsWindowHit::Title;
        }

        if row == bottom {
            if col == outer.x { return FontsWindowHit::Border(WindowResizeEdge::BottomLeft); }
            if col == right { return FontsWindowHit::Border(WindowResizeEdge::BottomRight); }
            return FontsWindowHit::Border(WindowResizeEdge::Bottom);
        }

        if col == outer.x { return FontsWindowHit::Border(WindowResizeEdge::Left); }
        if col == right { return FontsWindowHit::Border(WindowResizeEdge::Right); }
        if row == outer.y + 1 { return FontsWindowHit::Subheader; }
        if row == bottom.saturating_sub(1) { return FontsWindowHit::Footer; }
        if row >= outer.y + 3 && row < bottom.saturating_sub(1) {
            let row_idx = self.fonts_window.scroll + (row - (outer.y + 3)) as usize;
            return FontsWindowHit::Row(row_idx);
        }
        FontsWindowHit::Subheader
    }

    pub fn fonts_window_apply(&mut self, op: &FontsWindowOp) -> Option<String> {
        match op {
            FontsWindowOp::Open => { self.open_fonts_window(); None }
            FontsWindowOp::Close => { self.close_fonts_window(); None }
            FontsWindowOp::Toggle => {
                if self.fonts_window.active { self.close_fonts_window(); } else { self.open_fonts_window(); }
                None
            }
            FontsWindowOp::Dismiss => { self.close_fonts_window(); None }
            FontsWindowOp::Blur => { self.fonts_window.blur(); None }
            FontsWindowOp::Pin(opt) => {
                let pin = opt.unwrap_or(!self.fonts_window.pinned);
                self.fonts_window.pinned = pin;
                Some(format!("FONTS WINDOW: pinned={}", self.fonts_window.pinned))
            }
            FontsWindowOp::Dock(opt) => {
                let dock = opt.unwrap_or(!self.fonts_window.docked);
                self.fonts_window.docked = dock;
                Some(format!("FONTS WINDOW: docked={}", self.fonts_window.docked))
            }
            FontsWindowOp::Step(n) => {
                let rows = self.fonts_window_rows();
                if !rows.is_empty() {
                    let cur = self.fonts_window.selected as i32;
                    self.fonts_window.selected = (cur + n).clamp(0, (rows.len() - 1) as i32) as usize;
                    self.ensure_fonts_window_selected_visible();
                }
                None
            }
            FontsWindowOp::Page(n) => {
                let rows = self.fonts_window_rows();
                if !rows.is_empty() {
                    let cur = self.fonts_window.selected as i32;
                    self.fonts_window.selected = (cur + n * 5).clamp(0, (rows.len() - 1) as i32) as usize;
                    self.ensure_fonts_window_selected_visible();
                }
                None
            }
            FontsWindowOp::First => { self.fonts_window.selected = 0; self.fonts_window.scroll = 0; None }
            FontsWindowOp::Last => {
                let rows = self.fonts_window_rows();
                if !rows.is_empty() {
                    self.fonts_window.selected = rows.len() - 1;
                    self.ensure_fonts_window_selected_visible();
                }
                None
            }
            FontsWindowOp::Scroll(n) => {
                let rows = self.fonts_window_rows();
                let max_scroll = rows.len().saturating_sub(6);
                let cur = self.fonts_window.scroll as i32;
                self.fonts_window.scroll = (cur + n).clamp(0, max_scroll as i32) as usize;
                None
            }
            FontsWindowOp::SelectRow(idx) => {
                let rows = self.fonts_window_rows();
                if *idx < rows.len() {
                    self.fonts_window.selected = *idx;
                    self.ensure_fonts_window_selected_visible();
                }
                None
            }
            FontsWindowOp::ApplySelected => {
                let rows = self.fonts_window_rows();
                if let Some(row) = rows.get(self.fonts_window.selected) {
                    let active_id = self.active_pane_id();
                    if let Some(pane) = self.pane_mut(active_id) {
                        pane.font_options_mut().family = row.family.clone();
                    }
                    self.fonts_window_dirty = true;
                    let status = format!("PANE FONT: {}", row.display_name);
                    self.fonts_window.last_status = Some(status.clone());
                    if !self.fonts_window.pinned { self.close_fonts_window(); }
                    Some(status)
                } else { None }
            }
            FontsWindowOp::ApplyAll => {
                let rows = self.fonts_window_rows();
                if let Some(row) = rows.get(self.fonts_window.selected) {
                    for pane in self.panes.values_mut() {
                        pane.font_options_mut().family = row.family.clone();
                    }
                    self.fonts_window_dirty = true;
                    let status = format!("ALL PANES FONT: {}", row.display_name);
                    self.fonts_window.last_status = Some(status.clone());
                    if !self.fonts_window.pinned { self.close_fonts_window(); }
                    Some(status)
                } else { None }
            }
            FontsWindowOp::SizeStep(delta) => {
                let active_id = self.active_pane_id();
                let def_pt = self.config().font_size_pt;
                if let Some(pane) = self.pane_mut(active_id) {
                    let cur = pane.font_options().size_pt.unwrap_or(def_pt);
                    let new_pt = ((cur as i16) + delta).clamp(6, 72) as u16;
                    pane.font_options_mut().size_pt = Some(new_pt);
                    self.fonts_window_dirty = true;
                    let status = format!("PANE FONT SIZE: {}pt", new_pt);
                    self.fonts_window.last_status = Some(status.clone());
                    Some(status)
                } else { None }
            }
            FontsWindowOp::ScaleStep(delta) => {
                let active_id = self.active_pane_id();
                let def_pct = self.config().font_scale_pct;
                if let Some(pane) = self.pane_mut(active_id) {
                    let cur = pane.font_options().scale_pct.unwrap_or(def_pct);
                    let new_pct = ((cur as i16) + delta).clamp(50, 400) as u16;
                    pane.font_options_mut().scale_pct = Some(new_pct);
                    self.fonts_window_dirty = true;
                    let status = format!("PANE FONT SCALE: {}%", new_pct);
                    self.fonts_window.last_status = Some(status.clone());
                    Some(status)
                } else { None }
            }
            FontsWindowOp::Scan => {
                let target_dir = crate::pane::typography::default_custom_fonts_dir();
                let count = target_dir.as_ref().map(|dir| crate::pane::typography::scan_and_register_fonts_dir(dir).len()).unwrap_or(0);
                self.open_fonts_window();
                let status = format!("SCANNED FONTS: {} font(s) registered", count);
                self.fonts_window.last_status = Some(status.clone());
                Some(status)
            }
            FontsWindowOp::Move { dx, dy } => {
                let (rows, cols) = self.default_size;
                let ws = Rect::new(0, 0, cols.max(24), rows.max(8));
                if let Some(cur) = self.fonts_window_rect(ws) {
                    let nx = ((cur.x as i16) + dx).max(0) as u16;
                    let ny = ((cur.y as i16) + dy).max(0) as u16;
                    self.fonts_window.float_pos = Some((nx, ny));
                }
                None
            }
            FontsWindowOp::Resize { dw, dh } => {
                let (rows, cols) = self.default_size;
                let ws = Rect::new(0, 0, cols.max(24), rows.max(8));
                if let Some(cur) = self.fonts_window_rect(ws) {
                    let nw = ((cur.width as i16) + dw).max(20) as u16;
                    let nh = ((cur.height as i16) + dh).max(6) as u16;
                    self.fonts_window.float_size = Some((nw, nh));
                }
                None
            }
            FontsWindowOp::SetRect { x, y, w, h } => {
                self.fonts_window.float_pos = Some((*x, *y));
                self.fonts_window.float_size = Some((*w, *h));
                None
            }
        }
    }
}
