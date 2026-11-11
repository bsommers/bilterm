//! TRAFFIC MONITOR window state, operations and apply (`TMON-03`, Phase 64-03).
//!
//! A floating, pinnable, dockable utility window cloned from the CLIPBOARD window structure. All
//! operations are [`TrafficMonitorOp`]s executed through `WindowManager::apply(Action::TrafficMonitor)`.

pub mod data;

use ratatui::layout::Rect;

use crate::traffic::{Filter, GroupKey, GroupResult, Locality, SNAPSHOT_LIMIT_MAX, SortKey, SortSpec, group_by};
use crate::ui::typography::PanelTarget;
use crate::wm::agents_list::ListDrag;
use crate::wm::panel_dock::DockPanel;
use crate::wm::{Modal, WindowManager};
pub use data::TrafficQuery;
use data::{clean_filter, group_json};

pub const TRAFFIC_MIN_W: u16 = 52;
pub const TRAFFIC_MIN_H: u16 = 12;
pub const TRAFFIC_DEF_W: u16 = 84;
pub const TRAFFIC_DEF_H: u16 = 22;
/// Maximum drill-down depth.
pub const DRILL_DEPTH: usize = 4;
/// Rows requested from the meter per frame.
pub const ROW_LIMIT: usize = SNAPSHOT_LIMIT_MAX;

/// Highlighted rate column (`1`, `2`, `3` keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RateCol {
    #[default]
    R1,
    R10,
    R60,
}

impl RateCol {
    pub fn sort_key(self) -> SortKey {
        match self {
            Self::R1 => SortKey::Rate1s,
            Self::R10 => SortKey::Rate10s,
            Self::R60 => SortKey::Rate1m,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::R1 => "1s",
            Self::R10 => "10s",
            Self::R60 => "1m",
        }
    }
}

/// Operations on the TRAFFIC MONITOR window (every surface lowers to one of these).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrafficMonitorOp {
    Open,
    Close,
    Toggle,
    Dismiss,
    Blur,
    Pin(Option<bool>),
    Dock(Option<bool>),
    GroupBy(GroupKey),
    CycleGroup(i32),
    Sort(SortKey, Option<bool>),
    CycleSort(i32),
    ToggleSortDir,
    RateCol(RateCol),
    Filter(String),
    FilterClear,
    FilterFocus(bool),
    FilterChar(char),
    FilterBackspace,
    Locality(Option<Locality>),
    CycleLocality,
    Pause(Option<bool>),
    Drill,
    DrillUp,
    SelectRow(usize),
    Step(i32),
    Page(i32),
    First,
    Last,
    Scroll(i32),
    Show(Option<usize>),
    Reset,
    Enable(Option<bool>),
    Classify(Option<bool>),
    Oia(Option<bool>),
    Scale(u16),
    Move { dx: i16, dy: i16 },
    Resize { dw: i16, dh: i16 },
    SetRect { x: u16, y: u16, w: u16, h: u16 },
    SizeAuto,
}

fn onoff(v: &Option<bool>) -> &'static str {
    match v {
        Some(true) => "on",
        Some(false) => "off",
        None => "toggle",
    }
}

