//! Session Packing Viewer Window State & Operations (Phase 54, VIEW-02).
//!
//! Provides the non-modal floating window state for the Session Packing Viewer (`SESSION MAP`).
//! Floats above terminal composite panes and owns keyboard input only while focused.

use std::collections::BTreeMap;
use ratatui::layout::Rect;

use crate::wm::WindowManager;
use crate::wm::WindowResizeEdge;
use crate::wm::panel_dock::DockPanel;
use crate::wm::session_pack_layout::{pack_sessions_treemap, SessionPackInput, SessionPackRect, WindowPackInput};

/// Operations dispatched to the session packing viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionMapOp {
    Open,
    Close,
    Toggle,
    Dismiss,
    Blur,
    Pin(Option<bool>),
    Dock(Option<bool>),
    StepWindow(i32),
    StepSession(i32),
    SelectWindow(usize),
    SelectSession(String),
    Pickup(Option<usize>),
    TargetSession(String),
    Drop,
    CancelDrag,
    ScaleStep(i16),
    Move { dx: i16, dy: i16 },
    Resize { dw: i16, dh: i16 },
    SetRect { x: u16, y: u16, w: u16, h: u16 },
}

impl SessionMapOp {
    /// Canonical colon form of the operation for logging, recording, and tests.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Open => ":sessions map open".to_string(),
            Self::Close => ":sessions map close".to_string(),
            Self::Toggle => ":sessions map toggle".to_string(),
            Self::Dismiss => ":sessions map dismiss".to_string(),
            Self::Blur => ":sessions map blur".to_string(),
            Self::Pin(Some(true)) => ":sessions map pin on".to_string(),
            Self::Pin(Some(false)) => ":sessions map pin off".to_string(),
            Self::Pin(None) => ":sessions map pin toggle".to_string(),
            Self::Dock(Some(true)) => ":sessions map dock on".to_string(),
            Self::Dock(Some(false)) => ":sessions map dock off".to_string(),
            Self::Dock(None) => ":sessions map dock toggle".to_string(),
            Self::StepWindow(n) => format!(":sessions map step-window {n}"),
            Self::StepSession(n) => format!(":sessions map step-session {n}"),
            Self::SelectWindow(id) => format!(":sessions map select {id}"),
            Self::SelectSession(s) => format!(":sessions map select-session {s}"),
            Self::Pickup(Some(id)) => format!(":sessions map pickup {id}"),
            Self::Pickup(None) => ":sessions map pickup".to_string(),
            Self::TargetSession(s) => format!(":sessions map target {s}"),
            Self::Drop => ":sessions map drop".to_string(),
            Self::CancelDrag => ":sessions map cancel".to_string(),
            Self::ScaleStep(n) => format!(":sessions map scale {n}"),
            Self::Move { dx, dy } => format!(":sessions map move {dx} {dy}"),
            Self::Resize { dw, dh } => format!(":sessions map resize {dw} {dh}"),
            Self::SetRect { x, y, w, h } => format!(":sessions map rect {x} {y} {w} {h}"),
        }
    }
}

/// Active pointer gesture inside the session map window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionMapDrag {
    Move { grab_dx: u16, grab_dy: u16 },
    Resize { edge: WindowResizeEdge, initial: Rect },
    WindowDrag {
        pane_id: usize,
        source_session: String,
        grab_x: u16,
        grab_y: u16,
        current_x: u16,
        current_y: u16,
        target_session: Option<String>,
    },
}

/// State of the non-modal `SESSION MAP` window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionMapState {
    pub active: bool,
    pub focused: bool,
    pub pinned: bool,
    pub docked: bool,
    pub selected_pane_id: Option<usize>,
    pub holding_pane_id: Option<usize>,
    pub target_session: Option<String>,
    pub window_session_overrides: BTreeMap<usize, String>,
    pub float_pos: (u16, u16),
    pub float_size: (u16, u16),
    pub drag: Option<SessionMapDrag>,
}

impl Default for SessionMapState {
    fn default() -> Self {
        Self {
            active: false,
            focused: false,
            pinned: false,
            docked: false,
            selected_pane_id: None,
            holding_pane_id: None,
            target_session: None,
            window_session_overrides: BTreeMap::new(),
            float_pos: (8, 3),
            float_size: (74, 22),
            drag: None,
        }
    }
}

impl SessionMapState {
    /// `true` when the window is open and owns the keyboard.
    pub fn captures_input(&self) -> bool {
        self.active && self.focused
    }

