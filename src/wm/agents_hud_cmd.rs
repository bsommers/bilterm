//! Operations of the Phase 70 Mission Control HUD, turn replay and watchdog (HUD-01..03).
//!
//! Every surface (leader key, in-window keys, `:agents hud` / `:agent replay` / `:agents watchdog`,
//! palette, menu, tile mouse clicks) lowers to one of these ops wrapped in
//! [`crate::action::Action`]; the RPC/MCP/CLI methods `agent.hud|replay|watchdog` read or drive the
//! same state (`UNI-R01`).

use crate::pane::PaneId;
use crate::wm::mission_control_hud::HudButtonKind;

/// One `:agents hud ...` operation, lowered to [`crate::action::Action::AgentsHud`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HudOp {
    /// `:agents hud` / `:agents hud open` - open the Mission Control HUD.
    Open,
    /// `:agents hud close` (Esc, `q`) - close it.
    Close,
    /// `:agents hud toggle` (leader `*`, menu) - close when open, else open.
    Toggle,
    /// `:agents hud select next|prev` (arrows, Tab) - move the tile selection.
    Step(i16),
    /// `:agents hud page next|prev` (PageUp / PageDown) - move the tile page.
    Page(i16),
    /// `:agents hud select <pane>` (tile click) - select a tile.
    SelectPane(PaneId),
    /// `:agents hud approve|prompt|diff|zoom [pane]` (`a` `p` `d` `z`, tile buttons).
    ///
    /// `pane == None` targets the selected tile. `generation` is the token the clicked tile carried
    /// (`T-HUD-01`): an Approve whose token no longer matches the live tile is refused.
    Press {
        pane: Option<PaneId>,
        kind: HudButtonKind,
        generation: Option<u64>,
    },
}

/// One `:agent replay ...` operation, lowered to [`crate::action::Action::AgentsReplay`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayOp {
    /// `:agent replay <pane>` - open the read-only replay view over a pane's turn log.
    Open(PaneId),
    /// `:agent replay close` (Esc) - leave the view.
    Close,
    /// `:agent replay prev` (Left) - previous turn.
    Prev,
    /// `:agent replay next` (Right) - next turn.
    Next,
    /// `:agent replay first` (Home) - oldest turn.
    First,
    /// `:agent replay last` (End) - newest turn.
    Last,
    /// `:agent replay turn <id>` - jump to a turn by id.
    Goto(u64),
    /// `:agent replay up|down [n]` (Up / Down) - scroll within the turn (negative = up).
    Scroll(i16),
}

/// One `:agents watchdog ...` operation, lowered to [`crate::action::Action::AgentsWatchdog`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchdogOp {
    /// `:agents watchdog [status]` - report the configuration and recent alerts as data.
    Status,
}

impl HudOp {
    /// The canonical colon form (the inverse of the `:agents hud` parser).
    pub fn colon_form(&self) -> String {
        let p = ":agents hud";
        match self {
            Self::Open => format!("{p} open"),
            Self::Close => format!("{p} close"),
            Self::Toggle => format!("{p} toggle"),
            Self::Step(n) if *n < 0 => format!("{p} select prev"),
            Self::Step(_) => format!("{p} select next"),
            Self::Page(n) if *n < 0 => format!("{p} page prev"),
            Self::Page(_) => format!("{p} page next"),
            Self::SelectPane(id) => format!("{p} select {id}"),
            Self::Press { pane, kind, .. } => {
                let verb = kind.label().trim_matches(['[', ']']).to_ascii_lowercase();
                match pane {
                    Some(id) => format!("{p} {verb} {id}"),
                    None => format!("{p} {verb}"),
                }
            }
        }
    }
}

impl ReplayOp {
    /// The canonical colon form (the inverse of the `:agent replay` parser).
    pub fn colon_form(&self) -> String {
        let p = ":agent replay";
        match self {
            Self::Open(id) => format!("{p} {id}"),
            Self::Close => format!("{p} close"),
            Self::Prev => format!("{p} prev"),
            Self::Next => format!("{p} next"),
            Self::First => format!("{p} first"),
            Self::Last => format!("{p} last"),
            Self::Goto(id) => format!("{p} turn {id}"),
            Self::Scroll(n) if *n < 0 => format!("{p} up {}", n.unsigned_abs()),
            Self::Scroll(n) => format!("{p} down {n}"),
        }
    }
}