impl TrafficMonitorOp {
    /// Canonical `:traffic ...` form; the exact inverse of the parser.
    pub fn colon_form(&self) -> String {
        use data::{group_name, locality_name, sort_name};
        match self {
            Self::Open => ":traffic open".into(),
            Self::Close => ":traffic close".into(),
            Self::Toggle => ":traffic toggle".into(),
            Self::Dismiss => ":traffic dismiss".into(),
            Self::Blur => ":traffic blur".into(),
            Self::Pin(Some(true)) => ":traffic pin".into(),
            Self::Pin(Some(false)) => ":traffic unpin".into(),
            Self::Pin(None) => ":traffic pin toggle".into(),
            Self::Dock(Some(true)) => ":traffic dock".into(),
            Self::Dock(Some(false)) => ":traffic float".into(),
            Self::Dock(None) => ":traffic dock toggle".into(),
            Self::GroupBy(g) => format!(":traffic group {}", group_name(*g)),
            Self::CycleGroup(d) => format!(":traffic group {}", if *d >= 0 { "next" } else { "prev" }),
            Self::Sort(k, None) => format!(":traffic sort {}", sort_name(*k)),
            Self::Sort(k, Some(desc)) => {
                format!(":traffic sort {} {}", sort_name(*k), if *desc { "desc" } else { "asc" })
            }
            Self::CycleSort(d) => format!(":traffic sort {}", if *d >= 0 { "next" } else { "prev" }),
            Self::ToggleSortDir => ":traffic sort reverse".into(),
            Self::RateCol(c) => format!(":traffic rate {}", c.label()),
            Self::Filter(s) => format!(":traffic filter {s}"),
            Self::FilterClear => ":traffic clear-filter".into(),
            Self::FilterFocus(b) => format!(":traffic filter-focus {}", if *b { "on" } else { "off" }),
            Self::FilterChar(' ') => ":traffic filter-char space".into(),
            Self::FilterChar(c) => format!(":traffic filter-char {c}"),
            Self::FilterBackspace => ":traffic filter-backspace".into(),
            Self::Locality(l) => format!(":traffic locality {}", locality_name(*l)),
            Self::CycleLocality => ":traffic locality next".into(),
            Self::Pause(Some(true)) => ":traffic pause".into(),
            Self::Pause(Some(false)) => ":traffic resume".into(),
            Self::Pause(None) => ":traffic pause toggle".into(),
            Self::Drill => ":traffic drill".into(),
            Self::DrillUp => ":traffic up".into(),
            Self::SelectRow(i) => format!(":traffic select {}", i + 1),
            Self::Step(n) => format!(":traffic step {n:+}"),
            Self::Page(n) => format!(":traffic page {n:+}"),
            Self::First => ":traffic first".into(),
            Self::Last => ":traffic last".into(),
            Self::Scroll(n) => format!(":traffic scroll {n:+}"),
            Self::Show(None) => ":traffic show".into(),
            Self::Show(Some(n)) => format!(":traffic show {n}"),
            Self::Reset => ":traffic reset".into(),
            Self::Enable(v) => format!(":traffic enable {}", onoff(v)),
            Self::Classify(v) => format!(":traffic classify {}", onoff(v)),
            Self::Oia(v) => format!(":traffic oia {}", onoff(v)),
            Self::Scale(p) => format!(":traffic scale {p}"),
            Self::Move { dx, dy } => format!(":traffic move {dx} {dy}"),
            Self::Resize { dw, dh } => format!(":traffic resize {dw} {dh}"),
            Self::SetRect { x, y, w, h } => format!(":traffic rect {x} {y} {w} {h}"),
            Self::SizeAuto => ":traffic size auto".into(),
        }
    }
}

/// Persistent state of the TRAFFIC MONITOR window.
#[derive(Debug)]
pub struct TrafficMonitorState {
    /// Window is open (named `visible` because the 64-01 seam reads it).
    pub(crate) visible: bool,
    pub focused: bool,
    pub pinned: bool,
    pub docked: bool,
    pub rect: Option<Rect>,
    pub selected: usize,
    pub scroll: usize,
    pub group: GroupKey,
    pub sort: SortSpec,
    pub rate_col: RateCol,
    pub filter: Filter,
    pub filter_focused: bool,
    /// Frozen result while paused.
    pub paused: Option<GroupResult>,
    pub drill_stack: Vec<(GroupKey, String)>,
    pub drag: Option<ListDrag>,
    pub status: Option<String>,
    /// Explicit `:traffic classify` choice, restored on close instead of the config value.
    pub classify_override: Option<bool>,
}

impl Default for TrafficMonitorState {
    fn default() -> Self {
        Self {
            visible: false,
            focused: false,
            pinned: false,
            docked: false,
            rect: None,
            selected: 0,
            scroll: 0,
            group: GroupKey::Connection,
            sort: SortSpec::default(),
            rate_col: RateCol::R1,
            filter: Filter::default(),
            filter_focused: false,
            paused: None,
            drill_stack: Vec::new(),
            drag: None,
            status: None,
            classify_override: None,
        }
    }
}

