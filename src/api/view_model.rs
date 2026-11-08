//! Web view model projection & `@agent-manager/protocol` DTOs (`WEB-FE-02`, Phase 60).
//!
//! [`WorkspaceViewModelDto::from_wm`] takes one consistent snapshot of the workspace (split tree,
//! z-stack, agents, utility windows, CSS tokens) for `workspace.view_model` RPC and the REST routes.
//! Panes map to protocol `Agent`s with id `pane-<N>`.

use std::sync::OnceLock;

use super::json::JsonValue;
use super::output_ring::OutputHub;
use crate::pane::PaneId;
use crate::ui::design::AppearanceProfile;
use crate::wm::agent_state::AgentState;
use crate::wm::split_tree::SplitNode;
use crate::wm::split_tree::SplitDirection;
use crate::wm::{WindowManager, WorkspaceLayoutMode};

/// `@agent-manager/protocol` wire contract version.
pub const CONTRACT_VERSION: i64 = 1;
/// Capabilities every bilterm pane supports.
pub const AGENT_CAPABILITIES: &[&str] = &["input", "resize", "kill", "history", "broadcast"];

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Process start time (ms), fixed on first use.
pub fn server_start_ms() -> u64 {
    static START: OnceLock<u64> = OnceLock::new();
    *START.get_or_init(now_ms)
}

/// Server epoch string; changes on restart so clients know seqs were reset.
pub fn server_epoch() -> String {
    format!("bilterm-{:x}-{:x}", server_start_ms(), std::process::id())
}

/// `pane-<N>` wire id for pane `id`.
pub fn agent_id(id: PaneId) -> String {
    format!("pane-{id}")
}

/// Parses `pane-<N>` (or a bare number) back to a pane id.
pub fn parse_agent_id(s: &str) -> Option<PaneId> {
    s.strip_prefix("pane-").unwrap_or(s).parse().ok()
}

/// CSS custom properties for the workspace's active appearance profile.
pub fn css_tokens(wm: &WindowManager) -> String {
    AppearanceProfile::resolve(wm.config()).tokens().to_css_variables()
}

fn status_str(wm: &WindowManager, id: PaneId) -> &'static str {
    if wm.pane(id).is_some_and(|p| p.has_exited()) {
        return "disconnected";
    }
    match wm.agent_state(id) {
        Some(AgentState::Working) => "running",
        Some(AgentState::Blocked) => "thinking",
        Some(AgentState::Errored) => "error",
        Some(AgentState::Idle | AgentState::Done) | None => "idle",
    }
}

/// Protocol `Agent` JSON for pane `id`, or `None` when it does not exist.
pub fn agent_json(wm: &WindowManager, id: PaneId, hub: Option<&OutputHub>) -> Option<JsonValue> {
    let pane = wm.pane(id)?;
    let name = wm
        .agent_mail()
        .name_for_pane(id)
        .map_or_else(|| pane.title().to_string(), str::to_string);
    let (mode, host) = match pane.remote_target() {
        Some(t) => ("ssh", t.rsplit('@').next().unwrap_or(t).to_string()),
        None => ("tmux", "localhost".to_string()),
    };
    let z = wm.z_stack();
    let geom = z.geometries().get(&id);
    let num = |v: Option<u16>| v.map_or(JsonValue::Null, |n| JsonValue::Number(i64::from(n)));
    let z_index = z.order().iter().position(|&p| p == id).map_or(0, |i| i as i64);
    let layout = JsonValue::obj([
        ("isFloating", JsonValue::Bool(wm.layout_mode() == WorkspaceLayoutMode::Stacked)),
        ("x", num(geom.and_then(|g| g.x_offset))),
        ("y", num(geom.and_then(|g| g.y_offset))),
        ("width", num(geom.and_then(|g| g.width))),
        ("height", num(geom.and_then(|g| g.height))),
        ("zIndex", JsonValue::Number(z_index)),
        ("isMinimized", JsonValue::Bool(geom.is_some_and(|g| g.minimized))),
    ]);
    let started = server_start_ms() as i64;
    let last_seq = hub.map_or(0, |h| h.last_seq(id));
    let strs = |v: &[&str]| JsonValue::Array(v.iter().map(|s| JsonValue::String((*s).to_string())).collect());
    Some(JsonValue::obj([
        ("id", JsonValue::String(agent_id(id))),
        ("name", JsonValue::String(name)),
        ("mode", JsonValue::String(mode.to_string())),
        ("targetHost", JsonValue::String(host)),
        ("status", JsonValue::String(status_str(wm, id).to_string())),
        ("unreadOutput", JsonValue::Bool(wm.agent_mail().unread_count_by_pane(id) > 0)),
        ("layout", layout),
        ("createdAt", JsonValue::Number(started)),
        ("lastActivity", JsonValue::Number(started)),
        ("kind", JsonValue::String("terminal".to_string())),
        ("backend", JsonValue::String("bilterm".to_string())),
        ("capabilities", strs(AGENT_CAPABILITIES)),
        ("tags", JsonValue::Array(Vec::new())),
        ("meta", JsonValue::obj([("lastSeq", JsonValue::Number(last_seq as i64))])),
    ]))
}

/// All panes as protocol `Agent`s, in pane-id order.
pub fn agents_json(wm: &WindowManager, hub: Option<&OutputHub>) -> Vec<JsonValue> {
    wm.panes().keys().filter_map(|&id| agent_json(wm, id, hub)).collect()
}

