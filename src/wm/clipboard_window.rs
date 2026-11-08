//! First-class non-modal `CLIPBOARD` viewer & manager window state, geometry, hit-testing, and operations (`CLIP-03`, Phase 57-02).

use ratatui::layout::Rect;

use crate::wm::WindowResizeEdge;
use crate::wm::agents_list::ListDrag;
use crate::wm::clipboard_registry::{
    ClipboardEntry, ClipboardFilterScope, ClipboardSource,
};
use crate::wm::panel_dock::DockPanel;
use crate::wm::{Modal, PaneId, WindowManager};

pub const CLIPBOARD_MIN_W: u16 = 44;
pub const CLIPBOARD_MIN_H: u16 = 12;
pub const CLIPBOARD_DEF_W: u16 = 78;
pub const CLIPBOARD_DEF_H: u16 = 22;

/// Title-bar badge targets in the `CLIPBOARD` window (`CLIP-03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardWindowBadge {
    Pin,
    Dock,
    Close,
}

/// Clickable footer action chips in the `CLIPBOARD` window (`CLIP-03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardActionChip {
    Paste,
    Yank,
    PinEntry,
    DeleteEntry,
    ClearUnpinned,
    FocusSearch,
}

/// Mouse hit-test regions for the `CLIPBOARD` window (`CLIP-03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardWindowHit {
    Outside,
    Title,
    Badge(ClipboardWindowBadge),
    Border(WindowResizeEdge),
    ScopeTab(ClipboardFilterScope),
    SessionCycle,
    WindowCycle,
    SearchRow,
    EntryRow(usize),
    PreviewArea,
    ActionChip(ClipboardActionChip),
}

/// Operations on the `CLIPBOARD` utility window and [`crate::wm::clipboard_registry::ClipboardRegistry`] (`CLIP-03`, `CLIP-04`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClipboardWindowOp {
    Open { scope: Option<ClipboardFilterScope> },
    Close,
    Toggle,
    Dismiss,
    Blur,
    Pin(Option<bool>),
    Dock(Option<bool>),
    SetScope(ClipboardFilterScope),
    CycleScope(i32),
    FilterSession(Option<String>),
    CycleSessionFilter,
    FilterWindow(Option<PaneId>),
    CycleWindowFilter,
    SearchFocus(bool),
    SearchChar(char),
    SearchBackspace,
    SearchClear,
    SearchSet(String),
    SelectRow(usize),
    Step(i32),
    Page(i32),
    First,
    Last,
    Scroll(i32),
    PreviewScroll(i32),
    PasteSelected,
    PasteIndex(usize),
    YankSelected,
    CopyText(String),
    PopYankRing,
    TogglePinSelected,
    PinEntryId(u64, Option<bool>),
    DeleteSelected,
    DeleteEntryId(u64),
    ClearUnpinned,
    SetPersist(Option<bool>),
    Move { dx: i16, dy: i16 },
    Resize { dw: i16, dh: i16 },
    SetRect { x: u16, y: u16, w: u16, h: u16 },
    SizeAuto,
    Scale(i16),
}

/// Persistent state of the `CLIPBOARD` utility window (`CLIP-03`).
#[derive(Clone, Debug, Default)]
pub struct ClipboardWindowState {
    pub active: bool,
    pub focused: bool,
    pub pinned: bool,
    pub docked: bool,
    pub scope: ClipboardFilterScope,
    pub session_filter: Option<String>,
    pub window_filter: Option<PaneId>,
    pub search_query: String,
    pub search_focused: bool,
    pub selected: usize,
    pub scroll: usize,
    pub preview_scroll: usize,
    pub rect: Option<Rect>,
    pub drag: Option<ListDrag>,
}