impl TrafficMonitorState {
    pub fn active(&self) -> bool {
        self.visible
    }
    pub fn captures_input(&self) -> bool {
        self.visible && self.focused
    }
    /// Drops focus when another window is raised.
    pub fn blur(&mut self) {
        self.focused = false;
        self.filter_focused = false;
        self.drag = None;
    }
    fn reset_view(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }
}

fn clamp_box(r: Rect, ws: Rect) -> Rect {
    if ws.width == 0 || ws.height == 0 {
        return Rect::default();
    }
    let w = r.width.max(TRAFFIC_MIN_W.min(ws.width)).min(ws.width);
    let h = r.height.max(TRAFFIC_MIN_H.min(ws.height)).min(ws.height);
    let x = r.x.clamp(ws.x, ws.x + ws.width.saturating_sub(w));
    let y = r.y.clamp(ws.y, ws.y + ws.height.saturating_sub(h));
    Rect::new(x, y, w, h)
}

const GROUP_ORDER: [GroupKey; 7] = [
    GroupKey::Connection,
    GroupKey::Session,
    GroupKey::Window,
    GroupKey::Pane,
    GroupKey::Origin,
    GroupKey::Locality,
    GroupKey::Direction,
];
const SORT_ORDER: [SortKey; 6] =
    [SortKey::Rate1s, SortKey::Rate10s, SortKey::Rate1m, SortKey::Total, SortKey::Peak, SortKey::Name];

fn cycle<T: Copy + PartialEq>(order: &[T], cur: T, d: i32) -> T {
    let i = order.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    let step = if d >= 0 { 1 } else { -1 };
    order[(i + step).rem_euclid(order.len() as i32) as usize]
}

impl WindowManager {
    pub fn traffic_monitor(&self) -> &TrafficMonitorState {
        &self.traffic_monitor
    }
    pub fn traffic_monitor_mut(&mut self) -> &mut TrafficMonitorState {
        &mut self.traffic_monitor
    }

    /// Query describing the window's current view.
    pub fn traffic_query(&self) -> TrafficQuery {
        let st = &self.traffic_monitor;
        TrafficQuery { group: st.group, sort: st.sort, filter: st.filter.clone(), window: 60, limit: ROW_LIMIT }
    }

    /// Live grouped result for `q` (never the paused snapshot).
    pub fn traffic_group_result(&self, q: &TrafficQuery) -> GroupResult {
        let snap = self.traffic_snapshot(ROW_LIMIT);
        group_by(&snap, q.group, &self.traffic_attribution(), &q.filter, q.sort, q.limit)
    }

    /// Rows the window displays: the frozen result while paused, else live.
    pub fn traffic_rows(&self) -> GroupResult {
        match &self.traffic_monitor.paused {
            Some(r) => r.clone(),
            None => self.traffic_group_result(&self.traffic_query()),
        }
    }

    /// Canonical snapshot JSON for `q` (shared by `:traffic show` and `traffic.snapshot`).
    pub fn traffic_group_json(&self, q: &TrafficQuery) -> String {
        let res = self.traffic_group_result(q);
        let snap_ms = self.traffic_snapshot(1).at_ms;
        group_json(&res, q, snap_ms, self.config.traffic.enabled)
    }

    /// Outer rect of the window when open (`ws` in the panel's own space).
    pub fn traffic_monitor_rect(&self, ws: Rect) -> Option<Rect> {
        let st = &self.traffic_monitor;
        if !st.visible || ws.width == 0 || ws.height == 0 {
            return None;
        }
        if st.docked
            && let Some(slot) = self.panel_dock_slot(ws, DockPanel::Traffic)
        {
            return Some(slot);
        }
        if let Some(custom) = st.rect {
            return Some(clamp_box(custom, ws));
        }
        let w = TRAFFIC_DEF_W.min(ws.width.saturating_sub(2)).max(TRAFFIC_MIN_W.min(ws.width));
        let h = TRAFFIC_DEF_H.min(ws.height.saturating_sub(2)).max(TRAFFIC_MIN_H.min(ws.height));
        let x = ws.x + ws.width.saturating_sub(w) / 2;
        let y = ws.y + ws.height.saturating_sub(h) / 2;
        Some(clamp_box(Rect::new(x, y, w, h), ws))
    }

