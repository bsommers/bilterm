//! TRAFFIC MONITOR window geometry: the single source of rect, layout and hit-testing used by both
//! the renderers and the mouse planner (Phase 64-03). All rects live in the panel's own (possibly
//! scaled virtual) space.

use ratatui::layout::Rect;

use crate::traffic::{GroupKey, Locality, SortKey};
use crate::wm::{WindowManager, WindowResizeEdge};

/// Title-bar badge targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficBadge {
    Pin,
    Dock,
    Close,
}

/// Clickable footer chips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficChip {
    Pause,
    Reset,
    Drill,
}

/// Footer chip labels (renderer and hit-test share them).
pub const FOOTER_CHIPS: [(TrafficChip, &str); 3] =
    [(TrafficChip::Pause, "[p Pause]"), (TrafficChip::Reset, "[x Reset]"), (TrafficChip::Drill, "[Enter Drill]")];

/// Group tabs in display order.
pub const TABS: [(GroupKey, &str); 7] = [
    (GroupKey::Connection, "Conn"),
    (GroupKey::Session, "Sess"),
    (GroupKey::Window, "Win"),
    (GroupKey::Pane, "Pane"),
    (GroupKey::Origin, "Orig"),
    (GroupKey::Locality, "Loc"),
    (GroupKey::Direction, "Dir"),
];

/// Locality chips in display order.
pub const LOC_CHIPS: [(Option<Locality>, &str); 4] = [
    (None, "[All]"),
    (Some(Locality::Local), "[Local]"),
    (Some(Locality::Loopback), "[Loop]"),
    (Some(Locality::Network), "[Net]"),
];

/// Table columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    Name,
    Loc,
    Rx1,
    Tx1,
    R10,
    R60,
    TotRx,
    TotTx,
    Peak,
    Esc,
    Ev,
    Spark,
}

impl Col {
    pub fn title(self) -> &'static str {
        match self {
            Self::Name => "NAME",
            Self::Loc => "LOC",
            Self::Rx1 => "RX/s",
            Self::Tx1 => "TX/s",
            Self::R10 => "10s",
            Self::R60 => "1m",
            Self::TotRx => "TOT RX",
            Self::TotTx => "TOT TX",
            Self::Peak => "PEAK",
            Self::Esc => "ESC%",
            Self::Ev => "EV/s",
            Self::Spark => "SPARK",
        }
    }
    pub fn sort_key(self) -> Option<SortKey> {
        match self {
            Self::Name => Some(SortKey::Name),
            Self::Rx1 | Self::Tx1 => Some(SortKey::Rate1s),
            Self::R10 => Some(SortKey::Rate10s),
            Self::R60 => Some(SortKey::Rate1m),
            Self::TotRx | Self::TotTx => Some(SortKey::Total),
            Self::Peak => Some(SortKey::Peak),
            _ => None,
        }
    }
    fn width(self) -> u16 {
        match self {
            Self::Name => 14,
            Self::Loc => 4,
            Self::Esc => 5,
            Self::Ev => 7,
            Self::Spark => 11,
            _ => 8,
        }
    }
}

/// Inclusion priority when the window is narrow (most important first).
const PRIORITY: [Col; 11] = [
    Col::Rx1,
    Col::Tx1,
    Col::Spark,
    Col::Loc,
    Col::R60,
    Col::TotRx,
    Col::TotTx,
    Col::R10,
    Col::Peak,
    Col::Esc,
    Col::Ev,
];
const ORDER: [Col; 12] = [
    Col::Name,
    Col::Loc,
    Col::Rx1,
    Col::Tx1,
    Col::R10,
    Col::R60,
    Col::TotRx,
    Col::TotTx,
    Col::Peak,
    Col::Esc,
    Col::Ev,
    Col::Spark,
];

/// Mouse hit-test regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficHit {
    Outside,
    Title,
    Badge(TrafficBadge),
    Edge(WindowResizeEdge),
    GroupTab(GroupKey),
    Header(SortKey),
    LocalityChip(Option<Locality>),
    Filter,
    Row(usize),
    Chip(TrafficChip),
    Body,
}

/// Rows that fit in a window of outer height `h`.
pub fn rows_capacity(h: u16) -> u16 {
    h.saturating_sub(8).max(1)
}

/// Resolved layout of one window (absolute cells).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrafficLayout {
    pub outer: Rect,
    pub inner: Rect,
    pub header_y: u16,
    pub tabs_y: u16,
    pub filter_y: u16,
    pub colhead_y: u16,
    pub rows_y: u16,
    pub rows_h: u16,
    pub detail_y: u16,
    pub footer_y: u16,
    /// `(column, x, width)` in display order, x absolute.
    pub columns: Vec<(Col, u16, u16)>,
    pub tabs: Vec<(GroupKey, u16, u16)>,
    pub chips: Vec<(Option<Locality>, u16, u16)>,
    /// Filter box `(x, width)`.
    pub filter: (u16, u16),
    pub footer: Vec<(TrafficChip, u16, u16)>,
    pub badges: Vec<(TrafficBadge, u16)>,
}