    /// Releases keyboard ownership and active drag without closing the window.
    pub fn blur(&mut self) {
        self.focused = false;
        self.drag = None;
    }

    /// Computes the window rectangle bounded within the workspace area.
    pub fn window_rect(&self, area: Rect) -> Rect {
        if area.width < 10 || area.height < 6 {
            return area;
        }
        let w = self.float_size.0.clamp(20, area.width.saturating_sub(2)).min(area.width);
        let h = self.float_size.1.clamp(8, area.height.saturating_sub(2)).min(area.height);
        let max_x = area.width.saturating_sub(w);
        let max_y = area.height.saturating_sub(h);
        let x = area.x.saturating_add(self.float_pos.0.min(max_x));
        let y = area.y.saturating_add(self.float_pos.1.min(max_y));
        Rect::new(x, y, w, h)
    }
}

impl WindowManager {
    pub fn session_map(&self) -> &SessionMapState {
        &self.session_map
    }

    pub fn session_map_mut(&mut self) -> &mut SessionMapState {
        &mut self.session_map
    }

    pub fn open_session_map(&mut self) {
        self.session_map.active = true;
        self.session_map.focused = true;
        if self.session_map.selected_pane_id.is_none() {
            self.session_map.selected_pane_id = Some(self.active_pane_id());
        }
        if self.session_map.target_session.is_none() {
            let active = if self.config.history_session.is_empty() { "default".to_string() } else { self.config.history_session.clone() };
            self.session_map.target_session = Some(active);
        }
    }

    pub fn close_session_map(&mut self) {
        self.session_map.active = false;
        self.session_map.focused = false;
        self.session_map.drag = None;
        self.session_map.holding_pane_id = None;
    }

    pub fn toggle_session_map(&mut self) {
        if self.session_map.active {
            self.close_session_map();
        } else {
            self.open_session_map();
        }
    }

    pub fn dismiss_session_map(&mut self) {
        if self.session_map.pinned {
            self.session_map.blur();
        } else {
            self.close_session_map();
        }
    }

    pub fn session_map_rect(&self, area: Rect) -> Option<Rect> {
        if !self.session_map.active {
            return None;
        }
        if self.session_map.docked
            && let Some(slot) = self.panel_dock_slot(area, DockPanel::SessionMap)
        {
            return Some(slot);
        }
        Some(self.session_map.window_rect(area))
    }

    pub fn step_session_map_window(&mut self, delta: i32) {
        let pids = self.ordered_pane_ids();
        if pids.is_empty() {
            return;
        }
        let cur_idx = self
            .session_map
            .selected_pane_id
            .and_then(|id| pids.iter().position(|&p| p == id))
            .unwrap_or(0);
        let next_idx = (cur_idx as i32 + delta).rem_euclid(pids.len() as i32) as usize;
        self.session_map.selected_pane_id = Some(pids[next_idx]);
    }

    pub fn step_session_map_session(&mut self, delta: i32) {
        let sessions = self.build_session_pack_inputs();
        if sessions.is_empty() {
            return;
        }
        let cur_idx = self
            .session_map
            .target_session
            .as_ref()
            .and_then(|s| sessions.iter().position(|in_s| in_s.session_name == *s))
            .unwrap_or(0);
        let next_idx = (cur_idx as i32 + delta).rem_euclid(sessions.len() as i32) as usize;
        self.session_map.target_session = Some(sessions[next_idx].session_name.clone());
    }