    fn traffic_visible_rows(&self) -> usize {
        let ws = self.dock_base_area();
        self.traffic_monitor_rect(ws)
            .map_or(8, |r| super::traffic_monitor_geometry::rows_capacity(r.height) as usize)
            .max(1)
    }

    fn traffic_clamp(&mut self) {
        let len = self.traffic_rows().rows.len();
        let vis = self.traffic_visible_rows();
        let st = &mut self.traffic_monitor;
        if len == 0 {
            st.selected = 0;
            st.scroll = 0;
            return;
        }
        st.selected = st.selected.min(len - 1);
        if st.scroll > st.selected {
            st.scroll = st.selected;
        }
        if st.selected >= st.scroll + vis {
            st.scroll = st.selected + 1 - vis;
        }
        st.scroll = st.scroll.min(len - 1);
    }

    fn traffic_status(&mut self, msg: String) {
        self.traffic_monitor.status = Some(msg.clone());
        self.active_pane_mut().set_status_message(Some(msg));
    }

    fn traffic_write_gate(&mut self) -> Result<(), String> {
        if self.is_api_origin() && !self.config().api.allow_config_write {
            let msg = "traffic changes over the API are disabled; set [api] allow_config_write = true in config.toml"
                .to_string();
            self.record_automation_error_msg(msg.clone());
            return Err(msg);
        }
        Ok(())
    }

    fn traffic_close(&mut self) {
        let restore = self.traffic_monitor.classify_override.unwrap_or(self.config.traffic.classify);
        self.traffic_meter.set_classify(restore);
        let st = &mut self.traffic_monitor;
        st.visible = false;
        st.focused = false;
        st.filter_focused = false;
        st.drag = None;
        st.paused = None;
        let _ = self.set_panel_docked(DockPanel::Traffic, false);
        self.traffic_monitor.docked = false;
    }

    fn traffic_open(&mut self) {
        self.modal = Modal::None;
        self.traffic_monitor.visible = true;
        self.take_keyboard(crate::wm::keyboard_focus::KeyboardOwner::TrafficMonitor);
        self.traffic_meter.set_classify(true);
    }