impl ClipboardWindowOp {
    /// Canonical colon command form for script studio recording and logs.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Open { scope: None } => ":clipboard open".to_string(),
            Self::Open { scope: Some(s) } => format!(":clipboard {}", s.label().to_ascii_lowercase()),
            Self::Close => ":clipboard close".to_string(),
            Self::Toggle => ":clipboard toggle".to_string(),
            Self::Dismiss => ":clipboard dismiss".to_string(),
            Self::Blur => ":clipboard blur".to_string(),
            Self::Pin(Some(true)) => ":clipboard pin".to_string(),
            Self::Pin(Some(false)) => ":clipboard unpin".to_string(),
            Self::Pin(None) => ":clipboard pin toggle".to_string(),
            Self::Dock(Some(true)) => ":clipboard dock".to_string(),
            Self::Dock(Some(false)) => ":clipboard float".to_string(),
            Self::Dock(None) => ":clipboard dock toggle".to_string(),
            Self::SetScope(s) => format!(":clipboard {}", s.label().to_ascii_lowercase()),
            Self::CycleScope(d) if *d >= 0 => ":clipboard tab next".to_string(),
            Self::CycleScope(_) => ":clipboard tab prev".to_string(),
            Self::FilterSession(Some(s)) => format!(":clipboard session {s}"),
            Self::FilterSession(None) => ":clipboard session".to_string(),
            Self::CycleSessionFilter => ":clipboard session cycle".to_string(),
            Self::FilterWindow(Some(w)) => format!(":clipboard window {w}"),
            Self::FilterWindow(None) => ":clipboard window".to_string(),
            Self::CycleWindowFilter => ":clipboard window cycle".to_string(),
            Self::SearchFocus(_) => ":clipboard filter".to_string(),
            Self::SearchChar(c) => format!(":clipboard filter {c}"),
            Self::SearchBackspace | Self::SearchClear => ":clipboard filter".to_string(),
            Self::SearchSet(q) => format!(":clipboard filter {q}"),
            Self::SelectRow(idx) => format!(":clipboard select {}", idx + 1),
            Self::Step(n) if *n < 0 => ":clipboard up".to_string(),
            Self::Step(_) => ":clipboard down".to_string(),
            Self::Page(n) if *n < 0 => ":clipboard page-up".to_string(),
            Self::Page(_) => ":clipboard page-down".to_string(),
            Self::First => ":clipboard first".to_string(),
            Self::Last => ":clipboard last".to_string(),
            Self::Scroll(n) => format!(":clipboard scroll {n:+}"),
            Self::PreviewScroll(n) => format!(":clipboard preview-scroll {n:+}"),
            Self::PasteSelected => ":clipboard paste".to_string(),
            Self::PasteIndex(i) => format!(":clipboard paste {i}"),
            Self::YankSelected => ":clipboard yank".to_string(),
            Self::CopyText(t) => format!(":clipboard copy {t}"),
            Self::PopYankRing => ":clipboard pop".to_string(),
            Self::TogglePinSelected => ":clipboard pin-entry".to_string(),
            Self::PinEntryId(id, _) => format!(":clipboard pin-entry {id}"),
            Self::DeleteSelected => ":clipboard delete".to_string(),
            Self::DeleteEntryId(id) => format!(":clipboard delete {id}"),
            Self::ClearUnpinned => ":clipboard clear".to_string(),
            Self::SetPersist(Some(true)) => ":clipboard persist on".to_string(),
            Self::SetPersist(Some(false)) => ":clipboard persist off".to_string(),
            Self::SetPersist(None) => ":clipboard persist".to_string(),
            Self::Move { dx, dy } => format!(":clipboard move {dx} {dy}"),
            Self::Resize { dw, dh } => format!(":clipboard resize {dw} {dh}"),
            Self::SetRect { x, y, w, h } => format!(":clipboard rect {x} {y} {w} {h}"),
            Self::SizeAuto => ":clipboard size auto".to_string(),
            Self::Scale(d) => format!(":clipboard scale {d:+}"),
        }
    }
}

impl ClipboardWindowState {
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }

    pub fn blur(&mut self) {
        self.focused = false;
        self.search_focused = false;
        self.drag = None;
    }
}

/// Computes how many entry rows fit inside `inner` and whether the bottom preview pane is visible.
pub fn clipboard_layout_rows(inner_height: u16) -> (u16, bool) {
    // Row 0: Scope tabs; Row 1: Search bar; Bottom row: Footer hints.
    // When inner_height >= 10, reserve 4 extra rows above footer (1 divider + 3 preview lines).
    if inner_height >= 10 {
        (inner_height.saturating_sub(7).max(1), true)
    } else {
        (inner_height.saturating_sub(3).max(1), false)
    }
}

fn clamp_box(r: Rect, ws: Rect) -> Rect {
    if ws.width == 0 || ws.height == 0 {
        return Rect::default();
    }
    let w = r.width.max(CLIPBOARD_MIN_W.min(ws.width)).min(ws.width);
    let h = r.height.max(CLIPBOARD_MIN_H.min(ws.height)).min(ws.height);
    let max_x = ws.x + ws.width.saturating_sub(w);
    let max_y = ws.y + ws.height.saturating_sub(h);
    let x = r.x.clamp(ws.x, max_x);
    let y = r.y.clamp(ws.y, max_y);
    Rect::new(x, y, w, h)
}