fn split_node_json(node: &SplitNode) -> JsonValue {
    match node {
        SplitNode::Leaf(id) => JsonValue::obj([
            ("type", JsonValue::String("leaf".to_string())),
            ("paneId", JsonValue::Number(*id as i64)),
            ("agentId", JsonValue::String(agent_id(*id))),
        ]),
        SplitNode::Split { direction, ratio_pct, first, second } => JsonValue::obj([
            ("type", JsonValue::String("split".to_string())),
            (
                "direction",
                JsonValue::String(
                    match direction {
                        SplitDirection::Horizontal => "horizontal",
                        SplitDirection::Vertical => "vertical",
                    }
                    .to_string(),
                ),
            ),
            ("ratio", JsonValue::Number(i64::from(*ratio_pct))),
            ("first", split_node_json(first)),
            ("second", split_node_json(second)),
        ]),
    }
}

fn utility(active: bool, focused: bool) -> JsonValue {
    JsonValue::obj([("visible", JsonValue::Bool(active)), ("focused", JsonValue::Bool(focused))])
}

/// Atomic workspace projection for web surfaces.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceViewModelDto {
    pub split_tree: JsonValue,
    pub z_stack: JsonValue,
    pub agents: Vec<JsonValue>,
    pub utility_windows: JsonValue,
    pub css_tokens: String,
    pub layout_mode: &'static str,
    pub epoch: String,
}

impl WorkspaceViewModelDto {
    pub fn from_wm(wm: &WindowManager) -> Self {
        Self::from_wm_with_hub(wm, None)
    }

    pub fn from_wm_with_hub(wm: &WindowManager, hub: Option<&OutputHub>) -> Self {
        let z = wm.z_stack();
        let opt_id = |v: Option<PaneId>| v.map_or(JsonValue::Null, |id| JsonValue::Number(id as i64));
        let z_stack = JsonValue::obj([
            ("order", JsonValue::Array(z.order().iter().map(|&id| JsonValue::Number(id as i64)).collect())),
            ("focusedPaneId", JsonValue::Number(wm.active_pane_id() as i64)),
            ("focusedAgentId", JsonValue::String(agent_id(wm.active_pane_id()))),
            ("zoomedPaneId", opt_id(wm.zoomed_pane())),
        ]);
        let utility_windows = JsonValue::obj([
            ("scriptStudio", utility(wm.script_studio().active, wm.script_studio().focused)),
            ("sessionMap", utility(wm.session_map().active, wm.session_map().focused)),
            ("clipboardViewer", utility(wm.clipboard_window().active, wm.clipboard_window().focused)),
            ("fontsWindow", utility(wm.fonts_window().active, wm.fonts_window().focused)),
            ("fileBrowser", utility(false, false)),
        ]);
        Self {
            split_tree: split_node_json(wm.split_tree().root()),
            z_stack,
            agents: agents_json(wm, hub),
            utility_windows,
            css_tokens: css_tokens(wm),
            layout_mode: match wm.layout_mode() {
                WorkspaceLayoutMode::Tiled => "tiled",
                WorkspaceLayoutMode::Stacked => "stacked",
            },
            epoch: server_epoch(),
        }
    }

    pub fn to_json(&self) -> JsonValue {
        JsonValue::obj([
            ("contract", JsonValue::Number(CONTRACT_VERSION)),
            ("epoch", JsonValue::String(self.epoch.clone())),
            ("layout_mode", JsonValue::String(self.layout_mode.to_string())),
            ("split_tree", self.split_tree.clone()),
            ("z_stack", self.z_stack.clone()),
            ("agents", JsonValue::Array(self.agents.clone())),
            ("utility_windows", self.utility_windows.clone()),
            ("css_tokens", JsonValue::String(self.css_tokens.clone())),
        ])
    }
}

/// Extended health body (`GET /health`, `GET /api/health`).
pub fn health_json(agents: usize) -> JsonValue {
    JsonValue::obj([
        ("ok", JsonValue::Bool(true)),
        ("version", JsonValue::String(env!("CARGO_PKG_VERSION").to_string())),
        ("contract", JsonValue::Number(CONTRACT_VERSION)),
        ("epoch", JsonValue::String(server_epoch())),
        (
            "backends",
            JsonValue::Array(vec![JsonValue::obj([
                ("id", JsonValue::String("bilterm".to_string())),
                ("up", JsonValue::Bool(true)),
            ])]),
        ),
        ("agents", JsonValue::Number(agents as i64)),
    ])
}

/// `LogLine[]` for pane `id` from the output ring (`GET /api/agents/:id/logs`).
pub fn log_lines_json(hub: &OutputHub, id: PaneId, before: Option<u64>, after: Option<u64>, limit: usize) -> (Vec<JsonValue>, bool) {
    let (chunks, has_more) = hub.history(id, before, after, limit);
    let mut lines = Vec::new();
    for c in chunks {
        for (n, text) in c.data.split('\n').enumerate() {
            if text.is_empty() && n > 0 {
                continue;
            }
            lines.push(JsonValue::obj([
                ("agentId", JsonValue::String(agent_id(id))),
                ("seq", JsonValue::Number(c.seq as i64)),
                ("line", JsonValue::Number(n as i64)),
                ("ts", JsonValue::Number(c.ts as i64)),
                ("text", JsonValue::String(text.trim_end_matches('\r').to_string())),
            ]));
        }
    }
    (lines, has_more)
}
