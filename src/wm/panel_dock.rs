//! Panel docking (Phase 51-09, D-04): `:arrange` and its aliases dock the open agents list, the
//! AGENT TRAFFIC window and (through its existing Push layout) the MCP console into a strip beside
//! the panes, so panes and panels no longer overlap.
//!
//! The strip is a pure function of live state (area, which panels are open and `docked`); no absolute
//! rect is stored, so a terminal resize keeps the tile. With no panel docked every function here is
//! an identity and the pane geometry is byte-identical to the pre-Phase-51 output.

use ratatui::layout::Rect;

use crate::ui::typography::PanelTarget;
use crate::wm::drawer::{DrawerEdge, DrawerLayoutMode};
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// Panels that can be docked into the tiling strip by `:arrange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockPanel {
    /// The AGENTS list window.
    AgentsList,
    /// The AGENT TRAFFIC window.
    AgentsTraffic,
    /// The SESSION MAP packing viewer.
    SessionMap,
}

/// Workspace width from which the strip sits on the right (narrower: at the bottom).
const WIDE_WORKSPACE: u16 = 100;
/// Smallest area that must remain for the panes (columns, rows).
const PANE_MIN: (u16, u16) = (20, 6);
const NO_ROOM: &str = "Not enough room to dock panels";

/// Strip geometry for one area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockLayout {
    /// The strip (real cells).
    pub strip: Rect,
    /// Slots in order, each inside the strip.
    pub slots: Vec<(DockPanel, Rect)>,
}

fn subtract_edge(area: Rect, edge: DrawerEdge, r: Rect) -> Rect {
    match edge {
        DrawerEdge::Left => Rect::new(area.x + r.width, area.y, area.width.saturating_sub(r.width).max(1), area.height),
        DrawerEdge::Right => Rect::new(area.x, area.y, area.width.saturating_sub(r.width).max(1), area.height),
        DrawerEdge::Top => Rect::new(area.x, area.y + r.height, area.width, area.height.saturating_sub(r.height).max(1)),
        DrawerEdge::Bottom => Rect::new(area.x, area.y, area.width, area.height.saturating_sub(r.height).max(1)),
    }
}

/// Real rect -> the scaled virtual space of a panel (`origin + (real - origin) * 100 / scale`); the
/// start rounds up and the end down so the slot never exceeds the strip.
fn to_virtual(origin: Rect, r: Rect, scale_pct: u16) -> Rect {
    let s = u32::from(scale_pct.max(1));
    let rel = |v: u16, o: u16| u32::from(v.saturating_sub(o));
    let x0 = (rel(r.x, origin.x) * 100).div_ceil(s);
    let y0 = (rel(r.y, origin.y) * 100).div_ceil(s);
    let x1 = rel(r.right(), origin.x) * 100 / s;
    let y1 = rel(r.bottom(), origin.y) * 100 / s;
    let cap = |v: u32| v.min(u32::from(u16::MAX)) as u16;
    Rect::new(
        origin.x.saturating_add(cap(x0)),
        origin.y.saturating_add(cap(y0)),
        cap(x1.saturating_sub(x0)),
        cap(y1.saturating_sub(y0)),
    )
}

impl WindowManager {
    /// Panels that are open and docked, in strip order (list first).
    fn docked_panels(&self) -> Vec<DockPanel> {
        let mut v = Vec::new();
        if self.agents_list.active && self.agents_list.docked {
            v.push(DockPanel::AgentsList);
        }
        if self.agents_window.active && self.agents_window.docked {
            v.push(DockPanel::AgentsTraffic);
        }
        if self.session_map.active && self.session_map.docked {
            v.push(DockPanel::SessionMap);
        }
        v
    }

    fn panel_scale_target(panel: DockPanel) -> PanelTarget {
        match panel {
            DockPanel::AgentsList => PanelTarget::AgentsList,
            DockPanel::AgentsTraffic => PanelTarget::Agents,
            DockPanel::SessionMap => PanelTarget::SessionMap,
        }
    }

    /// The workspace as the renderer and the mouse handler see it (menu and key bars removed).
    pub fn dock_base_area(&self) -> Rect {
        let (rows, cols) = self.default_size;
        let full = Rect::new(0, 0, cols.max(1), rows.max(1));
        crate::ui::shell::ShellLayout::compute(full, &self.config, self.panes.len()).workspace
    }

    /// `real_ws` minus the drawer (Push) and the MCP console (Push), exactly as
    /// `effective_workspace_area` computes it before it subtracts the dock strip.
    pub fn pre_dock_area(&self, real_ws: Rect) -> Rect {
        let mut area = real_ws;
        if self.config.drawer.layout == DrawerLayoutMode::Push
            && let Some(dr) = self.drawer_rect(area)
        {
            area = subtract_edge(area, self.config.drawer.edge, dr);
        }
        if self.mcp_console.active
            && self.mcp_console.layout_mode == DrawerLayoutMode::Push
            && let Some(mr) = self.mcp_console.panel_rect(area)
        {
            area = subtract_edge(area, self.mcp_console.edge, mr);
        }
        area
    }

