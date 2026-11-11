//! Keyboard ownership of the side windows (`FOCUS-01`): one owner at a time, one query, one cycle.
//!
//! The auxiliary windows (AGENTS list, agent traffic, session map, clipboard, traffic monitor,
//! command window, script studio, fonts, daemon, HUD), the drawer and the MCP console each keep a
//! `focused` flag. This module is the single place that knows all of them:
//! - [`WindowManager::keyboard_owner`] answers who receives keys (key routing and the UI use it);
//! - [`WindowManager::take_keyboard`] / [`WindowManager::release_keyboard`] move ownership;
//! - [`WindowManager::settle_keyboard_owner`] repairs the invariant after any action or pointer
//!   event, so a window focused by direct state change still ends up the only owner;
//! - [`FocusOp`] (`:focus cycle|pane|<window>`, `Ctrl+Alt+O`) walks pane -> open side windows in
//!   visual order -> pane.

use ratatui::layout::Rect;

use crate::wm::WindowManager;

/// Who receives key events when no workspace modal is open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyboardOwner {
    /// The active terminal pane.
    #[default]
    Pane,
    AgentsHud,
    CommandWindow,
    Clipboard,
    TrafficMonitor,
    Fonts,
    Daemon,
    ScriptStudio,
    AgentsList,
    SessionMap,
    AgentsWindow,
    Drawer,
    McpConsole,
}

impl KeyboardOwner {
    /// Every non-pane owner in key-routing priority order (the order `input::modal` dispatches in).
    pub const WINDOWS: [KeyboardOwner; 12] = [
        Self::AgentsHud,
        Self::CommandWindow,
        Self::Clipboard,
        Self::TrafficMonitor,
        Self::Fonts,
        Self::Daemon,
        Self::ScriptStudio,
        Self::AgentsList,
        Self::SessionMap,
        Self::AgentsWindow,
        Self::Drawer,
        Self::McpConsole,
    ];

    /// `true` for the terminal pane.
    pub const fn is_pane(self) -> bool {
        matches!(self, Self::Pane)
    }

    /// Colon token (`:focus <name>`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pane => "pane",
            Self::AgentsHud => "hud",
            Self::CommandWindow => "commands",
            Self::Clipboard => "clipboard",
            Self::TrafficMonitor => "traffic",
            Self::Fonts => "fonts",
            Self::Daemon => "daemon",
            Self::ScriptStudio => "script",
            Self::AgentsList => "agents-list",
            Self::SessionMap => "sessions",
            Self::AgentsWindow => "agents",
            Self::Drawer => "drawer",
            Self::McpConsole => "console",
        }
    }

    /// Short label of the status-row badge (`KBD:<label>`).
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Pane => "PANE",
            Self::AgentsHud => "HUD",
            Self::CommandWindow => "COMMANDS",
            Self::Clipboard => "CLIPBOARD",
            Self::TrafficMonitor => "TRAFFIC",
            Self::Fonts => "FONTS",
            Self::Daemon => "DAEMON",
            Self::ScriptStudio => "SCRIPT",
            Self::AgentsList => "AGENT-LIST",
            Self::SessionMap => "SESSIONS",
            Self::AgentsWindow => "AGENT-TRAFFIC",
            Self::Drawer => "DRAWER",
            Self::McpConsole => "MCP",
        }
    }

    /// Parses a `:focus <name>` window token (the canonical name or a common alias).
    pub fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "pane" | "terminal" | "shell" => Self::Pane,
            "hud" => Self::AgentsHud,
            "commands" | "command-window" => Self::CommandWindow,
            "clipboard" | "clip" => Self::Clipboard,
            "traffic" | "netmon" => Self::TrafficMonitor,
            "fonts" => Self::Fonts,
            "daemon" => Self::Daemon,
            "script" | "studio" => Self::ScriptStudio,
            "agents-list" | "list" => Self::AgentsList,
            "sessions" | "session-map" | "map" => Self::SessionMap,
            "agents" | "agent-traffic" => Self::AgentsWindow,
            "drawer" | "navigator" => Self::Drawer,
            "console" | "mcp" => Self::McpConsole,
            _ => return None,
        })
    }
}

/// One `:focus <verb>` keyboard-ownership operation, lowered to [`crate::action::Action::KeyboardFocus`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusOp {
    /// `:focus cycle` (`Ctrl+Alt+O`) - pane -> each open side window in visual order -> pane.
    Next,
    /// `:focus cycle prev` - the same walk backwards.
    Prev,
    /// `:focus pane` - hand the keyboard back to the active pane; every window stays open.
    Pane,
    /// `:focus <window>` - give an open side window the keyboard.
    Window(KeyboardOwner),
}