impl WindowManager {
    pub fn clipboard_window(&self) -> &ClipboardWindowState {
        &self.clipboard_window
    }

    pub fn clipboard_window_mut(&mut self) -> &mut ClipboardWindowState {
        &mut self.clipboard_window
    }

    pub fn clipboard_registry(&self) -> &crate::wm::clipboard_registry::ClipboardRegistry {
        &self.clipboard_registry
    }

    pub fn clipboard_registry_mut(&mut self) -> &mut crate::wm::clipboard_registry::ClipboardRegistry {
        &mut self.clipboard_registry
    }

    /// Drains any pending clipboard / yank / OSC 52 / media passthrough events from all panes into
    /// [`WindowManager::clipboard_registry`], keeping the workspace registry synchronized.
    pub fn sync_clipboard_records(&mut self) {
        let session_name = self.config().history_session.clone();
        let mut records = Vec::new();
        let mut media_records = Vec::new();
        for (&id, pane) in &mut self.panes {
            let title = pane.title().to_string();
            for (src, text) in pane.take_pending_clipboard_records() {
                records.push((id, title.clone(), src, text));
            }
            for (mime, bytes_len) in pane.take_pending_media_records() {
                media_records.push((id, title.clone(), mime, bytes_len));
            }
        }
        {
            let qt_id = self.quick_terminal.pane.id();
            let qt_title = self.quick_terminal.pane.title().to_string();
            for (src, text) in self.quick_terminal.pane.take_pending_clipboard_records() {
                records.push((qt_id, qt_title.clone(), src, text));
            }
            for (mime, bytes_len) in self.quick_terminal.pane.take_pending_media_records() {
                media_records.push((qt_id, qt_title.clone(), mime, bytes_len));
            }
        }
        for (pid, title, src, text) in records {
            self.clipboard_registry
                .record_text(session_name.clone(), Some(pid), title, src, text);
        }
        for (pid, title, mime, bytes_len) in media_records {
            self.clipboard_registry
                .record_media(session_name.clone(), Some(pid), title, mime, bytes_len);
        }
    }

    /// Seeds the [`ClipboardRegistry`] from the active pane's yank history if no entries exist yet
    /// for the active session.
    pub fn ensure_clipboard_registry_seeded(&mut self) {
        self.sync_clipboard_records();
        let sess = self.config().history_session.clone();
        let active_id = self.active_pane_id();
        let title = self
            .pane(active_id)
            .map(|p| p.title().to_string())
            .unwrap_or_else(|| format!("Pane #{active_id}"));
        let ring = self
            .pane(active_id)
            .map(|p| p.prompt().yank_history.clone())
            .unwrap_or_default();
        for item in ring.into_iter().rev() {
            if !self
                .clipboard_registry
                .entries()
                .iter()
                .any(|e| e.session_name == sess && e.content == item)
            {
                self.clipboard_registry.record_text(
                    sess.clone(),
                    Some(active_id),
                    title.clone(),
                    ClipboardSource::PromptCopy,
                    item,
                );
            }
        }
    }

    /// Returns the filtered [`ClipboardEntry`] references for the current `CLIPBOARD` window state.
    pub fn clipboard_window_items(&self) -> Vec<&ClipboardEntry> {
        let st = &self.clipboard_window;
        let active_sess = &self.config().history_session;
        let active_pid = self.active_pane_id();
        self.clipboard_registry.filtered_entries(
            st.scope,
            st.session_filter.as_deref(),
            st.window_filter,
            active_sess,
            active_pid,
            &st.search_query,
        )
    }

    /// Computes the outer `Rect` of the `CLIPBOARD` window when active.
    pub fn clipboard_window_rect(&self, workspace_area: Rect) -> Option<Rect> {
        if !self.clipboard_window.active || workspace_area.width == 0 || workspace_area.height == 0 {
            return None;
        }
        if self.clipboard_window.docked
            && let Some(slot) = self.panel_dock_slot(workspace_area, DockPanel::Clipboard)
        {
            return Some(slot);
        }
        if let Some(custom) = self.clipboard_window.rect {
            return Some(clamp_box(custom, workspace_area));
        }
        let w = CLIPBOARD_DEF_W.min(workspace_area.width.saturating_sub(2)).max(CLIPBOARD_MIN_W.min(workspace_area.width));
        let h = CLIPBOARD_DEF_H.min(workspace_area.height.saturating_sub(2)).max(CLIPBOARD_MIN_H.min(workspace_area.height));
        let x = workspace_area.x + workspace_area.width.saturating_sub(w) / 2;
        let y = workspace_area.y + workspace_area.height.saturating_sub(h) / 2;
        Some(clamp_box(Rect::new(x, y, w, h), workspace_area))
    }

