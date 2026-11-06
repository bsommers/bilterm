//! Geometry and hit-testing of the AGENTS list window (Phase 51-02, D-03).
//!
//! Frozen by plan 51-02. Callers pass the panel's own (scaled virtual) workspace. Bug reports go to
//! the orchestrator, not to wave-3 plans.
//!
//! [`WindowManager::agents_list_layout`] is the single source of window geometry for the renderer
//! (51-06) and the mouse handler (51-07): what is drawn is exactly what is clickable, at 100% and in
//! a scaled virtual space (`panel_virtual_area` / `panel_space`). All arithmetic saturates.

use ratatui::layout::Rect;

use crate::pane::PaneId;
use crate::ui::agents_ui::agents_pin_badge_x;
use crate::wm::panel_dock::DockPanel;
use crate::wm::{WindowManager, WindowResizeEdge, rect_contains};

/// Smallest workspace in which the window exists at all.
pub const LIST_MIN_WORKSPACE: (u16, u16) = (24, 6);
/// Smallest window a manual rect can shrink to (limited by the workspace).
pub const LIST_MIN_WINDOW: (u16, u16) = (20, 6);
/// Default window width in columns (never wider than the workspace).
pub const LIST_DEFAULT_WIDTH: u16 = 36;
/// Footer chips in drawing order, left to right, each followed by one blank cell.
pub const LIST_CHIP_LABELS: [(ListChip, &str); 3] =
    [(ListChip::Filter, "[filter]"), (ListChip::Dock, "[dock]"), (ListChip::Zoom, "[zoom]")];
/// Width of the pin badge cell.
const PIN_W: u16 = 1;

/// Clickable chips of the footer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListChip {
    Filter,
    Dock,
    Zoom,
}

/// Title-bar badges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListBadge {
    Pin,
    Minimize,
    /// The zoom-badge position: docks / floats the window.
    Dock,
    Close,
}

/// What lies under a cell of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListHit {
    Outside,
    Title,
    Badge(ListBadge),
    Border(WindowResizeEdge),
    Header,
    Row(PaneId),
    Chip(ListChip),
    Body,
    Footer,
}

/// Window regions shared by rendering and hit-testing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentsListRects {
    /// Whole window including the frame.
    pub outer: Rect,
    /// Area inside the frame.
    pub inner: Rect,
    /// Top frame row (title and badges).
    pub title_row: Rect,
    /// First inner row (column titles); empty when the window is too short for header and footer.
    pub header: Rect,
    /// Rows between header and footer.
    pub rows_area: Rect,
    /// Last inner row (chips); empty when the window is too short for header and footer.
    pub footer: Rect,
    /// Number of rows that fit in `rows_area`.
    pub visible_rows: usize,
    /// Title badges (empty under 24 columns; no `Pin` when the title cannot hold it).
    pub badges: Vec<(ListBadge, Rect)>,
    /// Footer chips (empty when the footer cannot hold all three).
    pub chips: Vec<(ListChip, Rect)>,
}

/// Resize edge under a frame cell. The four corners (including the two top ones) resize; the rest
/// of the top row is the title bar (same rule as the AGENT TRAFFIC window).
fn border_edge(outer: Rect, col: u16, row: u16) -> Option<WindowResizeEdge> {
    let right = outer.x + outer.width.saturating_sub(1);
    let bottom = outer.y + outer.height.saturating_sub(1);
    if row == outer.y {
        return if col == outer.x {
            Some(WindowResizeEdge::TopLeft)
        } else if col == right {
            Some(WindowResizeEdge::TopRight)
        } else {
            None
        };
    }
    let (on_bottom, on_left, on_right) = (row == bottom, col == outer.x, col == right);
    if !(on_bottom || on_left || on_right) {
        return None;
    }
    Some(
        if (on_bottom && col >= right.saturating_sub(1)) || (on_right && row >= bottom.saturating_sub(1)) {
            WindowResizeEdge::BottomRight
        } else if (on_bottom && col <= outer.x + 1) || (on_left && row >= bottom.saturating_sub(1)) {
            WindowResizeEdge::BottomLeft
        } else if on_bottom {
            WindowResizeEdge::Bottom
        } else if on_left {
            WindowResizeEdge::Left
        } else {
            WindowResizeEdge::Right
        },
    )
}

/// Moves and shrinks `r` so it lies inside `ws` and is at least the minimum window (where `ws`
/// allows). Never zero-size for a non-empty `ws`.
fn clamp_into(r: Rect, ws: Rect) -> Rect {
    let w = r.width.clamp(LIST_MIN_WINDOW.0.min(ws.width), ws.width);
    let h = r.height.clamp(LIST_MIN_WINDOW.1.min(ws.height), ws.height);
    let x = r.x.clamp(ws.x, ws.x + (ws.width - w));
    let y = r.y.clamp(ws.y, ws.y + (ws.height - h));
    Rect::new(x, y, w, h)
}