    /// Strip and slots for `area` (real cells, drawer and MCP already removed); `None` when no panel
    /// is docked or the panes would keep less than 20x6.
    pub(crate) fn dock_layout(&self, area: Rect) -> Option<DockLayout> {
        let panels = self.docked_panels();
        if panels.is_empty() {
            return None;
        }
        let n = u16::try_from(panels.len()).unwrap_or(1);
        let right = area.width >= WIDE_WORKSPACE;
        let (strip, slots) = if right {
            let w = (area.width / 3).clamp(28, 48);
            if area.width.saturating_sub(w) < PANE_MIN.0 || area.height < PANE_MIN.1 {
                return None;
            }
            let strip = Rect::new(area.x + (area.width - w), area.y, w, area.height);
            let h = area.height / n;
            (strip, (0..n).map(|i| Rect::new(strip.x, strip.y + i * h, w, h)).collect::<Vec<_>>())
        } else {
            let h = (area.height / 3).max(6);
            if area.height.saturating_sub(h) < PANE_MIN.1 || area.width < PANE_MIN.0 {
                return None;
            }
            let strip = Rect::new(area.x, area.y + (area.height - h), area.width, h);
            let w = area.width / n;
            (strip, (0..n).map(|i| Rect::new(strip.x + i * w, strip.y, w, h)).collect::<Vec<_>>())
        };
        Some(DockLayout { strip, slots: panels.into_iter().zip(slots).collect() })
    }

    /// Removes the dock strip from `area` (real cells); identity without a docked panel.
    pub fn subtract_docked_panels(&self, area: Rect) -> Rect {
        match self.dock_layout(area) {
            None => area,
            Some(l) if l.strip.width == area.width => {
                Rect::new(area.x, area.y, area.width, area.height - l.strip.height)
            }
            Some(l) => Rect::new(area.x, area.y, area.width - l.strip.width, area.height),
        }
    }

    /// Slot of a docked `panel` in the panel's own (scaled virtual) space; `None` when the panel is
    /// not open, not docked, or the strip has no room. Independent of `ws`: the slot comes from the
    /// live workspace, so callers passing the already reduced effective area get the same answer.
    pub fn panel_dock_slot(&self, ws: Rect, panel: DockPanel) -> Option<Rect> {
        let _ = ws;
        let base = self.dock_base_area();
        let layout = self.dock_layout(self.pre_dock_area(base))?;
        let real = layout.slots.iter().find(|(p, _)| *p == panel)?.1;
        let target = Self::panel_scale_target(panel);
        Some(to_virtual(base, real, self.scale_pct(target)))
    }

    /// Area `auto_arrange_windows` lays panes out in (menu/key bars removed, drawer and MCP removed,
    /// strip removed); mirrors `arrange_workspace_area` in `layout_profile.rs`.
    fn arrange_area(&self) -> Rect {
        let (rows, cols) = self.default_size;
        let full = Rect::new(0, 0, cols.max(1), rows.max(1));
        let mut top = u16::from(self.config.show_top_menubar && full.height >= 4);
        if top == 0 && crate::ui::shell::wants_pill_bar(&self.config, self.panes.len(), full.height) {
            top = 1;
        }
        let bot = u16::from(self.config.show_dos_fkey_bar && full.height >= 4 + top);
        let ws = Rect::new(0, top, full.width, full.height.saturating_sub(top + bot));
        self.effective_workspace_area(ws)
    }

    /// `true` when docking the currently flagged panels leaves room for the panes.
    fn dock_has_room(&self) -> bool {
        self.dock_layout(self.pre_dock_area(self.dock_base_area())).is_some()
    }

    fn dock_status(&mut self, msg: &str) {
        self.active_pane_mut().set_status_message(Some(msg.to_string()));
    }

    /// Docks every open eligible panel before `:arrange` re-flows the panes (called by
    /// `auto_arrange_windows` right before `resize_all`). Does nothing when no panel is open.
    pub fn dock_panels_on_arrange(&mut self) {
        let (list, traffic, mcp) =
            (self.agents_list.active, self.agents_window.active, self.mcp_console.active);
        if !(list || traffic || mcp) {
            return;
        }
        if mcp {
            self.mcp_console.layout_mode = DrawerLayoutMode::Push;
        }
        self.agents_list.docked = list;
        self.agents_window.docked = traffic;
        if (list || traffic) && !self.dock_has_room() {
            self.agents_list.docked = false;
            self.agents_window.docked = false;
            self.dock_status(NO_ROOM);
        }
        self.regrid_floating_windows();
    }

    /// The `stack-grid` pattern stores floating rects computed before the dock decision; rebuild them
    /// from the final area so they never sit under a panel.
    fn regrid_floating_windows(&mut self) {
        if self.layout_mode != WorkspaceLayoutMode::Stacked {
            return;
        }
        let ids = self.ordered_pane_ids();
        if ids.is_empty() || !ids.iter().all(|&id| self.z_stack.has_custom_geometry(id)) {
            return;
        }
        let area = self.arrange_area();
        for (id, rect) in self.split_tree.compute_rects(area) {
            self.z_stack.set_window_rect(id, rect, area);
        }
    }