    /// Classifies a mouse coordinate `(col, row)` against the `CLIPBOARD` window.
    pub fn clipboard_window_hit(&self, workspace_area: Rect, col: u16, row: u16) -> ClipboardWindowHit {
        let Some(outer) = self.clipboard_window_rect(workspace_area) else {
            return ClipboardWindowHit::Outside;
        };
        if col < outer.x
            || col >= outer.x + outer.width
            || row < outer.y
            || row >= outer.y + outer.height
        {
            return ClipboardWindowHit::Outside;
        }
        let on_left = col == outer.x;
        let on_right = col + 1 == outer.x + outer.width;
        let on_top = row == outer.y;
        let on_bottom = row + 1 == outer.y + outer.height;

        if on_top {
            if on_left {
                return ClipboardWindowHit::Border(WindowResizeEdge::TopLeft);
            }
            if on_right {
                return ClipboardWindowHit::Border(WindowResizeEdge::TopRight);
            }
            if outer.width >= 20 {
                let close_x = outer.x + outer.width.saturating_sub(4);
                let dock_x = outer.x + outer.width.saturating_sub(8);
                let pin_x = outer.x + outer.width.saturating_sub(12);
                if col >= close_x && col < close_x + 3 {
                    return ClipboardWindowHit::Badge(ClipboardWindowBadge::Close);
                }
                if col >= dock_x && col < dock_x + 3 {
                    return ClipboardWindowHit::Badge(ClipboardWindowBadge::Dock);
                }
                if col >= pin_x && col < pin_x + 3 {
                    return ClipboardWindowHit::Badge(ClipboardWindowBadge::Pin);
                }
            }
            return ClipboardWindowHit::Title;
        }
        if on_bottom {
            let edge = if on_left {
                WindowResizeEdge::BottomLeft
            } else if on_right {
                WindowResizeEdge::BottomRight
            } else {
                WindowResizeEdge::Bottom
            };
            return ClipboardWindowHit::Border(edge);
        }
        if on_left {
            return ClipboardWindowHit::Border(WindowResizeEdge::Left);
        }
        if on_right {
            return ClipboardWindowHit::Border(WindowResizeEdge::Right);
        }

        let inner_x = outer.x + 1;
        let inner_y = outer.y + 1;
        let inner_h = outer.height.saturating_sub(2);
        let rel_x = col.saturating_sub(inner_x);
        let rel_y = row.saturating_sub(inner_y);

        // Row 0 of inner: Scope tabs + Session/Window cycle chips
        if rel_y == 0 {
            if (0..6).contains(&rel_x) {
                return ClipboardWindowHit::ScopeTab(ClipboardFilterScope::All);
            }
            if (6..16).contains(&rel_x) {
                return ClipboardWindowHit::ScopeTab(ClipboardFilterScope::Session);
            }
            if (16..25).contains(&rel_x) {
                return ClipboardWindowHit::ScopeTab(ClipboardFilterScope::Window);
            }
            if (25..34).contains(&rel_x) {
                return ClipboardWindowHit::ScopeTab(ClipboardFilterScope::Pinned);
            }
            if (34..42).contains(&rel_x) {
                return ClipboardWindowHit::ScopeTab(ClipboardFilterScope::Media);
            }
            if (42..55).contains(&rel_x) {
                return ClipboardWindowHit::SessionCycle;
            }
            if rel_x >= 55 {
                return ClipboardWindowHit::WindowCycle;
            }
            return ClipboardWindowHit::Title;
        }

        // Row 1 of inner: Search / Filter input row
        if rel_y == 1 {
            return ClipboardWindowHit::SearchRow;
        }

        // Bottom row of inner: Footer action chips
        let footer_rel_y = inner_h.saturating_sub(1);
        if rel_y == footer_rel_y {
            if (0..14).contains(&rel_x) {
                return ClipboardWindowHit::ActionChip(ClipboardActionChip::Paste);
            }
            if (14..23).contains(&rel_x) {
                return ClipboardWindowHit::ActionChip(ClipboardActionChip::Yank);
            }
            if (23..31).contains(&rel_x) {
                return ClipboardWindowHit::ActionChip(ClipboardActionChip::PinEntry);
            }
            if (31..39).contains(&rel_x) {
                return ClipboardWindowHit::ActionChip(ClipboardActionChip::DeleteEntry);
            }
            if (39..49).contains(&rel_x) {
                return ClipboardWindowHit::ActionChip(ClipboardActionChip::ClearUnpinned);
            }
            if rel_x >= 49 {
                return ClipboardWindowHit::ActionChip(ClipboardActionChip::FocusSearch);
            }
            return ClipboardWindowHit::PreviewArea;
        }

        let (list_rows, _has_preview) = clipboard_layout_rows(inner_h);
        let list_start_rel_y = 2;
        if rel_y >= list_start_rel_y && rel_y < list_start_rel_y + list_rows {
            let row_offset = (rel_y - list_start_rel_y) as usize;
            let idx = self.clipboard_window.scroll + row_offset;
            if idx < self.clipboard_window_items().len() {
                return ClipboardWindowHit::EntryRow(idx);
            }
        }

        ClipboardWindowHit::PreviewArea
    }