impl WindowManager {
    /// Window rect inside `ws`, or `None` when inactive or the workspace is under 24x6.
    ///
    /// Docked: the dock slot when `panel_dock_slot` has one; manual `rect`: clamped into `ws`;
    /// default: 36 columns (never wider than `ws`), `min(rows + 4, 70% of ws height)` high with a
    /// floor of 6, right-aligned one column in from the right edge, one row below the top.
    pub fn agents_list_rect(&self, ws: Rect) -> Option<Rect> {
        let s = &self.agents_list;
        if !s.active || ws.width < LIST_MIN_WORKSPACE.0 || ws.height < LIST_MIN_WORKSPACE.1 {
            return None;
        }
        if s.docked
            && let Some(slot) = self.panel_dock_slot(ws, DockPanel::AgentsList)
        {
            let slot = slot.intersection(ws);
            if slot.width > 0 && slot.height > 0 {
                return Some(slot);
            }
        }
        if let Some(r) = s.rect {
            return Some(clamp_into(r, ws));
        }
        let width = LIST_DEFAULT_WIDTH.min(ws.width);
        let rows = u16::try_from(self.agents_list_rows().len()).unwrap_or(u16::MAX);
        let cap = u16::try_from(u32::from(ws.height) * 70 / 100).unwrap_or(u16::MAX);
        let height = rows.saturating_add(4).min(cap).max(LIST_MIN_WINDOW.1).min(ws.height);
        let x = ws.x + ws.width.saturating_sub(width.saturating_add(1));
        let y = ws.y + 1u16.min(ws.height.saturating_sub(height));
        Some(Rect::new(x, y, width, height))
    }

    /// All regions of the window inside `ws` (see [`Self::agents_list_rect`]).
    pub fn agents_list_layout(&self, ws: Rect) -> Option<AgentsListRects> {
        let outer = self.agents_list_rect(ws)?;
        let inner = Rect::new(
            outer.x.saturating_add(1),
            outer.y.saturating_add(1),
            outer.width.saturating_sub(2),
            outer.height.saturating_sub(2),
        );
        let title_row = Rect::new(outer.x, outer.y, outer.width, 1);
        let (header, rows_area, footer) = if inner.height >= 3 {
            (
                Rect::new(inner.x, inner.y, inner.width, 1),
                Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 2),
                Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
            )
        } else {
            (Rect::new(inner.x, inner.y, 0, 0), inner, Rect::new(inner.x, inner.y, 0, 0))
        };
        Some(AgentsListRects {
            outer,
            inner,
            title_row,
            header,
            rows_area,
            footer,
            visible_rows: usize::from(rows_area.height),
            badges: self.list_badges(outer),
            chips: list_chips(footer),
        })
    }

    /// Title badges: the `[_][zoom][X]` trio (cell formulas of `detect_title_bar_badge_click`) plus
    /// the pin badge (`agents_pin_badge_x`).
    fn list_badges(&self, outer: Rect) -> Vec<(ListBadge, Rect)> {
        if outer.width < 24 {
            return Vec::new();
        }
        let glyphs = self.config().glyph_set();
        let len = |s: &str| u16::try_from(s.chars().count()).unwrap_or(u16::MAX);
        let (min_len, zoom_len, close_len) =
            (len(glyphs.minimize_badge), len(glyphs.zoom_badge), len(glyphs.close_badge));
        let start = (outer.x + outer.width).saturating_sub(min_len + zoom_len + close_len + 2);
        let mut badges = Vec::with_capacity(4);
        if let Some(x) = agents_pin_badge_x(outer) {
            badges.push((ListBadge::Pin, Rect::new(x, outer.y, PIN_W, 1)));
        }
        badges.push((ListBadge::Minimize, Rect::new(start, outer.y, min_len, 1)));
        badges.push((ListBadge::Dock, Rect::new(start + min_len, outer.y, zoom_len, 1)));
        badges.push((ListBadge::Close, Rect::new(start + min_len + zoom_len, outer.y, close_len, 1)));
        badges
    }

    /// What lies under the cell `(col, row)` of a window laid out in `ws`.
    ///
    /// Precedence: badges, border (the four corners resize), title, header, chips, footer, rows,
    /// body. A row maps through `scroll` and is a `Row` only when that index exists.
    pub fn agents_list_hit(&self, ws: Rect, col: u16, row: u16) -> ListHit {
        let Some(l) = self.agents_list_layout(ws) else {
            return ListHit::Outside;
        };
        if !rect_contains(l.outer, col, row) {
            return ListHit::Outside;
        }
        if let Some((badge, _)) = l.badges.iter().find(|(_, r)| rect_contains(*r, col, row)) {
            return ListHit::Badge(*badge);
        }
        if let Some(edge) = border_edge(l.outer, col, row) {
            return ListHit::Border(edge);
        }
        if row == l.outer.y {
            return ListHit::Title;
        }
        if rect_contains(l.header, col, row) {
            return ListHit::Header;
        }
        if let Some((chip, _)) = l.chips.iter().find(|(_, r)| rect_contains(*r, col, row)) {
            return ListHit::Chip(*chip);
        }
        if rect_contains(l.footer, col, row) {
            return ListHit::Footer;
        }
        if rect_contains(l.rows_area, col, row) {
            let idx = self.agents_list.scroll.saturating_add(usize::from(row - l.rows_area.y));
            if let Some(r) = self.agents_list_rows().get(idx) {
                return ListHit::Row(r.pane_id);
            }
        }
        ListHit::Body
    }
}

/// Footer chips left to right from the footer's first cell; none when the footer cannot hold all
/// three (then the footer is a plain `Footer` hit).
fn list_chips(footer: Rect) -> Vec<(ListChip, Rect)> {
    let total: u16 = LIST_CHIP_LABELS.iter().map(|(_, l)| l.len() as u16 + 1).sum::<u16>() - 1;
    if footer.height == 0 || footer.width < total {
        return Vec::new();
    }
    let mut x = footer.x;
    LIST_CHIP_LABELS
        .iter()
        .map(|(chip, label)| {
            let w = label.len() as u16;
            let r = Rect::new(x, footer.y, w, 1);
            x += w + 1;
            (*chip, r)
        })
        .collect()
}