    /// The only place that flips a `docked` flag; returns whether it changed. Refuses `on` for a closed
    /// panel or without room (status `Not enough room to dock panels`), and re-flows the panes
    /// (`resize_all`) after every change.
    pub fn set_panel_docked(&mut self, panel: DockPanel, on: bool) -> bool {
        let (active, flag) = match panel {
            DockPanel::AgentsList => (self.agents_list.active, &mut self.agents_list.docked),
            DockPanel::AgentsTraffic => (self.agents_window.active, &mut self.agents_window.docked),
            DockPanel::SessionMap => (self.session_map.active, &mut self.session_map.docked),
        };
        let want = on && active;
        if *flag == want {
            if on && !active {
                self.dock_status("Open the panel before docking it");
            }
            return false;
        }
        *flag = want;
        if want && !self.dock_has_room() {
            match panel {
                DockPanel::AgentsList => self.agents_list.docked = false,
                DockPanel::AgentsTraffic => self.agents_window.docked = false,
                DockPanel::SessionMap => self.session_map.docked = false,
            }
            self.dock_status(NO_ROOM);
            return false;
        }
        self.reflow_after_dock_change();
        true
    }

    fn reflow_after_dock_change(&mut self) {
        let vp = Rect::new(0, 0, self.default_size.1, self.default_size.0);
        let _ = self.resize_all(vp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wm::SplitDirection;

    fn wm(cols: u16, rows: u16) -> WindowManager {
        let mut wm = WindowManager::new_headless(rows, cols);
        wm.split_focused(SplitDirection::Vertical).unwrap();
        wm.resize_all(Rect::new(0, 0, cols, rows)).unwrap();
        wm
    }

    #[test]
    fn layout_is_none_without_docked_panels() {
        let mut w = wm(120, 40);
        w.open_agents_list();
        assert_eq!(w.dock_layout(Rect::new(0, 0, 120, 40)), None);
        assert_eq!(w.subtract_docked_panels(Rect::new(0, 0, 120, 40)), Rect::new(0, 0, 120, 40));
    }

    #[test]
    fn strip_rules_right_bottom_and_floor() {
        let mut w = wm(120, 40);
        w.open_agents_list();
        w.agents_list.docked = true;
        let l = w.dock_layout(Rect::new(0, 0, 120, 40)).unwrap();
        assert_eq!(l.strip, Rect::new(80, 0, 40, 40));
        let l = w.dock_layout(Rect::new(0, 0, 300, 40)).unwrap();
        assert_eq!(l.strip.width, 48, "clamped at 48 columns");
        let l = w.dock_layout(Rect::new(0, 0, 99, 30)).unwrap();
        assert_eq!(l.strip, Rect::new(0, 20, 99, 10), "bottom strip: max(6, 30/3) rows");
        assert_eq!(w.dock_layout(Rect::new(0, 0, 30, 10)), None, "leftover under 20x6");
        assert_eq!(w.dock_layout(Rect::new(0, 0, 0, 0)), None, "degenerate areas never panic");
        assert_eq!(w.dock_layout(Rect::new(0, 0, u16::MAX, u16::MAX)).unwrap().strip.width, 48);
    }

    #[test]
    fn list_release_paths_via_dock_api() {
        let mut w = wm(120, 40);
        assert!(!w.set_panel_docked(DockPanel::AgentsList, true), "closed list refuses");
        assert_eq!(w.active_pane().status_message(), Some("Open the panel before docking it"));
        w.open_agents_list();
        let full = w.pane(w.active_pane_id()).unwrap().vt().size();
        assert!(w.set_panel_docked(DockPanel::AgentsList, true));
        let docked = w.pane(w.active_pane_id()).unwrap().vt().size();
        assert_ne!(full, docked, "docking reflows the panes");
        assert!(w.set_panel_docked(DockPanel::AgentsList, false));
        assert!(!w.agents_list.docked);
        assert_eq!(w.pane(w.active_pane_id()).unwrap().vt().size(), full, "release reflows back");
        assert!(!w.set_panel_docked(DockPanel::AgentsList, false), "unchanged reports false");
    }

    #[test]
    fn dock_refused_without_room_keeps_flag_off() {
        let mut w = wm(30, 10);
        w.open_agents_list();
        assert!(!w.set_panel_docked(DockPanel::AgentsList, true));
        assert!(!w.agents_list.docked);
        assert_eq!(w.active_pane().status_message(), Some(NO_ROOM));
    }

    #[test]
    fn virtual_conversion_never_exceeds_the_strip() {
        let origin = Rect::new(0, 1, 120, 40);
        for s in [75u16, 100, 125, 150, 200] {
            let v = to_virtual(origin, Rect::new(83, 1, 37, 40), s);
            let back = crate::ui::typography::scaled_panel_real_rect(origin, v, s);
            assert!(back.x.abs_diff(83) <= 1 && back.right().abs_diff(120) <= 1, "{s}: {back:?}");
        }
    }
}