    /// Applies one [`TrafficMonitorOp`]; `Err` carries a user-facing refusal.
    pub(crate) fn traffic_monitor_apply(&mut self, op: TrafficMonitorOp) -> Result<(), String> {
        use TrafficMonitorOp as Op;
        match op {
            Op::Open => self.traffic_open(),
            Op::Close => self.traffic_close(),
            Op::Toggle => {
                if self.traffic_monitor.visible {
                    self.traffic_close();
                } else {
                    self.traffic_open();
                }
            }
            Op::Dismiss => {
                if self.traffic_monitor.filter_focused {
                    self.traffic_monitor.filter_focused = false;
                } else if self.traffic_monitor.pinned || self.traffic_monitor.docked {
                    self.traffic_monitor.focused = false;
                } else {
                    self.traffic_close();
                }
            }
            Op::Blur => self.traffic_monitor.blur(),
            Op::Pin(v) => self.traffic_monitor.pinned = v.unwrap_or(!self.traffic_monitor.pinned),
            Op::Dock(v) => {
                let target = v.unwrap_or(!self.traffic_monitor.docked);
                if target {
                    self.traffic_monitor.pinned = true;
                    self.traffic_monitor.visible = true;
                }
                let _ = self.set_panel_docked(DockPanel::Traffic, target);
            }
            Op::GroupBy(g) => {
                self.traffic_monitor.group = g;
                self.traffic_refreeze();
                self.traffic_monitor.reset_view();
            }
            Op::CycleGroup(d) => {
                let g = cycle(&GROUP_ORDER, self.traffic_monitor.group, d);
                return self.traffic_monitor_apply(Op::GroupBy(g));
            }
            Op::Sort(k, desc) => {
                let st = &mut self.traffic_monitor;
                st.sort = match desc {
                    Some(d) => SortSpec { key: k, descending: d },
                    None if st.sort.key == k => SortSpec { key: k, descending: !st.sort.descending },
                    None => SortSpec { key: k, descending: k != SortKey::Name },
                };
                self.traffic_refreeze();
            }
            Op::CycleSort(d) => {
                let k = cycle(&SORT_ORDER, self.traffic_monitor.sort.key, d);
                return self.traffic_monitor_apply(Op::Sort(k, Some(k != SortKey::Name)));
            }
            Op::ToggleSortDir => {
                self.traffic_monitor.sort.descending = !self.traffic_monitor.sort.descending;
                self.traffic_refreeze();
            }
            Op::RateCol(c) => {
                self.traffic_monitor.rate_col = c;
                return self.traffic_monitor_apply(Op::Sort(c.sort_key(), Some(true)));
            }
            Op::Filter(s) => {
                self.traffic_monitor.filter.text = clean_filter(&s);
                self.traffic_refreeze();
                self.traffic_monitor.reset_view();
            }
            Op::FilterClear => {
                self.traffic_monitor.filter.text.clear();
                self.traffic_refreeze();
                self.traffic_monitor.reset_view();
            }
            Op::FilterFocus(b) => {
                self.traffic_monitor.filter_focused = b;
                if b {
                    self.traffic_monitor.visible = true;
                    self.traffic_monitor.focused = true;
                }
            }
            Op::FilterChar(c) => {
                if !c.is_control() && self.traffic_monitor.filter.text.chars().count() < crate::traffic::FILTER_CAP {
                    self.traffic_monitor.filter.text.push(c);
                    self.traffic_refreeze();
                    self.traffic_monitor.reset_view();
                }
            }
            Op::FilterBackspace => {
                self.traffic_monitor.filter.text.pop();
                self.traffic_refreeze();
                self.traffic_monitor.reset_view();
            }
            Op::Locality(l) => {
                self.traffic_monitor.filter.locality = l;
                self.traffic_refreeze();
                self.traffic_monitor.reset_view();
            }
            Op::CycleLocality => {
                let order = [None, Some(Locality::Local), Some(Locality::Loopback), Some(Locality::Network)];
                let l = cycle(&order, self.traffic_monitor.filter.locality, 1);
                return self.traffic_monitor_apply(Op::Locality(l));
            }
            Op::Pause(v) => {
                let want = v.unwrap_or(self.traffic_monitor.paused.is_none());
                self.traffic_monitor.paused = None;
                if want {
                    self.traffic_monitor.paused = Some(self.traffic_group_result(&self.traffic_query()));
                }
            }
            Op::Drill => self.traffic_drill(),
            Op::DrillUp => {
                if let Some((g, text)) = self.traffic_monitor.drill_stack.pop() {
                    self.traffic_monitor.group = g;
                    self.traffic_monitor.filter.text = text;
                    self.traffic_refreeze();
                    self.traffic_monitor.reset_view();
                }
            }
            Op::SelectRow(i) => {
                self.traffic_monitor.selected = i;
                self.traffic_clamp();
            }
            Op::Step(d) => {
                self.traffic_monitor.selected = (self.traffic_monitor.selected as i64 + i64::from(d)).max(0) as usize;
                self.traffic_clamp();
            }
            Op::Page(d) => {
                let page = self.traffic_visible_rows() as i32;
                return self.traffic_monitor_apply(Op::Step(d * page));
            }
            Op::First => self.traffic_monitor.reset_view(),
            Op::Last => {
                self.traffic_monitor.selected = usize::MAX / 2;
                self.traffic_clamp();
            }
            Op::Scroll(d) => {
                let len = self.traffic_rows().rows.len();
                let st = &mut self.traffic_monitor;
                st.scroll = (st.scroll as i64 + i64::from(d)).clamp(0, len.saturating_sub(1) as i64) as usize;
                st.selected = st.selected.max(st.scroll);
            }
            Op::Show(n) => {
                let q = self.traffic_query().with_limit(n.unwrap_or(ROW_LIMIT));
                let json = self.traffic_group_json(&q);
                self.set_command_data(json);
            }
            Op::Reset => {
                self.traffic_write_gate()?;
                self.traffic_meter.reset();
                self.traffic_monitor.paused = None;
                self.traffic_monitor.reset_view();
                self.traffic_status("Traffic counters reset".into());
            }
            Op::Enable(v) => {
                self.traffic_write_gate()?;
                let on = v.unwrap_or(!self.traffic_meter.snapshot(1).enabled);
                self.traffic_meter.set_enabled(on);
                self.traffic_status(format!("Traffic metering {}", if on { "enabled" } else { "disabled" }));
            }
            Op::Classify(v) => {
                self.traffic_write_gate()?;
                let cur = self.traffic_monitor.classify_override.unwrap_or(self.config.traffic.classify);
                let on = v.unwrap_or(!cur);
                self.traffic_monitor.classify_override = Some(on);
                self.traffic_meter.set_classify(on || self.traffic_monitor.visible);
                self.traffic_status(format!("Traffic escape classification {}", if on { "on" } else { "off" }));
            }
            Op::Oia(v) => {
                self.traffic_write_gate()?;
                let on = v.unwrap_or(!self.config.traffic.oia_indicator);
                self.config_mut().traffic.oia_indicator = on;
                self.traffic_meter.set_oia(on);
                self.traffic_status(format!("OIA traffic indicator {}", if on { "on" } else { "off" }));
            }
            Op::Scale(p) => self.set_scale(PanelTarget::Traffic, p),
            Op::Move { dx, dy } => {
                let ws = self.dock_base_area();
                if let Some(cur) = self.traffic_monitor_rect(ws) {
                    let nx = (i32::from(cur.x) + i32::from(dx)).max(0) as u16;
                    let ny = (i32::from(cur.y) + i32::from(dy)).max(0) as u16;
                    self.release_traffic_dock();
                    self.traffic_monitor.rect = Some(clamp_box(Rect::new(nx, ny, cur.width, cur.height), ws));
                }
            }
            Op::Resize { dw, dh } => {
                let ws = self.dock_base_area();
                if let Some(cur) = self.traffic_monitor_rect(ws) {
                    let nw = (i32::from(cur.width) + i32::from(dw)).max(i32::from(TRAFFIC_MIN_W)) as u16;
                    let nh = (i32::from(cur.height) + i32::from(dh)).max(i32::from(TRAFFIC_MIN_H)) as u16;
                    self.release_traffic_dock();
                    self.traffic_monitor.rect = Some(clamp_box(Rect::new(cur.x, cur.y, nw, nh), ws));
                }
            }
            Op::SetRect { x, y, w, h } => {
                let ws = self.dock_base_area();
                self.release_traffic_dock();
                self.traffic_monitor.rect = Some(clamp_box(Rect::new(x, y, w, h), ws));
            }
            Op::SizeAuto => self.traffic_monitor.rect = None,
        }
        Ok(())
    }