    pub fn apply_session_map_op(&mut self, op: &SessionMapOp) -> Option<String> {
        match op {
            SessionMapOp::Open => {
                self.open_session_map();
                None
            }
            SessionMapOp::Close => {
                self.close_session_map();
                None
            }
            SessionMapOp::Toggle => {
                self.toggle_session_map();
                None
            }
            SessionMapOp::Dismiss => {
                self.dismiss_session_map();
                None
            }
            SessionMapOp::Blur => {
                self.session_map.blur();
                None
            }
            SessionMapOp::Pin(opt) => {
                let on = opt.unwrap_or(!self.session_map.pinned);
                self.session_map.pinned = on;
                Some(format!("Session map pin {}", if on { "on" } else { "off" }))
            }
            SessionMapOp::Dock(opt) => {
                let on = opt.unwrap_or(!self.session_map.docked);
                self.set_panel_docked(DockPanel::SessionMap, on);
                Some(format!("Session map dock {}", if on { "on" } else { "off" }))
            }
            SessionMapOp::StepWindow(delta) => {
                self.step_session_map_window(*delta);
                None
            }
            SessionMapOp::StepSession(delta) => {
                self.step_session_map_session(*delta);
                None
            }
            SessionMapOp::SelectWindow(id) => {
                self.session_map.selected_pane_id = Some(*id);
                None
            }
            SessionMapOp::SelectSession(sess) => {
                self.session_map.target_session = Some(sess.clone());
                None
            }
            SessionMapOp::Pickup(opt_id) => {
                let id = opt_id.or(self.session_map.selected_pane_id);
                if let Some(pid) = id {
                    self.session_map.holding_pane_id = Some(pid);
                    self.session_map.selected_pane_id = Some(pid);
                    Some(format!("Picked up window #{pid}"))
                } else {
                    None
                }
            }
            SessionMapOp::TargetSession(sess) => {
                self.session_map.target_session = Some(sess.clone());
                Some(format!("Target session: {sess}"))
            }
            SessionMapOp::Drop => {
                if let (Some(pid), Some(target)) = (
                    self.session_map.holding_pane_id.take(),
                    self.session_map.target_session.as_ref().cloned(),
                ) {
                    self.session_map.window_session_overrides.insert(pid, target.clone());
                    self.session_map.drag = None;
                    Some(format!("Moved window #{pid} to session '{target}'"))
                } else {
                    self.session_map.holding_pane_id = None;
                    self.session_map.drag = None;
                    Some("Drop cancelled".to_string())
                }
            }
            SessionMapOp::CancelDrag => {
                self.session_map.holding_pane_id = None;
                self.session_map.drag = None;
                Some("Cancelled drag".to_string())
            }
            SessionMapOp::ScaleStep(delta) => {
                let n = self.step_scale(crate::ui::typography::PanelTarget::SessionMap, *delta);
                Some(format!("Session map scale {n}%"))
            }
            SessionMapOp::Move { dx, dy } => {
                let nx = (self.session_map.float_pos.0 as i32 + *dx as i32).max(0) as u16;
                let ny = (self.session_map.float_pos.1 as i32 + *dy as i32).max(0) as u16;
                self.session_map.float_pos = (nx, ny);
                None
            }
            SessionMapOp::Resize { dw, dh } => {
                let nw = (self.session_map.float_size.0 as i32 + *dw as i32).max(20) as u16;
                let nh = (self.session_map.float_size.1 as i32 + *dh as i32).max(8) as u16;
                self.session_map.float_size = (nw, nh);
                None
            }
            SessionMapOp::SetRect { x, y, w, h } => {
                self.session_map.float_pos = (*x, *y);
                self.session_map.float_size = (*w, *h);
                None
            }
        }
    }

    /// Derives session packing input slices from live window manager state.
    pub fn build_session_pack_inputs(&self) -> Vec<SessionPackInput> {
        let tree = self.session_tree();
        let active_sess = if self.config.history_session.is_empty() {
            "default".to_string()
        } else {
            self.config.history_session.clone()
        };

        let mut session_names: Vec<String> = tree.nodes.iter().map(|n| n.name.clone()).collect();
        if !session_names.contains(&active_sess) {
            session_names.insert(0, active_sess.clone());
        }
        for target in self.session_map.window_session_overrides.values() {
            if !session_names.contains(target) {
                session_names.push(target.clone());
            }
        }

        let mut session_windows: BTreeMap<String, Vec<WindowPackInput>> = BTreeMap::new();
        for &pid in &self.ordered_pane_ids() {
            if let Some(pane) = self.panes.get(&pid) {
                let target_sess = self
                    .session_map
                    .window_session_overrides
                    .get(&pid)
                    .cloned()
                    .unwrap_or_else(|| active_sess.clone());
                let title = pane.title().to_string();
                let focused = self.active_pane_id() == pid;
                session_windows.entry(target_sess.clone()).or_default().push(WindowPackInput {
                    pane_id: pid,
                    title,
                    weight: 10,
                    focused,
                });
                if !session_names.contains(&target_sess) {
                    session_names.push(target_sess);
                }
            }
        }

        let mut out = Vec::new();
        for name in session_names {
            let parent = tree.node(&name).and_then(|n| n.parent.clone());
            let windows = session_windows.remove(&name).unwrap_or_default();
            let is_active = name == active_sess;
            out.push(SessionPackInput {
                session_name: name,
                parent_session: parent,
                windows,
                active: is_active,
            });
        }
        out
    }

    /// Computes the packed session layout for the session map interior area.
    pub fn compute_session_pack_layout(&self, area: Rect) -> Vec<SessionPackRect> {
        let inputs = self.build_session_pack_inputs();
        pack_sessions_treemap(area, &inputs)
    }
}