    fn clamp_clipboard_selection(&mut self) {
        let len = self.clipboard_window_items().len();
        if len == 0 {
            self.clipboard_window.selected = 0;
            self.clipboard_window.scroll = 0;
        } else {
            if self.clipboard_window.selected >= len {
                self.clipboard_window.selected = len - 1;
            }
            if self.clipboard_window.scroll > self.clipboard_window.selected {
                self.clipboard_window.scroll = self.clipboard_window.selected;
            }
            let visible = 8usize;
            if self.clipboard_window.selected >= self.clipboard_window.scroll + visible {
                self.clipboard_window.scroll = self.clipboard_window.selected + 1 - visible;
            }
        }
    }

    /// Applies a [`ClipboardWindowOp`] to the workspace and `CLIPBOARD` window (`CLIP-03`, `CLIP-04`).
    pub fn clipboard_window_apply(&mut self, op: ClipboardWindowOp) {
        self.ensure_clipboard_registry_seeded();
        match op {
            ClipboardWindowOp::Open { scope } => {
                if let Some(sc) = scope {
                    self.clipboard_window.scope = sc;
                }
                self.clipboard_window.active = true;
                self.clipboard_window.focused = true;
                self.modal = Modal::None;
                self.clamp_clipboard_selection();
                let sess = self.config().history_session.clone();
                let persist = if self.config().clipboard_history_persist {
                    "ON"
                } else {
                    "OFF"
                };
                let count = self.clipboard_window_items().len();
                let scope_label = self.clipboard_window.scope.label();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Session '{sess}' clipboard viewer: {count} entry(s) [scope={scope_label}, persist={persist}]",
                )));
            }
            ClipboardWindowOp::Close => {
                self.clipboard_window.active = false;
                self.clipboard_window.focused = false;
                self.clipboard_window.search_focused = false;
                self.clipboard_window.drag = None;
            }
            ClipboardWindowOp::Toggle => {
                if self.clipboard_window.active {
                    self.clipboard_window_apply(ClipboardWindowOp::Close);
                } else {
                    self.clipboard_window_apply(ClipboardWindowOp::Open { scope: None });
                }
            }
            ClipboardWindowOp::Dismiss => {
                if self.clipboard_window.search_focused {
                    self.clipboard_window.search_focused = false;
                } else if self.clipboard_window.pinned || self.clipboard_window.docked {
                    self.clipboard_window.focused = false;
                } else {
                    self.clipboard_window_apply(ClipboardWindowOp::Close);
                }
            }
            ClipboardWindowOp::Blur => {
                self.clipboard_window.focused = false;
                self.clipboard_window.search_focused = false;
            }
            ClipboardWindowOp::Pin(val) => {
                self.clipboard_window.pinned = val.unwrap_or(!self.clipboard_window.pinned);
            }
            ClipboardWindowOp::Dock(val) => {
                let target = val.unwrap_or(!self.clipboard_window.docked);
                if target {
                    self.clipboard_window.pinned = true;
                    self.clipboard_window.active = true;
                }
                let _ = self.set_panel_docked(DockPanel::Clipboard, target);
            }
            ClipboardWindowOp::SetScope(scope) => {
                self.clipboard_window.scope = scope;
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
                self.clipboard_window.preview_scroll = 0;
            }
            ClipboardWindowOp::CycleScope(dir) => {
                self.clipboard_window.scope = if dir >= 0 {
                    self.clipboard_window.scope.next()
                } else {
                    self.clipboard_window.scope.prev()
                };
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
                self.clipboard_window.preview_scroll = 0;
            }
            ClipboardWindowOp::FilterSession(sess) => {
                self.clipboard_window.session_filter = sess.filter(|s| !s.is_empty() && s != "*");
                if self.clipboard_window.session_filter.is_some() {
                    self.clipboard_window.scope = ClipboardFilterScope::Session;
                }
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
            }
            ClipboardWindowOp::CycleSessionFilter => {
                let sessions = self.clipboard_registry.distinct_sessions();
                let next = match &self.clipboard_window.session_filter {
                    None => sessions.first().cloned(),
                    Some(curr) => {
                        let idx = sessions.iter().position(|s| s == curr);
                        match idx {
                            Some(i) if i + 1 < sessions.len() => Some(sessions[i + 1].clone()),
                            _ => None,
                        }
                    }
                };
                self.clipboard_window.session_filter = next;
                self.clamp_clipboard_selection();
            }
            ClipboardWindowOp::FilterWindow(pid) => {
                self.clipboard_window.window_filter = pid;
                if pid.is_some() {
                    self.clipboard_window.scope = ClipboardFilterScope::Window;
                }
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
            }
            ClipboardWindowOp::CycleWindowFilter => {
                let wins = self.clipboard_registry.distinct_windows();
                let next = match self.clipboard_window.window_filter {
                    None => wins.first().map(|(id, _)| *id),
                    Some(curr) => {
                        let idx = wins.iter().position(|(id, _)| *id == curr);
                        match idx {
                            Some(i) if i + 1 < wins.len() => Some(wins[i + 1].0),
                            _ => None,
                        }
                    }
                };
                self.clipboard_window.window_filter = next;
                self.clamp_clipboard_selection();
            }
            ClipboardWindowOp::SearchFocus(focused) => {
                self.clipboard_window.search_focused = focused;
                if focused {
                    self.clipboard_window.active = true;
                    self.clipboard_window.focused = true;
                }
            }
            ClipboardWindowOp::SearchChar(c) => {
                if self.clipboard_window.search_query.len() < 128 {
                    self.clipboard_window.search_query.push(c);
                    self.clipboard_window.selected = 0;
                    self.clipboard_window.scroll = 0;
                }
            }
            ClipboardWindowOp::SearchBackspace => {
                self.clipboard_window.search_query.pop();
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
            }
            ClipboardWindowOp::SearchClear => {
                self.clipboard_window.search_query.clear();
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
            }
            ClipboardWindowOp::SearchSet(q) => {
                self.clipboard_window.search_query = q.chars().take(128).collect();
                self.clipboard_window.active = true;
                self.clipboard_window.focused = true;
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
            }
            ClipboardWindowOp::SelectRow(idx) => {
                self.clipboard_window.selected = idx;
                self.clipboard_window.preview_scroll = 0;
                self.clamp_clipboard_selection();
            }
            ClipboardWindowOp::Step(delta) => {
                let len = self.clipboard_window_items().len();
                if len > 0 {
                    let cur = self.clipboard_window.selected as i32;
                    self.clipboard_window.selected = (cur + delta).clamp(0, len as i32 - 1) as usize;
                    self.clipboard_window.preview_scroll = 0;
                    self.clamp_clipboard_selection();
                }
            }
            ClipboardWindowOp::Page(dir) => {
                self.clipboard_window_apply(ClipboardWindowOp::Step(dir * 6));
            }
            ClipboardWindowOp::First => {
                self.clipboard_window.selected = 0;
                self.clipboard_window.scroll = 0;
                self.clipboard_window.preview_scroll = 0;
            }
            ClipboardWindowOp::Last => {
                let len = self.clipboard_window_items().len();
                self.clipboard_window.selected = len.saturating_sub(1);
                self.clipboard_window.preview_scroll = 0;
                self.clamp_clipboard_selection();
            }
            ClipboardWindowOp::Scroll(delta) => {
                let len = self.clipboard_window_items().len();
                if len > 0 {
                    let cur = self.clipboard_window.scroll as i32;
                    self.clipboard_window.scroll = (cur + delta).clamp(0, len as i32 - 1) as usize;
                    self.clipboard_window.selected = self
                        .clipboard_window
                        .selected
                        .clamp(self.clipboard_window.scroll, len - 1);
                }
            }
            ClipboardWindowOp::PreviewScroll(delta) => {
                let next = (self.clipboard_window.preview_scroll as i32 + delta).max(0) as usize;
                self.clipboard_window.preview_scroll = next;
            }
            ClipboardWindowOp::PasteSelected => {
                let selected_idx = self.clipboard_window.selected;
                let entry_opt = self
                    .clipboard_window_items()
                    .get(selected_idx)
                    .map(|e| (*e).clone());
                if let Some(entry) = entry_opt {
                    self.paste_clipboard_entry(&entry);
                    if !self.clipboard_window.pinned && !self.clipboard_window.docked {
                        self.clipboard_window.active = false;
                        self.clipboard_window.focused = false;
                    }
                } else {
                    self.active_pane_mut()
                        .set_status_message(Some("No clipboard entry selected".to_string()));
                }
            }
            ClipboardWindowOp::PasteIndex(idx_1) => {
                let idx0 = idx_1.saturating_sub(1);
                let entry_opt = self
                    .clipboard_window_items()
                    .get(idx0)
                    .map(|e| (*e).clone());
                if let Some(entry) = entry_opt {
                    self.paste_clipboard_entry(&entry);
                } else {
                    // Fallback to session history store if not in filtered items
                    let mut ring = self.active_pane().prompt().yank_history.clone();
                    if ring.is_empty() {
                        ring = self.active_pane().history_store().load_recent_clipboard(50);
                    }
                    if let Some(text) = ring.get(idx0).cloned() {
                        let pane = self.active_pane_mut();
                        pane.prompt_mut().yank_buffer = text.clone();
                        crate::clipboard::set_system_clipboard(&text);
                        let _ = crate::input::handle_paste_event(pane, &text);
                        pane.set_status_message(Some(format!(
                            "Pasted clipboard history #{} ({} chars)",
                            idx0 + 1,
                            text.chars().count()
                        )));
                    } else {
                        let avail = self.clipboard_window_items().len();
                        self.active_pane_mut().set_status_message(Some(format!(
                            "Clipboard history entry #{} not found ({avail} available)",
                            idx0 + 1
                        )));
                    }
                }
            }
            ClipboardWindowOp::YankSelected => {
                let selected_idx = self.clipboard_window.selected;
                if let Some(entry) = self
                    .clipboard_window_items()
                    .get(selected_idx)
                    .map(|e| (*e).clone())
                {
                    if entry.mime.is_media() {
                        self.active_pane_mut().set_status_message(Some(
                            "Media passthrough entry has no plain-text payload to yank".to_string(),
                        ));
                    } else {
                        let text = entry.content.clone();
                        let id = entry.id;
                        crate::clipboard::set_system_clipboard(&text);
                        let osc52 = crate::vt::encode_osc52_clipboard(&text);
                        let pane = self.active_pane_mut();
                        pane.prompt_mut().push_yank_entry(&text);
                        pane.set_last_osc52_sequence(Some(osc52));
                        pane.set_status_message(Some(format!(
                            "Yanked clipboard entry #{id} ({} chars) to system clipboard & OSC 52",
                            text.chars().count()
                        )));
                    }
                }
            }
            ClipboardWindowOp::CopyText(text) => {
                if !text.is_empty() {
                    self.active_pane_mut()
                        .record_yank_with_source(ClipboardSource::ExplicitCopy, &text);
                    self.sync_clipboard_records();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Added {} chars to session clipboard history (#1)",
                        text.chars().count()
                    )));
                }
            }
            ClipboardWindowOp::PopYankRing => {
                if let Some(entry) = self.active_pane_mut().prompt_mut().yank_pop() {
                    let idx = self.active_pane().prompt().yank_ring_index + 1;
                    let total = self.active_pane().prompt().yank_history.len();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Clipboard Ring [{idx}/{total}]: {}",
                        entry.replace('\n', " ↵ ")
                    )));
                } else {
                    self.active_pane_mut()
                        .set_status_message(Some("Clipboard history is empty".to_string()));
                }
            }
            ClipboardWindowOp::TogglePinSelected => {
                let selected_idx = self.clipboard_window.selected;
                if let Some(id) = self.clipboard_window_items().get(selected_idx).map(|e| e.id)
                    && let Some(pinned) = self.clipboard_registry.toggle_pin(id)
                {
                    self.clamp_clipboard_selection();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Clipboard entry #{id} {}",
                        if pinned { "pinned" } else { "unpinned" }
                    )));
                }
            }
            ClipboardWindowOp::PinEntryId(id, target) => {
                if let Some(curr) = self.clipboard_registry.get_by_id(id).map(|e| e.pinned) {
                    let want = target.unwrap_or(!curr);
                    if want != curr {
                        let _ = self.clipboard_registry.toggle_pin(id);
                    }
                    self.clamp_clipboard_selection();
                }
            }
            ClipboardWindowOp::DeleteSelected => {
                let selected_idx = self.clipboard_window.selected;
                if let Some(id) = self.clipboard_window_items().get(selected_idx).map(|e| e.id) {
                    self.clipboard_registry.delete_entry(id);
                    self.clamp_clipboard_selection();
                    self.active_pane_mut().set_status_message(Some(format!(
                        "Deleted clipboard entry #{id}"
                    )));
                }
            }
            ClipboardWindowOp::DeleteEntryId(id) => {
                if self.clipboard_registry.delete_entry(id) {
                    self.clamp_clipboard_selection();
                }
            }
            ClipboardWindowOp::ClearUnpinned => {
                let sess = self.config().history_session.clone();
                let pid = self.active_pane_id();
                let scope = self.clipboard_window.scope;
                let removed = self.clipboard_registry.clear_unpinned(scope, &sess, pid);
                if matches!(scope, ClipboardFilterScope::All | ClipboardFilterScope::Session) {
                    self.clear_session_clipboard_history();
                }
                self.clamp_clipboard_selection();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Cleared {removed} unpinned clipboard entry(s) for active session"
                )));
            }
            ClipboardWindowOp::SetPersist(val) => {
                let next = val.unwrap_or(!self.config().clipboard_history_persist);
                self.config_mut().clipboard_history_persist = next;
                self.sync_history_config();
                if next {
                    let ring = self.active_pane().prompt().yank_history.clone();
                    for entry in ring.iter().rev() {
                        let _ = self
                            .active_pane_mut()
                            .history_store_mut()
                            .record_clipboard_entry(entry);
                    }
                }
                let sess = self.config().history_session.clone();
                self.active_pane_mut().set_status_message(Some(format!(
                    "Clipboard history persistence for session '{sess}': {}",
                    if next { "ON" } else { "OFF" }
                )));
            }
            ClipboardWindowOp::Move { dx, dy } => {
                let ws = self.dock_base_area();
                if let Some(cur) = self.clipboard_window_rect(ws) {
                    let nx = (cur.x as i32 + dx as i32).max(0) as u16;
                    let ny = (cur.y as i32 + dy as i32).max(0) as u16;
                    self.clipboard_window.rect =
                        Some(clamp_box(Rect::new(nx, ny, cur.width, cur.height), ws));
                }
            }
            ClipboardWindowOp::Resize { dw, dh } => {
                let ws = self.dock_base_area();
                if let Some(cur) = self.clipboard_window_rect(ws) {
                    let nw = (cur.width as i32 + dw as i32).max(CLIPBOARD_MIN_W as i32) as u16;
                    let nh = (cur.height as i32 + dh as i32).max(CLIPBOARD_MIN_H as i32) as u16;
                    self.clipboard_window.rect =
                        Some(clamp_box(Rect::new(cur.x, cur.y, nw, nh), ws));
                }
            }
            ClipboardWindowOp::SetRect { x, y, w, h } => {
                let ws = self.dock_base_area();
                self.clipboard_window.rect = Some(clamp_box(Rect::new(x, y, w, h), ws));
            }
            ClipboardWindowOp::SizeAuto => {
                self.clipboard_window.rect = None;
            }
            ClipboardWindowOp::Scale(delta) => {
                self.step_scale(crate::ui::typography::PanelTarget::Clipboard, delta);
            }
        }
    }

    fn paste_clipboard_entry(&mut self, entry: &ClipboardEntry) {
        if entry.mime.is_media() {
            let pane = self.active_pane_mut();
            let _ = pane.write_raw_pty(&[0x16]);
            pane.set_status_message(Some(
                "Forwarded raw Ctrl+V (0x16) for clipboard media entry".to_string(),
            ));
            return;
        }
        let text = entry.content.clone();
        let pane = self.active_pane_mut();
        pane.prompt_mut().yank_buffer = text.clone();
        crate::clipboard::set_system_clipboard(&text);
        let _ = crate::input::handle_paste_event(pane, &text);
        pane.set_status_message(Some(format!(
            "Pasted clipboard entry #{} ({} chars)",
            entry.id,
            text.chars().count()
        )));
    }
}