impl FocusOp {
    /// The canonical colon form (the inverse of the `:focus` parser).
    pub fn colon_form(&self) -> String {
        match self {
            Self::Next => ":focus cycle".to_string(),
            Self::Prev => ":focus cycle prev".to_string(),
            Self::Pane | Self::Window(KeyboardOwner::Pane) => ":focus pane".to_string(),
            Self::Window(w) => format!(":focus {}", w.name()),
        }
    }
}

impl WindowManager {
    /// `true` when `owner` currently receives keys (its window is open and focused).
    fn owner_captures(&self, owner: KeyboardOwner) -> bool {
        match owner {
            KeyboardOwner::Pane => false,
            KeyboardOwner::AgentsHud => self.agents_hud.captures_input(),
            KeyboardOwner::CommandWindow => self.command_window.captures_input(),
            KeyboardOwner::Clipboard => self.clipboard_window.captures_input(),
            KeyboardOwner::TrafficMonitor => self.traffic_monitor.captures_input(),
            KeyboardOwner::Fonts => self.fonts_window.captures_input(),
            KeyboardOwner::Daemon => self.daemon_window.captures_input(),
            KeyboardOwner::ScriptStudio => self.script_studio.captures_input(),
            KeyboardOwner::AgentsList => self.agents_list.captures_input(),
            KeyboardOwner::SessionMap => self.session_map.captures_input(),
            KeyboardOwner::AgentsWindow => self.agents_window_captures_input(),
            KeyboardOwner::Drawer => self.drawer.focused,
            KeyboardOwner::McpConsole => self.mcp_console.captures_input(),
        }
    }

    /// `true` when `owner`'s window is on screen and can be given the keyboard.
    fn owner_open(&self, owner: KeyboardOwner) -> bool {
        match owner {
            KeyboardOwner::Pane => true,
            KeyboardOwner::AgentsHud => self.agents_hud.visible(),
            KeyboardOwner::CommandWindow => self.command_window.active,
            KeyboardOwner::Clipboard => self.clipboard_window.active,
            KeyboardOwner::TrafficMonitor => self.traffic_monitor.visible,
            KeyboardOwner::Fonts => self.fonts_window.active,
            KeyboardOwner::Daemon => self.daemon_window.active,
            KeyboardOwner::ScriptStudio => self.script_studio.active,
            KeyboardOwner::AgentsList => self.agents_list.active,
            KeyboardOwner::SessionMap => self.session_map.active,
            KeyboardOwner::AgentsWindow => self.agents_window.active,
            KeyboardOwner::Drawer => self.is_drawer_effectively_visible(),
            // An unpinned console is modal: it is not a stop of the cycle, it simply has the keys.
            KeyboardOwner::McpConsole => self.mcp_console.active && self.mcp_console.pinned,
        }
    }

    /// Releases `owner`'s keyboard without closing its window. An unpinned (modal) MCP console
    /// cannot be blurred, only closed.
    fn blur_owner(&mut self, owner: KeyboardOwner) {
        match owner {
            KeyboardOwner::Pane => {}
            KeyboardOwner::AgentsHud => self.agents_hud.blur(),
            KeyboardOwner::CommandWindow => self.command_window.blur(),
            KeyboardOwner::Clipboard => self.clipboard_window.blur(),
            KeyboardOwner::TrafficMonitor => self.traffic_monitor.blur(),
            KeyboardOwner::Fonts => self.fonts_window.blur(),
            KeyboardOwner::Daemon => self.daemon_window.focused = false,
            KeyboardOwner::ScriptStudio => self.script_studio.blur(),
            KeyboardOwner::AgentsList => self.agents_list.blur(),
            KeyboardOwner::SessionMap => self.session_map.blur(),
            KeyboardOwner::AgentsWindow => self.agents_window.blur(),
            KeyboardOwner::Drawer => {
                if self.drawer.focused {
                    self.drawer.focused = false;
                    self.reflow_default_viewport();
                }
            }
            KeyboardOwner::McpConsole => {
                if self.mcp_console.pinned {
                    self.mcp_console.focused = false;
                }
            }
        }
    }