    /// A manual move/resize releases the dock.
    fn release_traffic_dock(&mut self) {
        if self.traffic_monitor.docked {
            let _ = self.set_panel_docked(DockPanel::Traffic, false);
        }
    }

    /// While paused, a view change (group/sort/filter) re-freezes the current data under the new view.
    fn traffic_refreeze(&mut self) {
        if self.traffic_monitor.paused.is_some() {
            self.traffic_monitor.paused = Some(self.traffic_group_result(&self.traffic_query()));
        }
    }

    fn traffic_drill(&mut self) {
        let res = self.traffic_rows();
        let Some(row) = res.rows.get(self.traffic_monitor.selected).cloned() else {
            return;
        };
        let group = self.traffic_monitor.group;
        let (next, text, loc) = match group {
            GroupKey::Connection => (GroupKey::Pane, row.label.clone(), None),
            GroupKey::Window => {
                (GroupKey::Pane, row.label.split_once('/').map_or(row.label.clone(), |(_, w)| w.to_string()), None)
            }
            GroupKey::Session => (GroupKey::Window, row.label.clone(), None),
            GroupKey::Origin => (GroupKey::Connection, row.label.clone(), None),
            GroupKey::Locality => (GroupKey::Connection, String::new(), row.locality),
            GroupKey::Pane | GroupKey::Direction => {
                self.traffic_status("Nothing to drill into at this level".into());
                return;
            }
        };
        let st = &mut self.traffic_monitor;
        if st.drill_stack.len() >= DRILL_DEPTH {
            st.drill_stack.remove(0);
        }
        st.drill_stack.push((st.group, st.filter.text.clone()));
        st.group = next;
        st.filter.text = clean_filter(&text);
        if loc.is_some() {
            st.filter.locality = loc;
        }
        st.reset_view();
        self.traffic_refreeze();
    }
}