impl TrafficLayout {
    pub fn compute(outer: Rect) -> Self {
        let inner = Rect::new(outer.x + 1, outer.y + 1, outer.width.saturating_sub(2), outer.height.saturating_sub(2));
        let rows_h = rows_capacity(outer.height);
        let rows_y = inner.y + 4;
        let iw = inner.width;

        let mut take = vec![];
        let mut used = 0u16;
        for c in PRIORITY {
            if iw.saturating_sub(Col::Name.width()) >= used + c.width() {
                used += c.width();
                take.push(c);
            }
        }
        let mut columns = Vec::new();
        let mut x = inner.x;
        for c in ORDER {
            let w = if c == Col::Name {
                iw.saturating_sub(used)
            } else if take.contains(&c) {
                c.width()
            } else {
                continue;
            };
            columns.push((c, x, w));
            x += w;
        }

        let mut tabs = Vec::new();
        let mut tx = inner.x;
        for (g, l) in TABS {
            let w = l.len() as u16 + 2;
            tabs.push((g, tx, w));
            tx += w + 1;
        }
        let chips_w: u16 = LOC_CHIPS.iter().map(|(_, l)| l.len() as u16 + 1).sum();
        let cx0 = inner.x + iw.saturating_sub(chips_w);
        let mut chips = Vec::new();
        let mut cx = cx0;
        for (l, t) in LOC_CHIPS {
            chips.push((l, cx, t.len() as u16));
            cx += t.len() as u16 + 1;
        }
        let mut footer = Vec::new();
        let mut fx = inner.x;
        for (c, t) in FOOTER_CHIPS {
            footer.push((c, fx, t.len() as u16));
            fx += t.len() as u16 + 1;
        }
        let badges = vec![
            (TrafficBadge::Pin, outer.x + outer.width.saturating_sub(12)),
            (TrafficBadge::Dock, outer.x + outer.width.saturating_sub(8)),
            (TrafficBadge::Close, outer.x + outer.width.saturating_sub(4)),
        ];
        Self {
            outer,
            inner,
            header_y: inner.y,
            tabs_y: inner.y + 1,
            filter_y: inner.y + 2,
            colhead_y: inner.y + 3,
            rows_y,
            rows_h,
            detail_y: rows_y + rows_h,
            footer_y: rows_y + rows_h + 1,
            columns,
            tabs,
            chips,
            filter: (inner.x, cx0.saturating_sub(inner.x + 1)),
            footer,
            badges,
        }
    }

    /// Pure classification of a cell; `row_count` is the number of displayed rows, `scroll` the first.
    pub fn hit(&self, col: u16, row: u16, row_count: usize, scroll: usize) -> TrafficHit {
        let o = self.outer;
        if col < o.x || col >= o.x + o.width || row < o.y || row >= o.y + o.height {
            return TrafficHit::Outside;
        }
        let (l, r) = (col == o.x, col + 1 == o.x + o.width);
        let (t, b) = (row == o.y, row + 1 == o.y + o.height);
        use WindowResizeEdge as E;
        if t {
            if l {
                return TrafficHit::Edge(E::TopLeft);
            }
            if r {
                return TrafficHit::Edge(E::TopRight);
            }
            if o.width >= 20 {
                for (bd, x) in &self.badges {
                    if col >= *x && col < *x + 3 {
                        return TrafficHit::Badge(*bd);
                    }
                }
            }
            return TrafficHit::Title;
        }
        if b {
            return TrafficHit::Edge(if l {
                E::BottomLeft
            } else if r {
                E::BottomRight
            } else {
                E::Bottom
            });
        }
        if l {
            return TrafficHit::Edge(E::Left);
        }
        if r {
            return TrafficHit::Edge(E::Right);
        }
        let within = |x: u16, w: u16| col >= x && col < x + w;
        if row == self.tabs_y {
            if let Some((g, _, _)) = self.tabs.iter().find(|(_, x, w)| within(*x, *w)) {
                return TrafficHit::GroupTab(*g);
            }
        } else if row == self.filter_y {
            if let Some((c, _, _)) = self.chips.iter().find(|(_, x, w)| within(*x, *w)) {
                return TrafficHit::LocalityChip(*c);
            }
            if within(self.filter.0, self.filter.1) {
                return TrafficHit::Filter;
            }
        } else if row == self.colhead_y {
            if let Some(k) = self.columns.iter().find(|(_, x, w)| within(*x, *w)).and_then(|(c, _, _)| c.sort_key()) {
                return TrafficHit::Header(k);
            }
        } else if row >= self.rows_y && row < self.rows_y + self.rows_h {
            let idx = scroll + usize::from(row - self.rows_y);
            if idx < row_count {
                return TrafficHit::Row(idx);
            }
        } else if row == self.footer_y
            && let Some((c, _, _)) = self.footer.iter().find(|(_, x, w)| within(*x, *w))
        {
            return TrafficHit::Chip(*c);
        }
        TrafficHit::Body
    }
}

/// Layout of the open window inside `ws`.
pub fn traffic_monitor_layout(wm: &WindowManager, ws: Rect) -> Option<TrafficLayout> {
    wm.traffic_monitor_rect(ws).map(TrafficLayout::compute)
}

/// Hit-test for `(col, row)` against the window in `ws`.
pub fn traffic_monitor_hit(wm: &WindowManager, ws: Rect, col: u16, row: u16) -> TrafficHit {
    match traffic_monitor_layout(wm, ws) {
        Some(l) => l.hit(col, row, wm.traffic_rows().rows.len(), wm.traffic_monitor().scroll),
        None => TrafficHit::Outside,
    }
}

impl WindowManager {
    /// Hit-test shortcut mirroring `clipboard_window_hit`.
    pub fn traffic_monitor_hit(&self, ws: Rect, col: u16, row: u16) -> TrafficHit {
        traffic_monitor_hit(self, ws, col, row)
    }
}