    /// Sets `owner`'s focus flag (the window must be open).
    fn focus_owner(&mut self, owner: KeyboardOwner) {
        match owner {
            KeyboardOwner::Pane => {}
            KeyboardOwner::AgentsHud => self.agents_hud.focused = true,
            KeyboardOwner::CommandWindow => self.command_window.focused = true,
            KeyboardOwner::Clipboard => self.clipboard_window.focused = true,
            KeyboardOwner::TrafficMonitor => self.traffic_monitor.focused = true,
            KeyboardOwner::Fonts => self.fonts_window.focused = true,
            KeyboardOwner::Daemon => self.daemon_window.focused = true,
            KeyboardOwner::ScriptStudio => self.script_studio.focused = true,
            KeyboardOwner::AgentsList => self.agents_list.focused = true,
            KeyboardOwner::SessionMap => self.session_map.focused = true,
            KeyboardOwner::AgentsWindow => self.agents_window.focused = true,
            KeyboardOwner::Drawer => {
                self.drawer.open = true;
                self.drawer.focused = true;
                self.sync_drawer_selection_to_active_pane();
                self.reflow_default_viewport();
            }
            KeyboardOwner::McpConsole => self.mcp_console.focused = true,
        }
    }

    /// Outer rect of `owner`'s window in workspace cells (ordering only; panel scale is ignored).
    fn owner_rect(&self, owner: KeyboardOwner, ws: Rect) -> Option<Rect> {
        match owner {
            KeyboardOwner::Pane => None,
            KeyboardOwner::AgentsHud => self.agents_hud_rect(ws),
            KeyboardOwner::CommandWindow => self.command_window_rect(ws),
            KeyboardOwner::Clipboard => self.clipboard_window_rect(ws),
            KeyboardOwner::TrafficMonitor => self.traffic_monitor_rect(ws),
            KeyboardOwner::Fonts => self.fonts_window_rect(ws),
            KeyboardOwner::Daemon => self.daemon_window_rect(ws),
            KeyboardOwner::ScriptStudio => self.script_studio_rect(ws),
            KeyboardOwner::AgentsList => self.agents_list_rect(ws),
            KeyboardOwner::SessionMap => self.session_map_rect(ws),
            KeyboardOwner::AgentsWindow => self.agents_window_rect(ws),
            KeyboardOwner::Drawer => self.drawer_rect(ws),
            KeyboardOwner::McpConsole => self.mcp_console.panel_rect(ws),
        }
    }

    /// Who receives keys right now: the first capturing window in routing order, else the pane.
    pub fn keyboard_owner(&self) -> KeyboardOwner {
        KeyboardOwner::WINDOWS
            .into_iter()
            .find(|o| self.owner_captures(*o))
            .unwrap_or(KeyboardOwner::Pane)
    }

    /// The owner the pane chrome shows (inactive frame and cursor, `KBD:<WINDOW>` badge): the
    /// keyboard owner, except that a focused drawer leaves the pane chrome alone. The classic drawer
    /// goldens (`tests/drawer_classic_parity.rs`, immutable since Phase 50) pin the frame with the
    /// drawer focused; the drawer shows its focus on its own frame.
    pub fn keyboard_owner_shown(&self) -> KeyboardOwner {
        match self.keyboard_owner() {
            KeyboardOwner::Drawer => KeyboardOwner::Pane,
            owner => owner,
        }
    }

    /// `true` while an unpinned (modal) MCP console is open: it has the keys and cannot be blurred.
    fn modal_console_open(&self) -> bool {
        self.mcp_console.active && !self.mcp_console.pinned
    }

    /// Records the owner for the next [`Self::settle_keyboard_owner`]. An open modal console is
    /// recorded as the owner even while a window above it in routing order holds the keys, so it
    /// is "new" only once.
    fn note_keyboard_owner(&mut self) {
        self.keyboard_owner_seen =
            if self.modal_console_open() { KeyboardOwner::McpConsole } else { self.keyboard_owner() };
    }

    /// Releases the keyboard from every floating window. The single list shared by pane focus
    /// changes and drawer focus, so no window can keep the keyboard next to a new owner.
    pub(crate) fn blur_floating_windows(&mut self) {
        for owner in KeyboardOwner::WINDOWS {
            if !matches!(owner, KeyboardOwner::Drawer | KeyboardOwner::McpConsole) {
                self.blur_owner(owner);
            }
        }
    }

    /// [`Self::blur_floating_windows`] plus a pinned MCP console (the drawer keeps its focus).
    pub(crate) fn blur_utility_windows(&mut self) {
        self.blur_floating_windows();
        self.blur_owner(KeyboardOwner::McpConsole);
    }

    /// Hands the keyboard back to the active pane: every side window, the drawer and a pinned MCP
    /// console are blurred; nothing is closed.
    pub fn release_keyboard(&mut self) {
        for owner in KeyboardOwner::WINDOWS {
            self.blur_owner(owner);
        }
        self.note_keyboard_owner();
    }

    /// Makes `owner` the only keyboard owner: every other window is blurred first. Returns `false`
    /// (and changes nothing) when `owner`'s window is not open.
    pub fn take_keyboard(&mut self, owner: KeyboardOwner) -> bool {
        if !self.owner_open(owner) {
            return false;
        }
        for other in KeyboardOwner::WINDOWS {
            if other != owner {
                self.blur_owner(other);
            }
        }
        if !self.owner_captures(owner) {
            self.focus_owner(owner);
        }
        self.note_keyboard_owner();
        true
    }

    /// Restores the single-owner invariant after an action or a pointer event: when more than one
    /// window claims the keyboard, the one that did not own it before (the newly focused one) wins
    /// and the rest are blurred. A modal MCP console that just opened takes it from every window.
    pub fn settle_keyboard_owner(&mut self) {
        let modal_console = self.modal_console_open();
        let claims: Vec<KeyboardOwner> = KeyboardOwner::WINDOWS
            .into_iter()
            .filter(|o| *o != KeyboardOwner::McpConsole || !modal_console)
            .filter(|o| self.owner_captures(*o))
            .collect();
        if modal_console && self.keyboard_owner_seen != KeyboardOwner::McpConsole {
            for other in claims {
                self.blur_owner(other);
            }
        } else if claims.len() > 1 {
            let keep = claims.iter().copied().find(|o| *o != self.keyboard_owner_seen).unwrap_or(claims[0]);
            for other in claims {
                if other != keep {
                    self.blur_owner(other);
                }
            }
        }
        self.note_keyboard_owner();
    }

    /// The stops of the focus cycle after the pane: open side windows left to right, then top to
    /// bottom (ties keep routing order).
    pub fn keyboard_focus_ring(&self) -> Vec<KeyboardOwner> {
        let ws = self.dock_base_area();
        let mut ring: Vec<(u16, u16, KeyboardOwner)> = KeyboardOwner::WINDOWS
            .into_iter()
            .filter(|o| self.owner_open(*o))
            .filter_map(|o| self.owner_rect(o, ws).map(|r| (r.x, r.y, o)))
            .collect();
        ring.sort_by_key(|(x, y, _)| (*x, *y));
        ring.into_iter().map(|(_, _, o)| o).collect()
    }

    /// Applies one [`FocusOp`]; returns the status message.
    pub fn keyboard_focus_apply(&mut self, op: FocusOp) -> String {
        let target = match op {
            FocusOp::Pane => KeyboardOwner::Pane,
            FocusOp::Window(w) => w,
            FocusOp::Next | FocusOp::Prev => {
                let ring = self.keyboard_focus_ring();
                if ring.is_empty() {
                    self.release_keyboard();
                    return "No side window is open".to_string();
                }
                // Position 0 is the pane, 1..=len the ring.
                let len = ring.len() + 1;
                let cur = ring.iter().position(|o| *o == self.keyboard_owner()).map_or(0, |i| i + 1);
                let next = if op == FocusOp::Next { (cur + 1) % len } else { (cur + len - 1) % len };
                if next == 0 { KeyboardOwner::Pane } else { ring[next - 1] }
            }
        };
        if target.is_pane() {
            self.release_keyboard();
            return "Keyboard: pane".to_string();
        }
        if self.take_keyboard(target) {
            format!("Keyboard: {} (Esc or Ctrl+Alt+O returns to the pane)", target.badge())
        } else {
            format!("{} is not open", target.badge())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_through_the_parser() {
        for owner in KeyboardOwner::WINDOWS.into_iter().chain([KeyboardOwner::Pane]) {
            assert_eq!(KeyboardOwner::parse(owner.name()), Some(owner));
        }
        assert_eq!(KeyboardOwner::parse("nope"), None);
    }

    #[test]
    fn a_window_focused_behind_the_helpers_back_becomes_the_only_owner() {
        let mut wm = WindowManager::new_headless(160, 40);
        wm.open_agents_list();
        wm.settle_keyboard_owner();
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::AgentsList);
        // Direct state change, as legacy mouse code does.
        wm.drawer.open = true;
        wm.drawer.focused = true;
        wm.settle_keyboard_owner();
        assert_eq!(wm.keyboard_owner(), KeyboardOwner::Drawer);
        assert!(wm.agents_list.active && !wm.agents_list.focused, "the list stays open, blurred");
    }
}
